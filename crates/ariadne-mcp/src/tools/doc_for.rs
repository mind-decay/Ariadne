//! `doc_for` — structured doc summary for one symbol.

use crate::catalog::Catalog;
use crate::errors::McpError;
use crate::tools::summarize;
use crate::types::{DocForOutput, SymbolQuery, SymbolSummary};

const MAX_PUBLIC_REFS: usize = 16;

/// Doc-like structured summary for `input.symbol`.
///
/// # Errors
/// Returns [`McpError::NotFound`] when `input.symbol` is unknown.
pub fn handle(cat: &Catalog, input: &SymbolQuery) -> Result<DocForOutput, McpError> {
    let id = cat
        .find_symbol(&input.symbol)
        .ok_or_else(|| McpError::NotFound(format!("symbol {}", input.symbol)))?;
    let meta = cat
        .meta_of(id)
        .ok_or_else(|| McpError::NotFound(format!("symbol meta {}", input.symbol)))?;
    let file = cat.path_of(meta.file).unwrap_or("").to_owned();
    let signature = format!("{} {}", meta.kind, meta.name);
    let brief = meta.name.clone();

    // First N callers — uses `GraphIndex::fan_in` via `blast_radius` depth 1.
    // `id` came from the catalog and every catalog symbol is a graph node,
    // so the `Option` is always `Some`; `unwrap_or_default` keeps the
    // empty-radius fallback for the unreachable desync case.
    let radius = cat
        .graph
        .blast_radius(id, 1, ariadne_graph::EdgeKindSet::ALL)
        .unwrap_or_default();
    let mut callers: Vec<SymbolSummary> = radius
        .must_touch
        .into_iter()
        .chain(radius.may_touch)
        .take(MAX_PUBLIC_REFS)
        .map(|s| summarize(cat, s))
        .collect();
    callers.sort_by_key(|a| a.id);

    Ok(DocForOutput {
        signature,
        kind: meta.kind.clone(),
        file,
        brief,
        public_refs: callers,
    })
}
