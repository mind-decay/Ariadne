//! Static refactor-suggestion engine. Three deterministic detectors —
//! god modules, cycle-break candidates, misplaced symbols — synthesised
//! from the tier-07 graph metrics. Every output is a *hint* for human (or
//! agent) review, never an authoritative instruction [src: tier-09
//! steps 5-7, plan.md D11].

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

use ariadne_core::{ReadSnapshot, SymbolId};
use petgraph::Direction::{Incoming, Outgoing};
use petgraph::stable_graph::NodeIndex;
use petgraph::visit::EdgeRef;

use crate::build::{EdgeKind, GraphIndex};
use crate::coupling::ModuleSpec;
use crate::cycles::Cycle;
use crate::errors::GraphError;
use crate::heuristics::{self, SymbolTable};

/// Cohesion below this value, paired with high efferent coupling, marks a
/// module as a "god module" candidate [src: tier-09 step 5].
const COHESION_FLOOR: f32 = 0.3;
/// Cap on the `top_outbound` list inside a [`GodModuleFinding`].
const TOP_OUTBOUND: usize = 5;

/// A module with high efferent coupling and low cohesion — a split
/// candidate.
#[derive(Debug, Clone)]
pub struct GodModuleFinding {
    /// Module name copied from the input [`ModuleSpec`].
    pub module: String,
    /// Efferent coupling (Ce) — distinct external symbols depended on.
    pub efferent: u32,
    /// Cohesion proxy in `[0, 1]`.
    pub cohesion: f32,
    /// Outbound traffic grouped by target symbol, ranked by edge count.
    pub top_outbound: Vec<(SymbolId, u32)>,
    /// Human-readable split suggestion referencing the hottest target.
    pub suggestion: String,
}

/// A directed edge inside a cycle, ranked as a removal / inversion
/// candidate.
#[derive(Debug, Clone)]
pub struct CycleBreakProposal {
    /// Source symbol of the edge.
    pub from: SymbolId,
    /// Destination symbol of the edge.
    pub to: SymbolId,
    /// Cut score in `(0, 1]`; higher = lower-traffic = cheaper to cut.
    pub score: f32,
    /// Static rationale naming the relevant design principle.
    pub rationale: &'static str,
}

/// A symbol whose callers predominantly live in a different module.
#[derive(Debug, Clone)]
pub struct MisplacedSymbol {
    /// The symbol that looks misplaced.
    pub symbol: SymbolId,
    /// Module the symbol is currently defined in.
    pub current_module: String,
    /// Module most of its callers belong to.
    pub target_module: String,
    /// Ratio of dominant-external call count to own-module call count
    /// (own clamped to ≥ 1 for the division).
    pub ratio: f32,
}

/// Rationale string cited by every [`CycleBreakProposal`].
const CYCLE_RATIONALE: &str = "Dependency-Inversion Principle: invert this edge behind an \
abstraction owned by the dependee. Lowest combined fan-in/fan-out makes it the cheapest cut \
(Martin instability metric I).";

/// Detect god modules: `Ce > threshold` **and** `cohesion < 0.3`.
///
/// # Errors
/// Propagates [`GraphError::Storage`] when the snapshot scan fails.
pub fn god_modules(
    graph: &GraphIndex,
    snap: &dyn ReadSnapshot,
    modules: &[ModuleSpec],
    threshold: f32,
) -> Result<Vec<GodModuleFinding>, GraphError> {
    let table = SymbolTable::from_snapshot(snap)?;
    let mut out = Vec::new();
    for module in modules {
        let member_ix: BTreeSet<NodeIndex> = module
            .members
            .iter()
            .filter_map(|s| graph.index.get(s).copied())
            .collect();
        let mut outbound: BTreeMap<SymbolId, u32> = BTreeMap::new();
        let mut total_out: u32 = 0;
        for &ix in &member_ix {
            for er in graph.graph.edges_directed(ix, Outgoing) {
                if !member_ix.contains(&er.target()) {
                    *outbound.entry(graph.graph[er.target()]).or_default() += 1;
                    total_out += 1;
                }
            }
        }
        let efferent = u32::try_from(outbound.len()).unwrap_or(u32::MAX);
        let cohesion = heuristics::cohesion(graph, module);
        if f64::from(efferent) <= f64::from(threshold) || cohesion >= COHESION_FLOOR {
            continue;
        }
        let mut top: Vec<(SymbolId, u32)> = outbound.into_iter().collect();
        top.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        top.truncate(TOP_OUTBOUND);
        let suggestion = top.first().map_or_else(String::new, |&(sym, cnt)| {
            let pct = pct_of(cnt, total_out);
            format!(
                "Consider splitting `{}` out into its own module — currently {pct}% of \
outbound traffic flows through it.",
                table.name(sym)
            )
        });
        out.push(GodModuleFinding {
            module: module.name.clone(),
            efferent,
            cohesion,
            top_outbound: top,
            suggestion,
        });
    }
    out.sort_by(|a, b| a.module.cmp(&b.module));
    Ok(out)
}

/// Rank the edges inside one strongly-connected component as cycle-break
/// candidates — cheapest (lowest-traffic) cut first.
#[must_use]
pub fn cycle_break_proposals(graph: &GraphIndex, scc: &Cycle) -> Vec<CycleBreakProposal> {
    let members: BTreeSet<SymbolId> = scc.members.iter().copied().collect();
    let mut props = Vec::new();
    for &from in &scc.members {
        let Some(&ix) = graph.index.get(&from) else {
            continue;
        };
        for er in graph.graph.edges_directed(ix, Outgoing) {
            let to = graph.graph[er.target()];
            if !members.contains(&to) {
                continue;
            }
            props.push(CycleBreakProposal {
                from,
                to,
                score: heuristics::cut_score(graph.fan_in(from), graph.fan_out(to)),
                rationale: CYCLE_RATIONALE,
            });
        }
    }
    props.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(Ordering::Equal)
            .then(a.from.cmp(&b.from))
            .then(a.to.cmp(&b.to))
    });
    props
}

/// Detect misplaced symbols: a symbol whose dominant external caller
/// module makes more than twice as many calls as its own module.
///
/// Tier-09 step 7's "visibility allows movement" guard is omitted:
/// `SymbolRecord` carries no visibility flag pre-SCIP-refinement, so
/// every symbol is treated as movable.
#[must_use]
pub fn misplaced_symbols(graph: &GraphIndex, modules: &[ModuleSpec]) -> Vec<MisplacedSymbol> {
    let member_of = heuristics::member_index(graph, modules);
    let mut out = Vec::new();
    for (mid, module) in modules.iter().enumerate() {
        for &symbol in &module.members {
            let Some(&ix) = graph.index.get(&symbol) else {
                continue;
            };
            let mut hist: BTreeMap<usize, u32> = BTreeMap::new();
            let mut own: u32 = 0;
            for er in graph.graph.edges_directed(ix, Incoming) {
                let kind = er.weight().kind;
                if kind != EdgeKind::Calls && kind != EdgeKind::Imports {
                    continue;
                }
                match member_of.get(&er.source()) {
                    Some(&cm) if cm == mid => own += 1,
                    Some(&cm) => *hist.entry(cm).or_default() += 1,
                    None => {}
                }
            }
            // Pick the heaviest external module; ties break to the lower
            // module index for determinism.
            let best = hist.iter().max_by(|a, b| a.1.cmp(b.1).then(b.0.cmp(a.0)));
            if let Some((&target, &cnt)) = best {
                if cnt > own.saturating_mul(2) {
                    out.push(MisplacedSymbol {
                        symbol,
                        current_module: module.name.clone(),
                        target_module: modules[target].name.clone(),
                        ratio: heuristics::ratio(cnt, own.max(1)),
                    });
                }
            }
        }
    }
    out.sort_by_key(|m| m.symbol);
    out
}

/// Percentage of `total` represented by `part`, rounded to a whole
/// number. A zero total yields 0.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn pct_of(part: u32, total: u32) -> u32 {
    if total == 0 {
        return 0;
    }
    (f64::from(part) / f64::from(total) * 100.0).round() as u32
}
