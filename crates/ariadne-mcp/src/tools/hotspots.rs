//! `hotspots` — churn × complexity ranking at file or symbol grain (tier-15b).
//!
//! Builds the complexity map the tier-13 use case needs from the catalog's
//! per-symbol `complexity` (file grain → per-file Σ; symbol grain →
//! passthrough), calls `file_hotspots` / `symbol_hotspots`, and projects the
//! result to the wire row shape. Logic identical to the daemon
//! `queries::analytics::hotspots` so cold and warm JSON match
//! [src: crates/ariadne-graph/src/hotspot.rs:102-150]. Block 1 tier-02 caps the
//! result to a default page + cursor and projects symbol-grain rows at the
//! requested verbosity (concise default drops the embedded symbol's cryptic
//! id/offset fields) via the shared `ariadne_graph::economy` helper
//! [src: block-1 plan.md D1-D5].

use std::cmp::Ordering;
use std::collections::BTreeMap;

use ariadne_core::SymbolId;
use ariadne_graph::economy::{self, Budget, Verbosity};
use ariadne_graph::{HotspotGrain, HotspotReport as GraphHotspots, file_hotspots, symbol_hotspots};

use crate::catalog::Catalog;
use crate::errors::McpError;
use crate::tools::summarize;
use crate::types::{Grain, GrainScopeInput, HotspotOutput, HotspotRow, Verbosity as WireVerbosity};

/// Whether `path` is in scope for an optional path prefix (`None` = all).
fn in_scope(path: &str, prefix: Option<&str>) -> bool {
    prefix.is_none_or(|p| path.starts_with(p))
}

/// Rank churn × complexity hotspots at `input.grain`, filtered by prefix and
/// capped to one page in stable (score desc, then file / symbol-id asc) order.
///
/// # Errors
/// Returns [`McpError::InvalidInput`] when `input.cursor` is malformed or was
/// minted against a different index revision.
pub fn handle(cat: &Catalog, input: &GrainScopeInput) -> Result<HotspotOutput, McpError> {
    let prefix = input.prefix.as_deref();
    let report = match input.grain {
        Grain::File => {
            let mut file_complexity: BTreeMap<String, u32> = BTreeMap::new();
            for meta in cat.symbols.values() {
                if let Some(path) = cat.path_of(meta.file) {
                    *file_complexity.entry(path.to_owned()).or_insert(0) += meta.complexity;
                }
            }
            file_hotspots(&cat.churn, &file_complexity)
        }
        Grain::Symbol => {
            let symbol_complexity: BTreeMap<SymbolId, u32> = cat
                .symbols
                .iter()
                .map(|(id, m)| (*id, m.complexity))
                .collect();
            symbol_hotspots(&cat.symbol_churn, &symbol_complexity)
        }
    };
    let rows = project(cat, report, prefix);
    let verbosity = to_economy(input.verbosity);
    let revision = u32::try_from(cat.revision).unwrap_or(u32::MAX);
    let cursor = input
        .cursor
        .as_deref()
        .map(|c| economy::Cursor::decode(c, revision))
        .transpose()
        .map_err(|e| McpError::InvalidInput(e.to_string()))?;
    let budget = Budget {
        limit: input.limit.map_or(economy::DEFAULT_PAGE, |l| l as usize),
        cursor,
        verbosity,
    };
    let total = rows.len();
    let paged = economy::paginate(rows, cmp_row, &budget, revision, 0);
    let rows: Vec<HotspotRow> = paged
        .rows
        .into_iter()
        .map(|r| project_row(r, verbosity))
        .collect();
    let note = paged
        .next_cursor
        .as_ref()
        .map(|_| economy::truncation_note(rows.len(), total, "hotspots"));
    Ok(HotspotOutput {
        rows,
        next_cursor: paged.next_cursor,
        note,
    })
}

/// Map the MCP-facing verbosity onto the economy use case's verbosity.
fn to_economy(v: WireVerbosity) -> Verbosity {
    match v {
        WireVerbosity::Concise => Verbosity::Concise,
        WireVerbosity::Detailed => Verbosity::Detailed,
    }
}

/// Stable order for a hotspot page: strongest first (score desc), then file
/// path / symbol id ascending — a meaningful, deterministic top-N (D4). Score
/// is an `f32`; `total_cmp` gives a total order (no NaN in `[0,1]`).
fn cmp_row(a: &HotspotRow, b: &HotspotRow) -> Ordering {
    b.score
        .total_cmp(&a.score)
        .then_with(|| a.file.cmp(&b.file))
        .then_with(|| sym_id(a).cmp(&sym_id(b)))
}

/// The embedded symbol id used as the symbol-grain tie-break (0 for a file-grain
/// row). Read before any concise projection nulls the field, so it is `Some`.
fn sym_id(row: &HotspotRow) -> u64 {
    row.symbol.as_ref().and_then(|s| s.id).unwrap_or(0)
}

/// Drop the embedded symbol's cryptic id/offset fields in concise verbosity (D3).
/// File-grain rows carry no symbol, so concise == detailed for them.
fn project_row(mut row: HotspotRow, verbosity: Verbosity) -> HotspotRow {
    if matches!(verbosity, Verbosity::Concise) {
        if let Some(sym) = row.symbol.as_mut() {
            sym.id = None;
            sym.byte_start = None;
            sym.byte_end = None;
        }
    }
    row
}

/// Project a graph hotspot report into wire rows, dropping out-of-scope units.
fn project(cat: &Catalog, report: GraphHotspots, prefix: Option<&str>) -> Vec<HotspotRow> {
    report
        .entries
        .into_iter()
        .filter_map(|e| match e.grain {
            HotspotGrain::File { path } => in_scope(&path, prefix).then_some(HotspotRow {
                file: path,
                symbol: None,
                churn: e.churn,
                complexity: e.complexity,
                score: e.score,
            }),
            HotspotGrain::Symbol { symbol } => {
                let sym = summarize(cat, symbol);
                in_scope(&sym.file, prefix).then_some(HotspotRow {
                    file: String::new(),
                    symbol: Some(sym),
                    churn: e.churn,
                    complexity: e.complexity,
                    score: e.score,
                })
            }
        })
        .collect()
}
