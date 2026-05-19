//! `coupling_report` — Martin metrics with each file treated as a module.
//!
//! Tier-08 has no first-class module taxonomy; using files keeps the
//! tool useful immediately and matches the per-file-unit boundary the
//! plan adopts from Glean (D12).

use std::collections::BTreeSet;

use ariadne_core::FileId;
use ariadne_graph::{CouplingMetrics, ModuleSpec};

use crate::catalog::Catalog;
use crate::types::{CouplingOutput, CouplingRow, ScopeInput};

/// Compute per-file coupling metrics filtered by `scope.prefix`.
#[must_use]
pub fn handle(cat: &Catalog, scope: &ScopeInput) -> CouplingOutput {
    let modules = build_modules(cat, scope.prefix.as_deref());
    let report = cat.graph.coupling_report(&modules);
    let rows = report.rows.iter().map(to_row).collect();
    CouplingOutput { rows }
}

/// Project `Catalog` symbols into one `ModuleSpec` per file, optionally
/// gated by a path prefix.
#[must_use]
pub fn build_modules(cat: &Catalog, prefix: Option<&str>) -> Vec<ModuleSpec> {
    let mut by_file: std::collections::BTreeMap<FileId, BTreeSet<ariadne_core::SymbolId>> =
        std::collections::BTreeMap::new();
    for (sid, meta) in &cat.symbols {
        by_file.entry(meta.file).or_default().insert(*sid);
    }
    let mut out = Vec::with_capacity(by_file.len());
    for (fid, members) in by_file {
        let Some(path) = cat.path_of(fid) else {
            continue;
        };
        if let Some(p) = prefix {
            if !path.starts_with(p) {
                continue;
            }
        }
        out.push(ModuleSpec {
            name: path.to_owned(),
            members,
            abstract_members: BTreeSet::new(),
        });
    }
    out
}

fn to_row(metrics: &CouplingMetrics) -> CouplingRow {
    CouplingRow {
        module: metrics.name.clone(),
        afferent: metrics.afferent,
        efferent: metrics.efferent,
        instability: metrics.instability,
        abstractness: metrics.abstractness,
        distance: metrics.distance,
    }
}
