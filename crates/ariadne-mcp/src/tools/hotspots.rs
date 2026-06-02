//! `hotspots` — churn × complexity ranking at file or symbol grain (tier-15b).
//!
//! Builds the complexity map the tier-13 use case needs from the catalog's
//! per-symbol `complexity` (file grain → per-file Σ; symbol grain →
//! passthrough), calls `file_hotspots` / `symbol_hotspots`, and projects the
//! result to the wire row shape. Logic identical to the daemon
//! `queries::analytics::hotspots` so cold and warm JSON match
//! [src: crates/ariadne-graph/src/hotspot.rs:102-150].

use std::collections::BTreeMap;

use ariadne_core::SymbolId;
use ariadne_graph::{HotspotGrain, HotspotReport as GraphHotspots, file_hotspots, symbol_hotspots};

use crate::catalog::Catalog;
use crate::tools::summarize;
use crate::types::{Grain, GrainScopeInput, HotspotOutput, HotspotRow};

/// Whether `path` is in scope for an optional path prefix (`None` = all).
fn in_scope(path: &str, prefix: Option<&str>) -> bool {
    prefix.is_none_or(|p| path.starts_with(p))
}

/// Rank churn × complexity hotspots at `input.grain`, filtered by prefix.
#[must_use]
pub fn handle(cat: &Catalog, input: &GrainScopeInput) -> HotspotOutput {
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
    HotspotOutput {
        rows: project(cat, report, prefix),
    }
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
