//! Documentation queries: `doc_for`, `doc_for_module`, `doc_for_project`.
//!
//! `doc_for_module` / `doc_for_project` render Markdown via the
//! `ariadne-graph` `docgen` use case, which re-scans symbols/files through
//! the `ReadSnapshot` it is handed — here the warm in-RAM snapshot mirror,
//! so the render never cold-reads redb.

use std::collections::{BTreeMap, HashSet};

use ariadne_core::{DaemonResponse, DocForReport, DocReport};
use ariadne_graph::{DocScope, EdgeKindSet, file_risk, symbol_role};

use crate::domain::catalog::WarmCatalog;
use crate::domain::dispatch::summarize;
use crate::domain::queries::health::build_modules;

const MAX_PUBLIC_REFS: usize = 16;

/// Reverse-BFS hop limit for the `doc_for` blast summary — the `blast_radius`
/// tool default, mirrored on the cold MCP handler so the counts stay equal
/// (parity). `may_touch` is structurally empty at depth 1; depth 3 lets it
/// carry the transitive callers `blast_may` reports.
const DOC_BLAST_DEPTH: u8 = 3;

/// Structured doc summary for `symbol`. Mirrors the cold MCP `doc_for` handler
/// field-for-field, including the tier-05 enrichment, so the warm and cold
/// reports are byte-equal for the same symbol (parity).
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
    let role = symbol_role(&kind, &file);
    let file_risk = file_risk(&file, &cat.churn, &file_complexity(cat));

    // Reverse blast radius at the canonical doc depth (3): `must_touch` are the
    // funnel (immediate-dominator) callers, `may_touch` the other callers within
    // the depth — empty at depth 1, so `blast_may` only carries signal here.
    let radius = cat
        .graph
        .blast_radius(id, DOC_BLAST_DEPTH, EdgeKindSet::ALL)
        .unwrap_or_default();
    // Blast counts reflect the unfiltered radius — scoping is doc-layer only.
    let blast_must = u32::try_from(radius.must_touch.len()).unwrap_or(u32::MAX);
    let blast_may = u32::try_from(radius.may_touch.len()).unwrap_or(u32::MAX);
    let scope = DocScope::default();
    // `public_refs` lists the must-touch (funnel) callers only — the direct,
    // most-relevant neighbours; the transitive `may_touch` set stays a count.
    let mut public_refs: Vec<_> = radius
        .must_touch
        .into_iter()
        .filter(|s| {
            cat.file_of(*s)
                .and_then(|f| cat.path_of(f))
                .is_some_and(|p| scope.include(p))
        })
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
        role,
        file_risk,
        blast_must,
        blast_may,
    })
}

/// Per-file complexity for the risk metric: each churn-set file's symbols'
/// `McCabe` complexity summed, keyed by defining-file path. Scoped to the churn
/// set because `file_risk` ranks only churn files — an entry for a file absent
/// from `cat.churn` is never read, so building it is dead allocation. Built from
/// the catalog the cold MCP path also carries, so `file_risk` is identical on
/// either route (parity) [src: audit/tier-05-report.md F2 — scope the build].
fn file_complexity(cat: &WarmCatalog) -> BTreeMap<String, u32> {
    let churn_paths: HashSet<&str> = cat.churn.iter().map(|c| c.path.as_str()).collect();
    let mut map: BTreeMap<String, u32> = BTreeMap::new();
    for meta in cat.symbols.values() {
        if let Some(path) = cat.path_of(meta.file).filter(|p| churn_paths.contains(*p)) {
            *map.entry(path.to_owned()).or_insert(0) += meta.complexity;
        }
    }
    map
}

/// Markdown documentation for the module at `path` (file = module identity).
pub(crate) fn doc_for_module(cat: &WarmCatalog, path: &str) -> DaemonResponse {
    let modules = build_modules(cat, None);
    let Some(module) = modules.iter().find(|m| m.name == path) else {
        return DaemonResponse::Error(format!("module {path} not found"));
    };
    match ariadne_graph::docgen::for_module(
        &cat.graph,
        &cat.snap,
        module,
        &cat.churn,
        &DocScope::default(),
    ) {
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
