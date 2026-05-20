//! `refactor_suggestions` — god modules, cycle breaks, misplaced symbols.
//!
//! Every finding is a *hint* for human or agent review, never an
//! authoritative command (tier-09 step 12).

use ariadne_core::{Storage, SymbolId};

use crate::catalog::Catalog;
use crate::errors::McpError;
use crate::tools::coupling_report::build_modules;
use crate::types::{
    CycleBreakRow, GodModuleRow, MisplacedRow, OutboundRow, RefactorOutput, ScopeInput,
};

/// Efferent-coupling threshold above which a low-cohesion module is
/// flagged as a god module (matches `weak_spots`).
const GOD_THRESHOLD: f32 = 8.0;

/// Aggregate the three refactor detectors, scoped by `scope.prefix`.
///
/// # Errors
/// [`McpError::Storage`] / [`McpError::Graph`] on snapshot or analysis
/// failure.
pub fn handle<S: Storage>(
    cat: &Catalog,
    storage: &S,
    scope: &ScopeInput,
) -> Result<RefactorOutput, McpError> {
    let modules = build_modules(cat, scope.prefix.as_deref());
    let snap = storage.snapshot().map_err(McpError::Storage)?;
    let gods = ariadne_graph::refactor::god_modules(&cat.graph, &snap, &modules, GOD_THRESHOLD)
        .map_err(McpError::Graph)?;
    let god_modules = gods
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

    let misplaced_symbols = ariadne_graph::refactor::misplaced_symbols(&cat.graph, &modules)
        .into_iter()
        .map(|m| MisplacedRow {
            symbol: name_of(cat, m.symbol),
            current_module: m.current_module,
            target_module: m.target_module,
            ratio: m.ratio,
        })
        .collect();

    Ok(RefactorOutput {
        god_modules,
        cycle_breaks,
        misplaced_symbols,
    })
}

/// Resolve a symbol id to its canonical name, falling back to `#id`.
fn name_of(cat: &Catalog, id: SymbolId) -> String {
    cat.meta_of(id)
        .map_or_else(|| format!("#{}", id.get()), |m| m.name.clone())
}
