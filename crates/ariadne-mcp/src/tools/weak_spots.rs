//! `weak_spots` — cycles ∪ god-modules ∪ dead-code top-N.

use ariadne_graph::DeadCodeConfig;

use crate::catalog::Catalog;
use crate::tools::{coupling_report, summarize};
use crate::types::{CycleRow, ScopeInput, WeakSpotsOutput};

/// Efferent-coupling threshold above which a file-as-module is flagged a
/// god module. Raised from 8 to 15 after the tier-14 dogfood: on
/// Ariadne's own 202-file repo, the `is_library_target` exclusion still
/// left 25 modules flagged (efferent 9–30) — a dense bulk of ordinary
/// Rust modules at 9–15 and a 4-module tail at 17+. 15 cuts at the top
/// of that bulk so the signal is an actionable tail, not noise
/// [src: tier-14 step 8 dogfood].
const GOD_THRESHOLD: u32 = 15;
const MAX_DEAD: usize = 16;

/// Whether `path` names a Cargo *library* compilation target — i.e. not
/// an integration test, bench, or example, and not a build script.
///
/// God-module detection only flags library files: a high efferent count
/// in `tests/`, `benches/`, or `examples/` is the expected shape of a
/// test, bench, or example fanning out to the API under exercise, not an
/// architecture smell. Classification follows Cargo's directory
/// convention [src: <https://doc.rust-lang.org/cargo/guide/project-layout.html>];
/// per-language target classification is future work.
fn is_library_target(path: &str) -> bool {
    if path.rsplit('/').next() == Some("build.rs") {
        return false;
    }
    !path
        .split('/')
        .any(|c| matches!(c, "tests" | "benches" | "examples"))
}

/// Aggregate architecture-risk findings (cycles, god modules, dead code)
/// filtered by `scope.prefix`.
#[must_use]
pub fn handle(cat: &Catalog, scope: &ScopeInput) -> WeakSpotsOutput {
    let modules = coupling_report::build_modules(cat, scope.prefix.as_deref());
    let coupling = cat.graph.coupling_report(&modules);
    let god_modules = coupling
        .rows
        .into_iter()
        .filter(|m| is_library_target(&m.name) && m.efferent > GOD_THRESHOLD)
        .map(|m| crate::types::CouplingRow {
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

    let dead = cat.graph.dead_code(&DeadCodeConfig::default());
    let dead_symbols = dead
        .symbols
        .into_iter()
        .take(MAX_DEAD)
        .map(|d| summarize(cat, d.id))
        .collect();

    WeakSpotsOutput {
        cycles,
        god_modules,
        dead_symbols,
    }
}
