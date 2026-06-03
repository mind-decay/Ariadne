//! Documentation queries: `doc_for`, `doc_for_module`, `doc_for_project`.
//!
//! `doc_for_module` / `doc_for_project` render Markdown via the
//! `ariadne-graph` `docgen` use case, which re-scans symbols/files through
//! the `ReadSnapshot` it is handed — here the warm in-RAM snapshot mirror,
//! so the render never cold-reads redb.

use ariadne_core::{DaemonResponse, DocForReport, DocReport};
use ariadne_graph::{DocScope, EdgeKindSet};

use crate::domain::catalog::WarmCatalog;
use crate::domain::dispatch::summarize;
use crate::domain::queries::health::build_modules;

const MAX_PUBLIC_REFS: usize = 16;

/// Structured doc summary for `symbol`.
pub(crate) fn doc_for(cat: &WarmCatalog, symbol: &str) -> DaemonResponse {
    let Some(id) = cat.find_symbol(symbol) else {
        return DaemonResponse::Error(format!("symbol {symbol} not found"));
    };
    let Some(meta) = cat.meta_of(id) else {
        return DaemonResponse::Error(format!("symbol meta {symbol} not found"));
    };
    let file = cat.path_of(meta.file).unwrap_or("").to_owned();
    let signature = format!("{} {}", meta.kind, meta.name);
    let brief = meta.name.clone();
    let kind = meta.kind.clone();

    let radius = cat
        .graph
        .blast_radius(id, 1, EdgeKindSet::ALL)
        .unwrap_or_default();
    let mut public_refs: Vec<_> = radius
        .must_touch
        .into_iter()
        .chain(radius.may_touch)
        .take(MAX_PUBLIC_REFS)
        .map(|s| summarize(cat, s))
        .collect();
    public_refs.sort_by_key(|a| a.id);

    DaemonResponse::DocFor(DocForReport {
        signature,
        kind,
        file,
        brief,
        public_refs,
    })
}

/// Markdown documentation for the module at `path` (file = module identity).
pub(crate) fn doc_for_module(cat: &WarmCatalog, path: &str) -> DaemonResponse {
    let modules = build_modules(cat, None);
    let Some(module) = modules.iter().find(|m| m.name == path) else {
        return DaemonResponse::Error(format!("module {path} not found"));
    };
    match ariadne_graph::docgen::for_module(&cat.graph, &cat.snap, module, &DocScope::default()) {
        Ok(markdown) => DaemonResponse::Doc(DocReport { markdown }),
        Err(err) => DaemonResponse::Error(err.to_string()),
    }
}

/// Markdown architecture overview for the project, scoped by `prefix`.
pub(crate) fn doc_for_project(cat: &WarmCatalog, prefix: Option<&str>) -> DaemonResponse {
    let modules = build_modules(cat, prefix);
    match ariadne_graph::docgen::for_project(
        &cat.graph,
        &cat.snap,
        &modules,
        &cat.churn,
        &cat.co_change,
        &DocScope::default(),
    ) {
        Ok(markdown) => DaemonResponse::Doc(DocReport { markdown }),
        Err(err) => DaemonResponse::Error(err.to_string()),
    }
}
