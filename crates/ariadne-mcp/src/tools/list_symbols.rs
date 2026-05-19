//! `list_symbols` — substring filter on canonical name.

use crate::catalog::Catalog;
use crate::tools::summarize;
use crate::types::{ListSymbolsInput, SymbolSummary};

const DEFAULT_LIMIT: u32 = 64;

/// Top-K symbols whose canonical name contains the query (case-insensitive)
/// and whose kind matches `input.kind` when provided.
#[must_use]
pub fn handle(cat: &Catalog, input: &ListSymbolsInput) -> Vec<SymbolSummary> {
    let limit = usize::try_from(input.limit.unwrap_or(DEFAULT_LIMIT).max(1)).unwrap_or(usize::MAX);
    let needle = input.query.to_lowercase();
    let kind_filter = input.kind.as_deref();
    let mut out = Vec::with_capacity(limit.min(64));
    for (id, meta) in &cat.symbols {
        if !needle.is_empty() && !meta.name.to_lowercase().contains(&needle) {
            continue;
        }
        if let Some(want_kind) = kind_filter {
            if meta.kind != want_kind {
                continue;
            }
        }
        out.push(summarize(cat, *id));
        if out.len() >= limit {
            break;
        }
    }
    out
}
