//! Dead-code detector. Reports symbols with `fan_in == 0` that are not
//! marked as exported or as known entry points
//! [src: tier-07 step 8 + plan.md D11].

use std::collections::BTreeSet;

use ariadne_core::SymbolId;
use petgraph::Direction::Incoming;

use crate::build::GraphIndex;

/// Caller-supplied policy hooks. A symbol with `fan_in == 0` is
/// reported as dead unless it appears in any of the lists below.
#[derive(Debug, Clone, Default)]
pub struct DeadCodeConfig {
    /// Symbols whose `kind` indicates an entry point (`main`, binary
    /// crate entries, exported library APIs). Caller resolves the
    /// taxonomy.
    pub entry_points: BTreeSet<SymbolId>,
    /// Symbols re-exported from the project's public API.
    pub exported: BTreeSet<SymbolId>,
    /// Symbols belonging to test harnesses; skipped wholesale.
    pub tests: BTreeSet<SymbolId>,
    /// Per-language root set computed by [`crate::roots::is_root`] over
    /// the caller's [`ariadne_core::SymbolRecord`] metadata. Consulted
    /// before the fan-in=0 test, so `main`/exported/`#[test]` symbols
    /// do not surface as dead code (tier-05 RD4).
    pub roots: BTreeSet<SymbolId>,
}

/// One row of [`DeadCodeReport`].
#[derive(Debug, Clone)]
pub struct DeadSymbol {
    /// Symbol id with no incoming edges.
    pub id: SymbolId,
    /// Short human reason — "no callers, no exports".
    pub reason: &'static str,
}

/// All symbols reported as dead, sorted by id.
#[derive(Debug, Clone, Default)]
pub struct DeadCodeReport {
    /// Dead symbols.
    pub symbols: Vec<DeadSymbol>,
}

impl GraphIndex {
    /// Symbols with zero incoming edges, minus configured exemptions.
    #[must_use]
    pub fn dead_code(&self, cfg: &DeadCodeConfig) -> DeadCodeReport {
        let mut out = Vec::new();
        for &id in self.index.keys() {
            if cfg.entry_points.contains(&id)
                || cfg.exported.contains(&id)
                || cfg.tests.contains(&id)
                || cfg.roots.contains(&id)
            {
                continue;
            }
            let Some(&ix) = self.index.get(&id) else {
                continue;
            };
            if self.graph.edges_directed(ix, Incoming).next().is_none() {
                out.push(DeadSymbol {
                    id,
                    reason: "no callers, no exports",
                });
            }
        }
        out.sort_by_key(|d| d.id);
        DeadCodeReport { symbols: out }
    }
}
