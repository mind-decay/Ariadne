//! Architecture-health queries: `coupling_report`, `weak_spots`.

use std::collections::{BTreeMap, BTreeSet};

use ariadne_core::{
    CouplingReport, CouplingRow, CycleRow, DaemonResponse, FileId, SymbolId, WeakSpotsReport,
};
use ariadne_graph::{CouplingMetrics, DeadCodeConfig, ModuleSpec, roots::is_root};

use crate::domain::catalog::WarmCatalog;
use crate::domain::dispatch::summarize;

/// Efferent-coupling threshold above which a library file is a god module
/// (matches the v1 MCP tuning) [src: tier-14 step 8 dogfood].
const GOD_THRESHOLD: u32 = 15;
const MAX_DEAD: usize = 16;

/// Project catalog symbols into one [`ModuleSpec`] per file, gated by an
/// optional path prefix.
pub(crate) fn build_modules(cat: &WarmCatalog, prefix: Option<&str>) -> Vec<ModuleSpec> {
    let mut by_file: BTreeMap<FileId, BTreeSet<SymbolId>> = BTreeMap::new();
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

fn to_row(m: &CouplingMetrics) -> CouplingRow {
    CouplingRow {
        module: m.name.clone(),
        afferent: m.afferent,
        efferent: m.efferent,
        instability: m.instability,
        abstractness: m.abstractness,
        distance: m.distance,
    }
}

/// Per-file Martin coupling metrics filtered by `prefix`.
pub(crate) fn coupling_report(cat: &WarmCatalog, prefix: Option<&str>) -> DaemonResponse {
    let modules = build_modules(cat, prefix);
    let report = cat.graph.coupling_report(&modules);
    DaemonResponse::Coupling(CouplingReport {
        rows: report.rows.iter().map(to_row).collect(),
    })
}

/// Whether `path` names a Cargo *library* compilation target — not an
/// integration test, bench, example, or build script. God-module detection
/// flags only library files (matches the v1 MCP exclusion).
fn is_library_target(path: &str) -> bool {
    if path.rsplit('/').next() == Some("build.rs") {
        return false;
    }
    !path
        .split('/')
        .any(|c| matches!(c, "tests" | "benches" | "examples"))
}

/// Cycles ∪ god modules ∪ dead-code candidates, filtered by `prefix`. The
/// dead-code pass excludes the per-language root set so `main`, exported
/// API, and test functions do not surface (tier-05 RD4).
pub(crate) fn weak_spots(cat: &WarmCatalog, prefix: Option<&str>) -> DaemonResponse {
    let modules = build_modules(cat, prefix);
    let coupling = cat.graph.coupling_report(&modules);
    let god_modules = coupling
        .rows
        .into_iter()
        .filter(|m| is_library_target(&m.name) && m.efferent > GOD_THRESHOLD)
        .map(|m| CouplingRow {
            module: m.name,
            afferent: m.afferent,
            efferent: m.efferent,
            instability: m.instability,
            abstractness: m.abstractness,
            distance: m.distance,
        })
        .collect();

    let cycles = cat
        .graph
        .cycle_report()
        .cycles
        .into_iter()
        .map(|cycle| {
            let mut members: Vec<String> = cycle
                .members
                .into_iter()
                .filter_map(|sid| cat.meta_of(sid).map(|m| m.name.clone()))
                .collect();
            members.sort();
            CycleRow { members }
        })
        .collect();

    let mut roots: BTreeSet<SymbolId> = BTreeSet::new();
    for (id, meta) in &cat.symbols {
        if is_root(
            meta.lang,
            meta.visibility,
            &meta.attributes,
            &meta.kind,
            &meta.name,
        ) {
            roots.insert(*id);
        }
    }
    let cfg = DeadCodeConfig {
        roots,
        ..Default::default()
    };
    let dead_symbols = cat
        .graph
        .dead_code(&cfg)
        .symbols
        .into_iter()
        .take(MAX_DEAD)
        .map(|d| summarize(cat, d.id))
        .collect();

    DaemonResponse::WeakSpots(WeakSpotsReport {
        cycles,
        god_modules,
        dead_symbols,
    })
}
