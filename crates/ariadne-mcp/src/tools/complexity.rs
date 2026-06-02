//! `complexity` — `McCabe` cyclomatic complexity ranking at file or symbol grain
//! (tier-15b). No graph use case exists: tier-13 D2 places the file-complexity
//! fold at the composition root, so the handler folds the catalog's per-symbol
//! `complexity` into per-file Σ (file grain) or per-symbol rows (symbol grain)
//! and ranks descending. Logic identical to the daemon
//! `queries::analytics::complexity` so cold and warm JSON match.

use std::collections::BTreeMap;

use crate::catalog::Catalog;
use crate::tools::summarize;
use crate::types::{ComplexityOutput, ComplexityRow, Grain, GrainScopeInput};

/// Whether `path` is in scope for an optional path prefix (`None` = all).
fn in_scope(path: &str, prefix: Option<&str>) -> bool {
    prefix.is_none_or(|p| path.starts_with(p))
}

/// Rank `McCabe` complexity at `input.grain`, filtered by prefix, descending.
#[must_use]
pub fn handle(cat: &Catalog, input: &GrainScopeInput) -> ComplexityOutput {
    let prefix = input.prefix.as_deref();
    let mut rows = match input.grain {
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
    rows.sort_by(|a, b| {
        b.complexity
            .cmp(&a.complexity)
            .then_with(|| key(a).cmp(&key(b)))
    });
    ComplexityOutput { rows }
}

/// Sort key: file path (file grain) or symbol id (symbol grain); breaks
/// complexity ties ascending.
fn key(row: &ComplexityRow) -> (String, u64) {
    (row.file.clone(), row.symbol.as_ref().map_or(0, |s| s.id))
}
