//! `doc_for` — structured doc summary for one symbol.

use std::collections::{BTreeMap, HashSet};

use ariadne_graph::{DocScope, EdgeKindSet, file_risk, symbol_role};

use crate::catalog::Catalog;
use crate::errors::McpError;
use crate::tools::summarize;
use crate::types::{DocForOutput, SymbolQuery, SymbolSummary};

const MAX_PUBLIC_REFS: usize = 16;

/// Reverse-BFS hop limit for the `doc_for` blast summary — the `blast_radius`
/// tool default, so `blast_must` / `blast_may` match what `blast_radius` reports
/// and `may_touch` (empty at depth 1) carries transitive callers.
const DOC_BLAST_DEPTH: u8 = 3;

/// Doc-like structured summary for `input.symbol`. Mirrors the warm daemon
/// `doc_for` query field-for-field, including the tier-05 enrichment, so the
/// cold and warm reports are byte-equal for the same symbol (parity).
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
    let role = symbol_role(&meta.kind, &file);
    let file_risk = file_risk(&file, &cat.churn, &file_complexity(cat));

    // Reverse blast radius at the canonical doc depth (3, the `blast_radius`
    // tool default), so `may_touch` carries transitive callers instead of being
    // structurally empty as it is at depth 1. `must_touch` are the callers the
    // symbol funnels through (immediate dominators); `may_touch` are the other
    // callers reachable within the depth. `id` came from the catalog and every
    // catalog symbol is a graph node, so the `Option` is always `Some`;
    // `unwrap_or_default` keeps the empty-radius fallback for the unreachable
    // desync case.
    let radius = cat
        .graph
        .blast_radius(id, DOC_BLAST_DEPTH, EdgeKindSet::ALL)
        .unwrap_or_default();
    // Blast counts reflect the unfiltered radius — scoping is doc-layer only.
    let blast_must = u32::try_from(radius.must_touch.len()).unwrap_or(u32::MAX);
    let blast_may = u32::try_from(radius.may_touch.len()).unwrap_or(u32::MAX);
    let scope = DocScope::default();
    // `public_refs` lists the must-touch (funnel) callers only — the direct,
    // most-relevant neighbours — scope-filtered to source paths. The transitive
    // `may_touch` set stays a count, never diluting the ref list.
    let mut callers: Vec<SymbolSummary> = radius
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
    callers.sort_by_key(|a| a.id);

    Ok(DocForOutput {
        signature,
        kind: meta.kind.clone(),
        file,
        brief,
        public_refs: callers,
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
/// the same catalog fields the warm daemon path carries, so `file_risk` is
/// identical on either route (parity) [src: audit/tier-05-report.md F2].
fn file_complexity(cat: &Catalog) -> BTreeMap<String, u32> {
    let churn_paths: HashSet<&str> = cat.churn.iter().map(|c| c.path.as_str()).collect();
    let mut map: BTreeMap<String, u32> = BTreeMap::new();
    for meta in cat.symbols.values() {
        if let Some(path) = cat.path_of(meta.file).filter(|p| churn_paths.contains(*p)) {
            *map.entry(path.to_owned()).or_insert(0) += meta.complexity;
        }
    }
    map
}
