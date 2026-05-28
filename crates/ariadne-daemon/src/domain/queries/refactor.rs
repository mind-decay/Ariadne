//! Refactor-hint query: `refactor_suggestions` — god modules ∪ cycle breaks ∪
//! misplaced symbols. Mirrors the v1 MCP `refactor_suggestions` tool against
//! the warm graph + snapshot mirror, substituting [`WarmCatalog`] for the MCP
//! `Catalog` + redb snapshot. Every finding is a *hint* for human or agent
//! review, never an authoritative command
//! [src: crates/ariadne-mcp/src/tools/refactor.rs].

use ariadne_core::{
    CycleBreakRow, DaemonResponse, GodModuleRow, MisplacedRow, OutboundRow, RefactorReport,
    SymbolId,
};

use crate::domain::catalog::WarmCatalog;
use crate::domain::queries::health::build_modules;

/// Efferent-coupling threshold above which a low-cohesion module is flagged as
/// a god module (matches the v1 MCP `refactor_suggestions` tuning).
const GOD_THRESHOLD: f32 = 8.0;

/// Aggregate the three refactor detectors, scoped by `prefix`.
pub(crate) fn refactor_suggestions(cat: &WarmCatalog, prefix: Option<&str>) -> DaemonResponse {
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

    DaemonResponse::Refactor(RefactorReport {
        god_modules,
        cycle_breaks,
        misplaced_symbols,
    })
}

/// Resolve a symbol id to its canonical name, falling back to `#id` — matches
/// the v1 MCP projector.
fn name_of(cat: &WarmCatalog, id: SymbolId) -> String {
    cat.meta_of(id)
        .map_or_else(|| format!("#{}", id.get()), |m| m.name.clone())
}
