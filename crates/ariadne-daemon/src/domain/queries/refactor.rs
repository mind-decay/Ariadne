//! Refactor-hint query: `refactor_suggestions` — god modules ∪ cycle breaks ∪
//! misplaced symbols. Mirrors the v1 MCP `refactor_suggestions` tool against
//! the warm graph + snapshot mirror, substituting [`WarmCatalog`] for the MCP
//! `Catalog` + redb snapshot. Every finding is a *hint* for human or agent
//! review, never an authoritative command. Block 1 tier-03 caps each of the
//! three lists independently behind one multi-list cursor via the shared
//! `ariadne_graph::economy` helper so the cold and warm paths stay
//! byte-identical (verbosity is a no-op — the rows are name/metric only)
//! [src: crates/ariadne-mcp/src/tools/refactor.rs].

use std::cmp::Ordering;

use ariadne_core::{
    CycleBreakRow, DaemonResponse, GodModuleRow, MisplacedRow, OutboundRow, RefactorReport,
    SymbolId, Verbosity,
};
use ariadne_graph::economy::{self, Budget, Verbosity as EconVerbosity};

use crate::domain::catalog::WarmCatalog;
use crate::domain::queries::health::build_modules;

/// Efferent-coupling threshold above which a low-cohesion module is flagged as
/// a god module (matches the v1 MCP `refactor_suggestions` tuning).
const GOD_THRESHOLD: f32 = 8.0;

/// Aggregate the three refactor detectors, scoped by `prefix`, each list capped
/// to one page sharing a single multi-list cursor — the warm twin of the cold
/// `tools::refactor` handler, so their JSON is byte-identical (parity). A
/// malformed / stale cursor surfaces as a typed `DaemonResponse::Error`.
pub(crate) fn refactor_suggestions(
    cat: &WarmCatalog,
    prefix: Option<&str>,
    limit: Option<u32>,
    cursor: Option<&str>,
    verbosity: Verbosity,
) -> DaemonResponse {
    let modules = build_modules(cat, prefix);

    let gods = match ariadne_graph::refactor::god_modules(
        &cat.graph,
        &cat.snap,
        &modules,
        GOD_THRESHOLD,
    ) {
        Ok(gods) => gods,
        Err(err) => return DaemonResponse::Error(err.to_string()),
    };
    let god_modules: Vec<GodModuleRow> = gods
        .into_iter()
        .map(|g| GodModuleRow {
            module: g.module,
            efferent: g.efferent,
            cohesion: g.cohesion,
            top_outbound: g
                .top_outbound
                .into_iter()
                .map(|(s, edges)| OutboundRow {
                    symbol: name_of(cat, s),
                    edges,
                })
                .collect(),
            suggestion: g.suggestion,
        })
        .collect();

    let mut cycle_breaks = Vec::new();
    for cycle in cat.graph.cycle_report().cycles {
        for p in ariadne_graph::refactor::cycle_break_proposals(&cat.graph, &cycle) {
            cycle_breaks.push(CycleBreakRow {
                from: name_of(cat, p.from),
                to: name_of(cat, p.to),
                score: p.score,
                rationale: p.rationale.to_owned(),
            });
        }
    }

    let misplaced_symbols: Vec<MisplacedRow> =
        ariadne_graph::refactor::misplaced_symbols(&cat.graph, &modules)
            .into_iter()
            .map(|m| MisplacedRow {
                symbol: name_of(cat, m.symbol),
                current_module: m.current_module,
                target_module: m.target_module,
                ratio: m.ratio,
            })
            .collect();

    let revision = u32::try_from(cat.revision).unwrap_or(u32::MAX);
    let decoded = match cursor
        .map(|c| economy::Cursor::decode(c, revision))
        .transpose()
    {
        Ok(c) => c,
        Err(err) => return DaemonResponse::Error(err.to_string()),
    };
    let budget = Budget {
        limit: limit.map_or(economy::DEFAULT_PAGE, |l| l as usize),
        cursor: decoded,
        verbosity: to_economy(verbosity),
    };
    let total_gods = god_modules.len();
    let total_breaks = cycle_breaks.len();
    let total_misplaced = misplaced_symbols.len();
    let gods_page = economy::paginate_sublist(god_modules, cmp_god, &budget, 0);
    let breaks_page = economy::paginate_sublist(cycle_breaks, cmp_break, &budget, 1);
    let misplaced_page = economy::paginate_sublist(misplaced_symbols, cmp_misplaced, &budget, 2);
    let next_cursor = economy::multi_cursor(
        &[
            (gods_page.next_offset, gods_page.remainder),
            (breaks_page.next_offset, breaks_page.remainder),
            (misplaced_page.next_offset, misplaced_page.remainder),
        ],
        revision,
    );
    let mut truncated = Vec::new();
    if gods_page.remainder {
        truncated.push((gods_page.rows.len(), total_gods, "god_modules"));
    }
    if breaks_page.remainder {
        truncated.push((breaks_page.rows.len(), total_breaks, "cycle_breaks"));
    }
    if misplaced_page.remainder {
        truncated.push((
            misplaced_page.rows.len(),
            total_misplaced,
            "misplaced_symbols",
        ));
    }
    let note = next_cursor
        .as_ref()
        .map(|_| economy::multi_truncation_note(&truncated));
    DaemonResponse::Refactor(RefactorReport {
        god_modules: gods_page.rows,
        cycle_breaks: breaks_page.rows,
        misplaced_symbols: misplaced_page.rows,
        next_cursor,
        note,
    })
}

/// Map the protocol verbosity onto the economy use case's verbosity.
fn to_economy(v: Verbosity) -> EconVerbosity {
    match v {
        Verbosity::Concise => EconVerbosity::Concise,
        Verbosity::Detailed => EconVerbosity::Detailed,
    }
}

/// Stable order for the god-module page (identical to the cold handler, D4):
/// most-efferent first, then module path ascending.
fn cmp_god(a: &GodModuleRow, b: &GodModuleRow) -> Ordering {
    b.efferent.cmp(&a.efferent).then(a.module.cmp(&b.module))
}

/// Stable order for the cycle-break page (identical to the cold handler, D4):
/// highest cut score first, then the `(from, to)` edge ascending.
fn cmp_break(a: &CycleBreakRow, b: &CycleBreakRow) -> Ordering {
    b.score
        .total_cmp(&a.score)
        .then_with(|| (&a.from, &a.to).cmp(&(&b.from, &b.to)))
}

/// Stable order for the misplaced-symbol page (identical to the cold handler,
/// D4): highest displacement ratio first, then symbol name ascending.
fn cmp_misplaced(a: &MisplacedRow, b: &MisplacedRow) -> Ordering {
    b.ratio
        .total_cmp(&a.ratio)
        .then_with(|| a.symbol.cmp(&b.symbol))
}

/// Resolve a symbol id to its canonical name, falling back to `#id` — matches
/// the v1 MCP projector.
fn name_of(cat: &WarmCatalog, id: SymbolId) -> String {
    cat.meta_of(id)
        .map_or_else(|| format!("#{}", id.get()), |m| m.name.clone())
}
