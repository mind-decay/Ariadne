//! Shared scoring + lookup helpers for the `docgen` and `refactor`
//! modules. Two concerns:
//! - [`SymbolTable`] — a snapshot-derived `SymbolId`/`FileId` → metadata
//!   lookup, materialised once per doc/refactor call so the renderers can
//!   resolve names without holding a live storage borrow.
//! - Pure scoring proxies — module cohesion and cycle-cut ranking.
//!
//! Cohesion proxy: `intra_edges / (intra_edges + cross_edges)`. Chosen
//! over LCOM4 deliberately — LCOM4 needs per-method field-access sets the
//! tier-05 SCIP ingest does not record, whereas the edge-ratio proxy is
//! computable from the graph alone and stays stable under incremental
//! edits [src: tier-09 step 2;
//! <https://en.wikipedia.org/wiki/Cohesion_(computer_science)>].

use std::collections::{BTreeMap, BTreeSet};

use ariadne_core::{FileId, ReadSnapshot, SymbolId, SymbolRecord};
use fxhash::FxHashMap;
use petgraph::Direction::{Incoming, Outgoing};
use petgraph::stable_graph::NodeIndex;
use petgraph::visit::EdgeRef;

use crate::build::GraphIndex;
use crate::coupling::ModuleSpec;
use crate::errors::GraphError;

/// Chunk size for the streaming snapshot scans (mirrors `build.rs`).
const SCAN_CHUNK: usize = 4096;

/// Snapshot-derived symbol + file metadata lookup. Built once per
/// docgen/refactor call by draining the `ReadSnapshot` streams into
/// ordered maps — iteration order is therefore `SymbolId` / `FileId`
/// order and never leaks hashmap non-determinism into rendered output.
#[derive(Debug)]
pub(crate) struct SymbolTable {
    symbols: BTreeMap<SymbolId, SymbolRecord>,
    files: BTreeMap<FileId, String>,
}

impl SymbolTable {
    /// Materialise the table from a read snapshot.
    pub(crate) fn from_snapshot(snap: &dyn ReadSnapshot) -> Result<Self, GraphError> {
        let mut symbols = BTreeMap::new();
        for chunk in snap.iter_symbols(SCAN_CHUNK).map_err(GraphError::Storage)? {
            for (id, rec) in chunk.map_err(GraphError::Storage)? {
                symbols.insert(id, rec);
            }
        }
        let mut files = BTreeMap::new();
        for chunk in snap.iter_files(SCAN_CHUNK).map_err(GraphError::Storage)? {
            for (id, rec) in chunk.map_err(GraphError::Storage)? {
                files.insert(id, rec.path);
            }
        }
        Ok(Self { symbols, files })
    }

    /// Canonical name of `id`, or `<unknown>` when absent from the snapshot.
    pub(crate) fn name(&self, id: SymbolId) -> &str {
        self.symbols
            .get(&id)
            .map_or("<unknown>", |r| r.canonical_name.as_str())
    }

    /// Free-form kind tag of `id`, or the empty string when absent.
    pub(crate) fn kind(&self, id: SymbolId) -> &str {
        self.symbols.get(&id).map_or("", |r| r.kind.as_str())
    }

    /// Defining-file path of `id`, or the empty string when absent.
    pub(crate) fn path(&self, id: SymbolId) -> &str {
        self.symbols
            .get(&id)
            .and_then(|r| self.files.get(&r.defining_file))
            .map_or("", String::as_str)
    }

    /// Borrow every `SymbolId → SymbolRecord` pair in [`SymbolId`] order. Used
    /// by the project-doc insight helpers to fold per-file complexity and map
    /// files to their symbols without a second snapshot scan.
    pub(crate) fn symbols(&self) -> &BTreeMap<SymbolId, SymbolRecord> {
        &self.symbols
    }

    /// Iterate every known file path in [`FileId`] order.
    pub(crate) fn file_paths(&self) -> impl Iterator<Item = &str> {
        self.files.values().map(String::as_str)
    }
}

/// Ratio of two counts in f32, computed in f64 so the denominator
/// survives counts past `u16::MAX`. Zero denominator yields 0.
#[allow(clippy::cast_possible_truncation)]
pub(crate) fn ratio(num: u32, den: u32) -> f32 {
    if den == 0 {
        return 0.0;
    }
    (f64::from(num) / f64::from(den)) as f32
}

/// Cohesion proxy of `module`: `intra / (intra + cross)` where `intra`
/// counts edges with both endpoints inside the module and `cross` counts
/// edges with exactly one endpoint inside. An isolated module (no
/// incident edges) scores 0.
pub(crate) fn cohesion(g: &GraphIndex, module: &ModuleSpec) -> f32 {
    let members: BTreeSet<NodeIndex> = module
        .members
        .iter()
        .filter_map(|s| g.index.get(s).copied())
        .collect();
    let mut intra = 0u32;
    let mut cross = 0u32;
    for &ix in &members {
        // Outgoing: an edge to a member is intra (counted once, here);
        // an edge to a non-member is a crossing edge.
        for er in g.graph.edges_directed(ix, Outgoing) {
            if members.contains(&er.target()) {
                intra += 1;
            } else {
                cross += 1;
            }
        }
        // Incoming from a non-member is the *other* half of the crossing
        // edges; incoming from a member was already counted as that
        // member's outgoing intra edge.
        for er in g.graph.edges_directed(ix, Incoming) {
            if !members.contains(&er.source()) {
                cross += 1;
            }
        }
    }
    ratio(intra, intra + cross)
}

/// Map every graph node that belongs to a module to that module's index
/// in `modules`. Symbols claimed by multiple specs resolve to the last
/// claiming module; callers pass disjoint specs.
pub(crate) fn member_index(g: &GraphIndex, modules: &[ModuleSpec]) -> FxHashMap<NodeIndex, usize> {
    let mut out = FxHashMap::default();
    for (mid, m) in modules.iter().enumerate() {
        for s in &m.members {
            if let Some(&ix) = g.index.get(s) {
                out.insert(ix, mid);
            }
        }
    }
    out
}

/// Cycle-cut ranking score for the edge `src -> dst`:
/// `1 / max(fan_in(src), fan_out(dst))`. A higher score marks a
/// lower-traffic edge — the cheapest one to invert or remove. The `max`
/// is clamped to 1 so the score stays in `(0, 1]`.
#[allow(clippy::cast_precision_loss)]
pub(crate) fn cut_score(fan_in_src: usize, fan_out_dst: usize) -> f32 {
    let m = fan_in_src.max(fan_out_dst).max(1);
    1.0_f32 / m as f32
}
