//! `weak_spots` — cycles ∪ god-modules ∪ dead-code top-N.

use ariadne_graph::DeadCodeConfig;

use crate::catalog::Catalog;
use crate::tools::{coupling_report, summarize};
use crate::types::{CycleRow, ScopeInput, WeakSpotsOutput};

const GOD_THRESHOLD: u32 = 8;
const MAX_DEAD: usize = 16;

/// Aggregate architecture-risk findings (cycles, god modules, dead code)
/// filtered by `scope.prefix`.
#[must_use]
pub fn handle(cat: &Catalog, scope: &ScopeInput) -> WeakSpotsOutput {
    let modules = coupling_report::build_modules(cat, scope.prefix.as_deref());
    let coupling = cat.graph.coupling_report(&modules);
    let god_modules = coupling
        .rows
        .into_iter()
        .filter(|m| m.efferent > GOD_THRESHOLD)
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
