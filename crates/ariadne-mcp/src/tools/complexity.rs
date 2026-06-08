//! `complexity` — `McCabe` cyclomatic complexity ranking at file or symbol grain
//! (tier-15b). No graph use case exists: tier-13 D2 places the file-complexity
//! fold at the composition root, so the handler folds the catalog's per-symbol
//! `complexity` into per-file Σ (file grain) or per-symbol rows (symbol grain)
//! and ranks descending. Logic identical to the daemon
//! `queries::analytics::complexity` so cold and warm JSON match. Block 1
//! tier-02 caps the result to a default page + cursor and projects symbol-grain
//! rows at the requested verbosity (concise default drops the embedded symbol's
//! cryptic id/offset fields) via the shared `ariadne_graph::economy` helper
//! [src: block-1 plan.md D1-D5].

use std::cmp::Ordering;
use std::collections::BTreeMap;

use ariadne_graph::economy::{self, Budget, Verbosity};

use crate::catalog::Catalog;
use crate::errors::McpError;
use crate::tools::summarize;
use crate::types::{
    ComplexityOutput, ComplexityRow, Grain, GrainScopeInput, Verbosity as WireVerbosity,
};

/// Whether `path` is in scope for an optional path prefix (`None` = all).
fn in_scope(path: &str, prefix: Option<&str>) -> bool {
    prefix.is_none_or(|p| path.starts_with(p))
}

/// Rank `McCabe` complexity at `input.grain`, filtered by prefix, descending,
/// capped to one page in stable (complexity desc, then key asc) order.
///
/// # Errors
/// Returns [`McpError::InvalidInput`] when `input.cursor` is malformed or was
/// minted against a different index revision.
pub fn handle(cat: &Catalog, input: &GrainScopeInput) -> Result<ComplexityOutput, McpError> {
    let prefix = input.prefix.as_deref();
    let rows = match input.grain {
        Grain::File => {
            let mut by_file: BTreeMap<String, u32> = BTreeMap::new();
            for meta in cat.symbols.values() {
                let Some(path) = cat.path_of(meta.file) else {
                    continue;
                };
                if in_scope(path, prefix) {
                    *by_file.entry(path.to_owned()).or_insert(0) += meta.complexity;
                }
            }
            by_file
                .into_iter()
                .map(|(file, complexity)| ComplexityRow {
                    file,
                    symbol: None,
                    complexity,
                })
                .collect::<Vec<_>>()
        }
        Grain::Symbol => cat
            .symbols
            .iter()
            .filter(|(_, meta)| in_scope(cat.path_of(meta.file).unwrap_or(""), prefix))
            .map(|(id, meta)| ComplexityRow {
                file: String::new(),
                symbol: Some(summarize(cat, *id)),
                complexity: meta.complexity,
            })
            .collect::<Vec<_>>(),
    };
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
    let rows: Vec<ComplexityRow> = paged
        .rows
        .into_iter()
        .map(|r| project_row(r, verbosity))
        .collect();
    let note = paged
        .next_cursor
        .as_ref()
        .map(|_| economy::truncation_note(rows.len(), total, "complexity rows"));
    Ok(ComplexityOutput {
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

/// Stable order for a complexity page: most complex first (complexity desc),
/// then key (file path, then symbol id) ascending — a deterministic top-N (D4).
fn cmp_row(a: &ComplexityRow, b: &ComplexityRow) -> Ordering {
    b.complexity
        .cmp(&a.complexity)
        .then_with(|| key(a).cmp(&key(b)))
}

/// Sort key: file path (file grain) or symbol id (symbol grain); breaks
/// complexity ties ascending. Read before any concise projection nulls the id,
/// so the embedded `id` is `Some`.
fn key(row: &ComplexityRow) -> (&str, u64) {
    (
        row.file.as_str(),
        row.symbol.as_ref().and_then(|s| s.id).unwrap_or(0),
    )
}

/// Drop the embedded symbol's cryptic id/offset fields in concise verbosity (D3).
/// File-grain rows carry no symbol, so concise == detailed for them.
fn project_row(mut row: ComplexityRow, verbosity: Verbosity) -> ComplexityRow {
    if matches!(verbosity, Verbosity::Concise) {
        if let Some(sym) = row.symbol.as_mut() {
            sym.id = None;
            sym.byte_start = None;
            sym.byte_end = None;
        }
    }
    row
}
