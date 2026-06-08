//! `refactor_suggestions` — god modules, cycle breaks, misplaced symbols.
//!
//! Every finding is a *hint* for human or agent review, never an
//! authoritative command (tier-09 step 12).
//!
//! Block 1 tier-03 caps each of the three lists independently behind ONE opaque
//! multi-list cursor via the shared `ariadne_graph::economy` helper so the cold
//! and warm paths stay byte-identical. Verbosity is a no-op — every refactor
//! row is name/metric only, so concise == detailed; the cap is the economy win
//! [src: .claude/plans/data-fidelity-arc/block-1/plan.md D1-D5].

use std::cmp::Ordering;

use ariadne_core::{Storage, SymbolId};
use ariadne_graph::economy::{self, Budget};

use crate::catalog::Catalog;
use crate::errors::McpError;
use crate::tools::coupling_report::build_modules;
use crate::types::{
    CycleBreakRow, GodModuleRow, MisplacedRow, OutboundRow, RefactorInput, RefactorOutput,
};

/// Efferent-coupling threshold above which a low-cohesion module is
/// flagged as a god module (matches `weak_spots`).
const GOD_THRESHOLD: f32 = 8.0;

/// Aggregate the three refactor detectors, scoped by `input.prefix`, each list
/// capped to one page sharing a single multi-list cursor.
///
/// # Errors
/// [`McpError::Storage`] / [`McpError::Graph`] on snapshot or analysis
/// failure, or [`McpError::InvalidInput`] when `input.cursor` is malformed or
/// was minted against a different index revision.
pub fn handle<S: Storage>(
    cat: &Catalog,
    storage: &S,
    input: &RefactorInput,
) -> Result<RefactorOutput, McpError> {
    let modules = build_modules(cat, input.prefix.as_deref());
    let snap = storage.snapshot().map_err(McpError::Storage)?;
    let gods = ariadne_graph::refactor::god_modules(&cat.graph, &snap, &modules, GOD_THRESHOLD)
        .map_err(McpError::Graph)?;
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
    let cursor = input
        .cursor
        .as_deref()
        .map(|c| economy::Cursor::decode(c, revision))
        .transpose()
        .map_err(|e| McpError::InvalidInput(e.to_string()))?;
    Ok(page(
        god_modules,
        cycle_breaks,
        misplaced_symbols,
        cursor,
        input,
        revision,
    ))
}

/// Stable order for the god-module page: most-efferent first, then module path
/// ascending (D4).
fn cmp_god(a: &GodModuleRow, b: &GodModuleRow) -> Ordering {
    b.efferent.cmp(&a.efferent).then(a.module.cmp(&b.module))
}

/// Stable order for the cycle-break page: highest cut score first, then the
/// `(from, to)` edge ascending (D4). Score is an `f32`; `total_cmp` gives a
/// total order.
fn cmp_break(a: &CycleBreakRow, b: &CycleBreakRow) -> Ordering {
    b.score
        .total_cmp(&a.score)
        .then_with(|| (&a.from, &a.to).cmp(&(&b.from, &b.to)))
}

/// Stable order for the misplaced-symbol page: highest displacement ratio
/// first, then symbol name ascending (D4).
fn cmp_misplaced(a: &MisplacedRow, b: &MisplacedRow) -> Ordering {
    b.ratio
        .total_cmp(&a.ratio)
        .then_with(|| a.symbol.cmp(&b.symbol))
}

/// Sort, cap, and steer the three lists behind one multi-list cursor. Shared
/// shape with the warm daemon handler so their JSON is byte-identical (parity).
/// No concise projection: every row is name/metric only (verbosity no-op).
// Each parameter is a distinct sublist or page input; bundling them would only
// add indirection.
#[allow(clippy::too_many_arguments)]
fn page(
    god_modules: Vec<GodModuleRow>,
    cycle_breaks: Vec<CycleBreakRow>,
    misplaced_symbols: Vec<MisplacedRow>,
    cursor: Option<economy::Cursor>,
    input: &RefactorInput,
    revision: u32,
) -> RefactorOutput {
    let budget = Budget {
        limit: input.limit.map_or(economy::DEFAULT_PAGE, |l| l as usize),
        cursor,
        verbosity: economy::Verbosity::Concise,
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
    RefactorOutput {
        god_modules: gods_page.rows,
        cycle_breaks: breaks_page.rows,
        misplaced_symbols: misplaced_page.rows,
        next_cursor,
        note,
    }
}

/// Resolve a symbol id to its canonical name, falling back to `#id`.
fn name_of(cat: &Catalog, id: SymbolId) -> String {
    cat.meta_of(id)
        .map_or_else(|| format!("#{}", id.get()), |m| m.name.clone())
}
