//! Navigation queries: `list_symbols`, `find_definition`, `find_references`.

use std::collections::BTreeMap;

use ariadne_core::{DaemonResponse, ReadSnapshot, ReferenceSite};

use crate::domain::catalog::WarmCatalog;
use crate::domain::dispatch::summarize;

const DEFAULT_LIMIT: u32 = 64;

/// Top-K symbols whose canonical name contains `query` (case-insensitive)
/// and whose kind matches `kind` when provided.
pub(crate) fn list_symbols(
    cat: &WarmCatalog,
    query: &str,
    kind: Option<&str>,
    limit: Option<u32>,
) -> DaemonResponse {
    let limit = usize::try_from(limit.unwrap_or(DEFAULT_LIMIT).max(1)).unwrap_or(usize::MAX);
    let needle = query.to_lowercase();
    let mut out = Vec::with_capacity(limit.min(64));
    for (id, meta) in &cat.symbols {
        if !needle.is_empty() && !meta.name.to_lowercase().contains(&needle) {
            continue;
        }
        if let Some(want) = kind {
            if meta.kind != want {
                continue;
            }
        }
        out.push(summarize(cat, *id));
        if out.len() >= limit {
            break;
        }
    }
    DaemonResponse::Symbols(out)
}

/// Resolve `symbol` to its defining [`SymbolSummary`](ariadne_core::SymbolSummary).
pub(crate) fn find_definition(cat: &WarmCatalog, symbol: &str) -> DaemonResponse {
    match cat.find_symbol(symbol) {
        Some(id) => DaemonResponse::Definition(summarize(cat, id)),
        None => DaemonResponse::Error(format!("symbol {symbol} not found")),
    }
}

/// Reference sites whose target is `symbol`, one row per distinct caller
/// (last edge wins for the span), keyed and ordered by caller id.
pub(crate) fn find_references(cat: &WarmCatalog, symbol: &str) -> DaemonResponse {
    let Some(id) = cat.find_symbol(symbol) else {
        return DaemonResponse::Error(format!("symbol {symbol} not found"));
    };
    let edges = match cat.snap.incoming_edges(id) {
        Ok(edges) => edges,
        Err(err) => return DaemonResponse::Error(err.to_string()),
    };
    let mut by_caller: BTreeMap<u64, ReferenceSite> = BTreeMap::new();
    for (key, rec) in edges {
        let caller_name = cat
            .meta_of(key.src)
            .map(|m| m.name.clone())
            .unwrap_or_default();
        let file = cat.path_of(rec.source_span.file).unwrap_or("").to_owned();
        by_caller.insert(
            key.src.get(),
            ReferenceSite {
                caller: key.src.get(),
                caller_name,
                file,
                byte_start: rec.source_span.byte_start,
                byte_end: rec.source_span.byte_end,
            },
        );
    }
    DaemonResponse::References(by_caller.into_values().collect())
}
