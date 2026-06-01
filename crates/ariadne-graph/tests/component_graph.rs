//! Component-graph regression guard — tier-09 steps 1-2.
//!
//! A leaf component rendered by two parents (`Renders` edges) plus one
//! `UsesHook` edge. Asserts `blast_radius` reaches the parents and
//! `coupling_report` counts the component edges.
//!
//! Inspection (step 2): the analytics walk edges generically, so the
//! component `EdgeKind`s flow through with no source change —
//! - `EdgeKind::from_core` collapses `Renders`/`UsesHook` (and any future
//!   `#[non_exhaustive]` core variant) onto the graph alphabet's `Calls`
//!   [src: crates/ariadne-graph/src/build.rs:69-79].
//! - `coupling_report` sums `edges_directed` with no kind allow-list
//!   [src: crates/ariadne-graph/src/coupling.rs:102-113].
//! - `blast_radius` reverse-BFS filters by the caller's `EdgeKindSet`;
//!   `CALLS` (in `ALL`) covers the collapsed component edges
//!   [src: crates/ariadne-graph/src/blast.rs:108-117].
//!
//! No algorithm drops the new variants, so this test is green by
//! construction and stands as a regression guard.

use std::collections::BTreeSet;

use ariadne_core::{
    ChunkStream, EdgeKey, EdgeKind as CoreEdgeKind, EdgeRecord, FileId, FileRecord, Lang,
    ReadSnapshot, Span, StorageError, SymbolId, SymbolRecord, Visibility,
};
use ariadne_graph::{EdgeKindSet, GraphIndex, ModuleSpec};

fn fid(n: u32) -> FileId {
    FileId::new(n).expect("nonzero file id")
}

fn sid(n: u64) -> SymbolId {
    SymbolId::new(n).expect("nonzero symbol id")
}

/// `leaf` is rendered by `parent_a` and `parent_b`; `leaf` uses one hook.
const LEAF: u64 = 1;
const PARENT_A: u64 = 2;
const PARENT_B: u64 = 3;
const HOOK: u64 = 4;

/// In-memory `ReadSnapshot` carrying a component graph: four symbols and
/// three component edges (`Renders` ×2, `UsesHook` ×1).
#[derive(Debug)]
struct ComponentSnapshot {
    symbols: Vec<(SymbolId, SymbolRecord)>,
    edges: Vec<(EdgeKey, EdgeRecord)>,
}

fn chunked<T: Clone + 'static>(data: &[T], chunk: usize) -> ChunkStream<'static, T> {
    let chunk = chunk.max(1);
    let parts: Vec<Result<Vec<T>, StorageError>> =
        data.chunks(chunk).map(|c| Ok(c.to_vec())).collect();
    Box::new(parts.into_iter())
}

impl ReadSnapshot for ComponentSnapshot {
    fn file(&self, _: FileId) -> Result<Option<FileRecord>, StorageError> {
        Ok(None)
    }
    fn symbols_in_file(&self, _: FileId) -> Result<Vec<SymbolRecord>, StorageError> {
        Ok(Vec::new())
    }
    fn outgoing_edges(&self, src: SymbolId) -> Result<Vec<(EdgeKey, EdgeRecord)>, StorageError> {
        Ok(self
            .edges
            .iter()
            .filter(|(k, _)| k.src == src)
            .cloned()
            .collect())
    }
    fn incoming_edges(&self, dst: SymbolId) -> Result<Vec<(EdgeKey, EdgeRecord)>, StorageError> {
        Ok(self
            .edges
            .iter()
            .filter(|(k, _)| k.dst == dst)
            .cloned()
            .collect())
    }
    fn edges_in_file(&self, _: FileId) -> Result<Vec<EdgeKey>, StorageError> {
        Ok(Vec::new())
    }
    fn iter_files(&self, _: usize) -> Result<ChunkStream<'_, (FileId, FileRecord)>, StorageError> {
        Ok(Box::new(std::iter::empty()))
    }
    fn iter_symbols(
        &self,
        chunk: usize,
    ) -> Result<ChunkStream<'_, (SymbolId, SymbolRecord)>, StorageError> {
        Ok(chunked(&self.symbols, chunk))
    }
    fn iter_edges(
        &self,
        chunk: usize,
    ) -> Result<ChunkStream<'_, (EdgeKey, EdgeRecord)>, StorageError> {
        Ok(chunked(&self.edges, chunk))
    }
}

/// Build the four-component fixture snapshot.
fn fixture() -> ComponentSnapshot {
    let symbol = |id: u64, name: &str, kind: &str| {
        (
            sid(id),
            SymbolRecord {
                canonical_name: name.to_owned(),
                kind: kind.to_owned(),
                defining_file: fid(1),
                defining_span: Span {
                    file: fid(1),
                    byte_start: 0,
                    byte_end: 0,
                },
                visibility: Visibility::Unknown,
                attributes: Vec::new(),
                complexity: 0,
            },
        )
    };
    let edge = |src: u64, dst: u64, kind: CoreEdgeKind| {
        (
            EdgeKey {
                src: sid(src),
                kind,
                dst: sid(dst),
            },
            EdgeRecord {
                source_span: Span {
                    file: fid(1),
                    byte_start: 0,
                    byte_end: 0,
                },
                evidence_lang: Lang::Tsx,
                weight: 1,
            },
        )
    };
    ComponentSnapshot {
        symbols: vec![
            symbol(LEAF, "Leaf", "component"),
            symbol(PARENT_A, "ParentA", "component"),
            symbol(PARENT_B, "ParentB", "component"),
            symbol(HOOK, "useState", "function"),
        ],
        edges: vec![
            edge(PARENT_A, LEAF, CoreEdgeKind::Renders),
            edge(PARENT_B, LEAF, CoreEdgeKind::Renders),
            edge(LEAF, HOOK, CoreEdgeKind::UsesHook),
        ],
    }
}

fn graph() -> GraphIndex {
    GraphIndex::build_from_snapshot(&fixture()).expect("build graph from component snapshot")
}

#[test]
fn blast_radius_of_leaf_includes_both_rendering_parents() {
    let g = graph();
    let br = g
        .blast_radius(sid(LEAF), 5, EdgeKindSet::ALL)
        .expect("leaf present in graph");
    let reached: BTreeSet<SymbolId> = br
        .must_touch
        .iter()
        .chain(br.may_touch.iter())
        .copied()
        .collect();
    assert_eq!(
        reached,
        BTreeSet::from([sid(PARENT_A), sid(PARENT_B)]),
        "a leaf component's blast radius must include the parents that render it",
    );
}

#[test]
fn blast_radius_traverses_uses_hook_edges() {
    let g = graph();
    // Depth 1: the single direct predecessor across the `UsesHook` edge.
    let br = g
        .blast_radius(sid(HOOK), 1, EdgeKindSet::ALL)
        .expect("hook present in graph");
    let reached: BTreeSet<SymbolId> = br
        .must_touch
        .iter()
        .chain(br.may_touch.iter())
        .copied()
        .collect();
    assert_eq!(
        reached,
        BTreeSet::from([sid(LEAF)]),
        "a hook's blast radius must include the component that uses it",
    );
}

#[test]
fn coupling_report_counts_component_edges() {
    let g = graph();
    let module = |name: &str, ids: &[u64]| ModuleSpec {
        name: name.to_owned(),
        members: ids.iter().map(|&i| sid(i)).collect(),
        abstract_members: BTreeSet::new(),
    };
    let modules = vec![
        module("leaf", &[LEAF]),
        module("parent_a", &[PARENT_A]),
        module("parent_b", &[PARENT_B]),
        module("hook", &[HOOK]),
    ];
    let report = g.coupling_report(&modules);
    let leaf = report
        .rows
        .iter()
        .find(|r| r.name == "leaf")
        .expect("leaf module row present");
    assert_eq!(
        leaf.afferent, 2,
        "leaf's two inbound `Renders` edges must register as afferent coupling",
    );
    assert_eq!(
        leaf.efferent, 1,
        "leaf's outbound `UsesHook` edge must register as efferent coupling",
    );
}
