//! Module-level coupling metrics: afferent (Ca), efferent (Ce),
//! instability `I = Ce/(Ca+Ce)`, abstractness `A`, and distance from
//! the main sequence `|A + I − 1|`. Classical Martin metrics
//! [src: <https://win.tue.nl/~aserebre/2IS55/2009-2010/10.pdf>].

use std::collections::BTreeSet;

use ariadne_core::SymbolId;
use fxhash::FxHashMap;
use petgraph::Direction::{Incoming, Outgoing};
use petgraph::stable_graph::NodeIndex;

use crate::build::GraphIndex;

/// Ratio of two non-negative counts in f32, computed in f64 to survive
/// counts that overflow u16. Returns 0 when the denominator is zero.
#[allow(clippy::cast_possible_truncation)]
fn ratio(num: u32, den: u32) -> f32 {
    if den == 0 {
        return 0.0;
    }
    (f64::from(num) / f64::from(den)) as f32
}

/// Caller-supplied module description. `abstract_members ⊆ members`;
/// the abstractness ratio is `|abstract| / |members|`.
#[derive(Debug, Clone)]
pub struct ModuleSpec {
    /// Human-readable module name (path stem, crate name, etc.).
    pub name: String,
    /// All symbols that belong to the module.
    pub members: BTreeSet<SymbolId>,
    /// Subset of `members` that are declared abstract / trait / interface.
    pub abstract_members: BTreeSet<SymbolId>,
}

/// One row of [`CouplingReport`].
#[derive(Debug, Clone)]
pub struct CouplingMetrics {
    /// Module name copied from input.
    pub name: String,
    /// Afferent coupling — distinct external symbols pointing in.
    pub afferent: u32,
    /// Efferent coupling — distinct external symbols pointed to.
    pub efferent: u32,
    /// Instability `I = Ce / (Ca + Ce)` ∈ [0, 1]; defined as 0 when
    /// both are zero.
    pub instability: f32,
    /// Abstractness `A = |abstract_members| / |members|`; 0 when the
    /// module is empty.
    pub abstractness: f32,
    /// Distance from the main sequence `|A + I − 1|`.
    pub distance: f32,
}

/// Report of one [`CouplingMetrics`] row per input module, sorted by
/// module name.
#[derive(Debug, Clone, Default)]
pub struct CouplingReport {
    /// Per-module metrics.
    pub rows: Vec<CouplingMetrics>,
}

impl GraphIndex {
    /// Compute Ca/Ce/I/A/distance for each [`ModuleSpec`].
    #[must_use]
    pub fn coupling_report(&self, modules: &[ModuleSpec]) -> CouplingReport {
        let member_of = self.member_index(modules);
        let mut rows = Vec::with_capacity(modules.len());
        for (mid, m) in modules.iter().enumerate() {
            let metrics = self.metrics_for(mid, m, &member_of);
            rows.push(metrics);
        }
        rows.sort_by(|a, b| a.name.cmp(&b.name));
        CouplingReport { rows }
    }

    fn member_index(&self, modules: &[ModuleSpec]) -> FxHashMap<NodeIndex, usize> {
        let mut out = FxHashMap::default();
        for (mid, m) in modules.iter().enumerate() {
            for s in &m.members {
                if let Some(&ix) = self.index.get(s) {
                    out.insert(ix, mid);
                }
            }
        }
        out
    }

    fn metrics_for(
        &self,
        mid: usize,
        m: &ModuleSpec,
        member_of: &FxHashMap<NodeIndex, usize>,
    ) -> CouplingMetrics {
        let mut afferent: BTreeSet<NodeIndex> = BTreeSet::new();
        let mut efferent: BTreeSet<NodeIndex> = BTreeSet::new();
        for s in &m.members {
            let Some(&ix) = self.index.get(s) else {
                continue;
            };
            for er in self.graph.edges_directed(ix, Incoming) {
                let src = petgraph::visit::EdgeRef::source(&er);
                if member_of.get(&src).copied() != Some(mid) {
                    afferent.insert(src);
                }
            }
            for er in self.graph.edges_directed(ix, Outgoing) {
                let dst = petgraph::visit::EdgeRef::target(&er);
                if member_of.get(&dst).copied() != Some(mid) {
                    efferent.insert(dst);
                }
            }
        }
        let ca = u32::try_from(afferent.len()).unwrap_or(u32::MAX);
        let ce = u32::try_from(efferent.len()).unwrap_or(u32::MAX);
        // f64 arithmetic so the denominator survives Ca+Ce ≫ u16::MAX
        // at the plan's 100K-file / 10M-LOC scale; results are ratios in
        // [0, 1] so the final f32 narrowing only loses precision past
        // ~7 decimal digits — acceptable for a presentation metric.
        let instability = ratio(ce, ca + ce);
        let abstractness = if m.members.is_empty() {
            0.0_f32
        } else {
            let num = u32::try_from(m.abstract_members.len()).unwrap_or(u32::MAX);
            let den = u32::try_from(m.members.len()).unwrap_or(u32::MAX);
            ratio(num, den)
        };
        let distance = (abstractness + instability - 1.0).abs();
        CouplingMetrics {
            name: m.name.clone(),
            afferent: ca,
            efferent: ce,
            instability,
            abstractness,
            distance,
        }
    }
}
