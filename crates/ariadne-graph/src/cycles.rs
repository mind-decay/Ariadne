//! Cycle detection via Tarjan SCC. Filters down to components of size
//! ≥2 — the architecturally interesting ones
//! [src: <https://docs.rs/petgraph/latest/petgraph/algo/fn.tarjan_scc.html>].

use ariadne_core::SymbolId;
use petgraph::algo::tarjan_scc;

use crate::build::GraphIndex;

/// Single strongly connected component returned by [`GraphIndex::cycle_report`].
#[derive(Debug, Clone, Default)]
pub struct Cycle {
    /// Sorted symbols participating in the cycle.
    pub members: Vec<SymbolId>,
}

/// All non-trivial SCCs in the graph.
#[derive(Debug, Clone, Default)]
pub struct CycleReport {
    /// SCCs of size ≥2, sorted internally by symbol id.
    pub cycles: Vec<Cycle>,
}

impl GraphIndex {
    /// Tarjan SCC over the full graph; keeps components of size ≥2.
    #[must_use]
    pub fn cycle_report(&self) -> CycleReport {
        let mut cycles: Vec<Cycle> = tarjan_scc(&self.graph)
            .into_iter()
            .filter(|scc| scc.len() >= 2)
            .map(|scc| {
                let mut members: Vec<SymbolId> = scc.into_iter().map(|ix| self.graph[ix]).collect();
                members.sort();
                Cycle { members }
            })
            .collect();
        cycles.sort_by(|a, b| a.members.cmp(&b.members));
        CycleReport { cycles }
    }
}
