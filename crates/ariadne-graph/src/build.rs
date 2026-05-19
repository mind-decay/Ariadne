//! Central `GraphIndex` data structure plus the build / mutate / delta
//! surface. Other analytics modules (`blast`, `coupling`, `cycles`,
//! `dead`, `plan_assist`) extend `GraphIndex` via inherent `impl` blocks.
//!
//! Storage choice: `StableDiGraph` so node indices survive removals —
//! required for the incremental delta path (`apply_delta`)
//! [src: <https://docs.rs/petgraph/latest/petgraph/stable_graph/struct.StableGraph.html>].

use ariadne_core::{EdgeKind as CoreEdgeKind, ReadSnapshot, SymbolId};
use bitflags::bitflags;
use fxhash::FxHashMap;
use petgraph::stable_graph::{NodeIndex, StableDiGraph};
use petgraph::visit::EdgeRef;
use rayon::prelude::*;

use crate::errors::GraphError;

/// Chunk size for streaming reads out of `ReadSnapshot`. 4096 records per
/// chunk keeps each in-flight batch under ~1 MB at our record sizes and
/// stays comfortably below the 100 K-file / 10 M-LOC working-set ceiling
/// declared in plan.md `<constraints>`.
const SCAN_CHUNK: usize = 4096;

/// Edge classification used by the in-RAM graph. Wider than the storage
/// `EdgeKind` (3 variants) — tier-07 surfaces the categories analytics
/// callers want to filter by [src: tier-07 step 2]. Storage's smaller
/// alphabet maps in via [`EdgeKind::from_core`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(u8)]
pub enum EdgeKind {
    /// Function/method call site.
    Calls = 0,
    /// Module/symbol import.
    Imports = 1,
    /// Type annotation / type usage.
    TypeOf = 2,
    /// Definition site → defined symbol.
    Defines = 3,
    /// Method override / trait impl edge.
    Overrides = 4,
    /// Read access on a binding.
    Reads = 5,
    /// Write access on a binding.
    Writes = 6,
    /// Class/trait/interface inheritance.
    Inherits = 7,
}

impl EdgeKind {
    /// Single-flag subset used for filter matching.
    #[must_use]
    pub fn to_flag(self) -> EdgeKindSet {
        match self {
            Self::Calls => EdgeKindSet::CALLS,
            Self::Imports => EdgeKindSet::IMPORTS,
            Self::TypeOf => EdgeKindSet::TYPE_OF,
            Self::Defines => EdgeKindSet::DEFINES,
            Self::Overrides => EdgeKindSet::OVERRIDES,
            Self::Reads => EdgeKindSet::READS,
            Self::Writes => EdgeKindSet::WRITES,
            Self::Inherits => EdgeKindSet::INHERITS,
        }
    }

    /// Map the storage `EdgeKind` (3 variants) into the graph alphabet.
    /// `References` lands on `Calls` because that is the dominant
    /// reference category until tier-08+ SCIP refinement; finer
    /// classification arrives when storage adds subkinds.
    #[must_use]
    pub fn from_core(kind: CoreEdgeKind) -> Self {
        match kind {
            CoreEdgeKind::Defines => Self::Defines,
            CoreEdgeKind::Imports => Self::Imports,
            // `CoreEdgeKind` is `#[non_exhaustive]`; `References` plus
            // any future variant collapses to `Calls` until the graph
            // alphabet extends.
            _ => Self::Calls,
        }
    }
}

bitflags! {
    /// Filter set over [`EdgeKind`]. Used by `blast_radius`, `plan_assist`,
    /// and any future per-kind report.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct EdgeKindSet: u16 {
        /// Call edges.
        const CALLS     = 1 << 0;
        /// Import edges.
        const IMPORTS   = 1 << 1;
        /// Type-of edges.
        const TYPE_OF   = 1 << 2;
        /// Definition edges.
        const DEFINES   = 1 << 3;
        /// Override edges.
        const OVERRIDES = 1 << 4;
        /// Read edges.
        const READS     = 1 << 5;
        /// Write edges.
        const WRITES    = 1 << 6;
        /// Inheritance edges.
        const INHERITS  = 1 << 7;
        /// Convenience union of all kinds.
        const ALL = Self::CALLS.bits() | Self::IMPORTS.bits() | Self::TYPE_OF.bits()
            | Self::DEFINES.bits() | Self::OVERRIDES.bits() | Self::READS.bits()
            | Self::WRITES.bits() | Self::INHERITS.bits();
    }
}

/// Edge body stored inside the petgraph adjacency list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EdgeMeta {
    /// Kind tag — used for filter matching during traversal.
    pub kind: EdgeKind,
    /// Coupling weight (carried over from `EdgeRecord::weight`).
    pub weight: u32,
}

/// Incremental edge delta passed to [`GraphIndex::apply_delta`].
#[derive(Debug, Default, Clone)]
pub struct EdgeDelta {
    /// Edges to add. Tuple form `(src, dst, kind, weight)`.
    pub added: Vec<(SymbolId, SymbolId, EdgeKind, u32)>,
    /// Edges to remove (matched by `(src, dst, kind)`).
    pub removed: Vec<(SymbolId, SymbolId, EdgeKind)>,
}

/// In-RAM graph of symbols + typed edges. The petgraph `StableDiGraph`
/// keeps node indices alive across removals so the
/// `SymbolId → NodeIndex` map only needs touching on symbol churn.
#[derive(Debug, Default)]
pub struct GraphIndex {
    pub(crate) graph: StableDiGraph<SymbolId, EdgeMeta>,
    pub(crate) index: FxHashMap<SymbolId, NodeIndex>,
}

impl GraphIndex {
    /// Construct an empty graph.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Symbol count.
    #[must_use]
    pub fn symbol_count(&self) -> usize {
        self.index.len()
    }

    /// Edge count.
    #[must_use]
    pub fn edge_count(&self) -> usize {
        self.graph.edge_count()
    }

    /// Insert (or look up) a symbol; idempotent.
    pub fn add_symbol(&mut self, id: SymbolId) -> NodeIndex {
        if let Some(ix) = self.index.get(&id).copied() {
            return ix;
        }
        let ix = self.graph.add_node(id);
        self.index.insert(id, ix);
        ix
    }

    /// Add a typed edge. Auto-inserts endpoints. Returns `false` if the
    /// edge was a duplicate of an existing `(src, dst, kind)`.
    pub fn add_edge(&mut self, src: SymbolId, dst: SymbolId, kind: EdgeKind) -> bool {
        self.add_edge_weighted(src, dst, kind, 1)
    }

    /// Same as `add_edge` but with an explicit weight (from `EdgeRecord`).
    pub fn add_edge_weighted(
        &mut self,
        src: SymbolId,
        dst: SymbolId,
        kind: EdgeKind,
        weight: u32,
    ) -> bool {
        let s = self.add_symbol(src);
        let d = self.add_symbol(dst);
        if self.find_edge(s, d, kind).is_some() {
            return false;
        }
        self.graph.add_edge(s, d, EdgeMeta { kind, weight });
        true
    }

    pub(crate) fn find_edge(
        &self,
        s: NodeIndex,
        d: NodeIndex,
        kind: EdgeKind,
    ) -> Option<petgraph::stable_graph::EdgeIndex> {
        self.graph
            .edges_connecting(s, d)
            .find(|er| er.weight().kind == kind)
            .map(|er| er.id())
    }

    /// Build a `GraphIndex` from a [`ReadSnapshot`]. Streams symbols
    /// then edges in 4096-record chunks (constant working-set memory
    /// regardless of repo size). Inside each edge chunk, rayon resolves
    /// the per-row `(src, dst)` → `NodeIndex` lookups in parallel
    /// against the already-built symbol index, after which the
    /// sequential merge `add_edge_weighted` runs on the main thread to
    /// preserve `StableDiGraph` invariants. Order-insensitive because
    /// petgraph adds are commutative for distinct `(src, dst, kind)`
    /// tuples [src: tier-07 step 4].
    ///
    /// # Errors
    /// Propagates [`ariadne_core::StorageError`] from the underlying
    /// snapshot scans (wrapped in [`GraphError::Storage`]).
    pub fn build_from_snapshot(snap: &dyn ReadSnapshot) -> Result<Self, GraphError> {
        let mut out = Self::new();
        let symbol_chunks = snap.iter_symbols(SCAN_CHUNK).map_err(GraphError::Storage)?;
        for chunk in symbol_chunks {
            for (id, _rec) in chunk.map_err(GraphError::Storage)? {
                out.add_symbol(id);
            }
        }
        let edge_chunks = snap.iter_edges(SCAN_CHUNK).map_err(GraphError::Storage)?;
        for chunk in edge_chunks {
            let edges = chunk.map_err(GraphError::Storage)?;
            // Per-chunk parallel projection: every record reshaped into
            // the petgraph alphabet (`EdgeKind::from_core`, weight) in
            // parallel against an immutable chunk slice. The sequential
            // merge that follows preserves `StableDiGraph` invariants
            // and the auto-create / dedup behaviour of
            // `add_edge_weighted`.
            let resolved: Vec<(SymbolId, SymbolId, EdgeKind, u32)> = edges
                .par_iter()
                .map(|(key, rec)| (key.src, key.dst, EdgeKind::from_core(key.kind), rec.weight))
                .collect();
            for (s, d, kind, w) in resolved {
                out.add_edge_weighted(s, d, kind, w);
            }
        }
        Ok(out)
    }

    /// Apply an incremental delta. Symbol-set churn (`added`/`removed`)
    /// is passed alongside the edge delta so dropped symbols also
    /// surrender their `NodeIndex`. `StableDiGraph` guarantees other
    /// indices survive
    /// [src: <https://docs.rs/petgraph/latest/petgraph/stable_graph/struct.StableGraph.html>].
    pub fn apply_delta(
        &mut self,
        added: Vec<SymbolId>,
        removed: Vec<SymbolId>,
        edge_diff: EdgeDelta,
    ) {
        for id in added {
            self.add_symbol(id);
        }
        for (src, dst, kind, weight) in edge_diff.added {
            self.add_edge_weighted(src, dst, kind, weight);
        }
        for (src, dst, kind) in edge_diff.removed {
            if let (Some(&s), Some(&d)) = (self.index.get(&src), self.index.get(&dst)) {
                if let Some(eix) = self.find_edge(s, d, kind) {
                    self.graph.remove_edge(eix);
                }
            }
        }
        for id in removed {
            if let Some(ix) = self.index.remove(&id) {
                self.graph.remove_node(ix);
            }
        }
    }
}
