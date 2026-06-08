//! Navigation queries: `list_symbols`, `find_definition`, `find_references`.

use std::cmp::Ordering;
use std::collections::BTreeMap;

use ariadne_core::{DaemonResponse, ReadSnapshot, ReferenceSite, ReferencesReport, Verbosity};
use ariadne_graph::economy::{self, Budget, Verbosity as EconVerbosity};

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

/// Reference sites whose target is `symbol`, one row per distinct caller (last
/// edge wins for the span), capped to one page. Mirrors the cold
/// `tools::find_references` handler shape — same dedup, stable sort, cursor,
/// and concise projection via `ariadne_graph::economy` — so the JSON is
/// byte-identical (parity) [src: data-fidelity-arc/block-1 D1-D5].
pub(crate) fn find_references(
    cat: &WarmCatalog,
    symbol: &str,
    limit: Option<u32>,
    cursor: Option<&str>,
    verbosity: Verbosity,
) -> DaemonResponse {
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
                caller: Some(key.src.get()),
                caller_name,
                file,
                byte_start: Some(rec.source_span.byte_start),
                byte_end: Some(rec.source_span.byte_end),
            },
        );
    }
    let rows: Vec<ReferenceSite> = by_caller.into_values().collect();
    // Stamp the catalog revision as u32 (D2), computed identically to the cold
    // path so the cursor is parity-stable; a revision that never realistically
    // exceeds u32 saturates rather than wrapping.
    let revision = u32::try_from(cat.revision).unwrap_or(u32::MAX);
    let decoded = match cursor
        .map(|c| economy::Cursor::decode(c, revision))
        .transpose()
    {
        Ok(c) => c,
        Err(err) => return DaemonResponse::Error(err.to_string()),
    };
    DaemonResponse::References(references_page(rows, decoded, limit, verbosity, revision))
}

/// Map the protocol verbosity onto the economy use case's verbosity.
fn to_economy(v: Verbosity) -> EconVerbosity {
    match v {
        Verbosity::Concise => EconVerbosity::Concise,
        Verbosity::Detailed => EconVerbosity::Detailed,
    }
}

/// Stable order for a reference page: by file, then byte offset, then caller
/// name (identical to the cold handler — keeps the paths byte-identical, D4).
fn cmp_site(a: &ReferenceSite, b: &ReferenceSite) -> Ordering {
    a.file
        .cmp(&b.file)
        .then(a.byte_start.cmp(&b.byte_start))
        .then(a.caller_name.cmp(&b.caller_name))
}

/// Sort, cap, project, and steer one page — the warm twin of the cold
/// `tools::find_references::page`.
fn references_page(
    rows: Vec<ReferenceSite>,
    cursor: Option<economy::Cursor>,
    limit: Option<u32>,
    verbosity: Verbosity,
    revision: u32,
) -> ReferencesReport {
    let econ = to_economy(verbosity);
    let budget = Budget {
        limit: limit.map_or(economy::DEFAULT_PAGE, |l| l as usize),
        cursor,
        verbosity: econ,
    };
    let total = rows.len();
    let paged = economy::paginate(rows, cmp_site, &budget, revision, 0);
    let references: Vec<ReferenceSite> = paged
        .rows
        .into_iter()
        .map(|mut site| {
            if matches!(econ, EconVerbosity::Concise) {
                site.caller = None;
                site.byte_start = None;
                site.byte_end = None;
            }
            site
        })
        .collect();
    let note = paged
        .next_cursor
        .as_ref()
        .map(|_| economy::truncation_note(references.len(), total, "references"));
    ReferencesReport {
        references,
        next_cursor: paged.next_cursor,
        note,
    }
}
