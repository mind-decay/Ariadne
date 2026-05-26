//! Shared fixtures for the tier-09 docgen + refactor golden tests.
//!
//! [`MemSnapshot`] is an in-memory `ReadSnapshot` — the pure-domain test
//! double the plan permits for unit tests of domain code. The real redb
//! path is exercised by the `ariadne-mcp` integration tests; here the
//! graph is supplied directly so the snapshot only has to answer the
//! symbol/file scans the renderers run [src: CLAUDE.md `<rules>`].

#![allow(dead_code, clippy::missing_panics_doc)]

use std::collections::BTreeSet;

use ariadne_core::{
    ChunkStream, EdgeKey, EdgeRecord, FileId, FileRecord, Lang, ReadSnapshot, Span, StorageError,
    SymbolId, SymbolRecord, Visibility,
};
use ariadne_graph::{EdgeKind, GraphIndex, ModuleSpec};

/// Wrap a non-zero `u32` as a [`FileId`].
#[must_use]
pub fn fid(n: u32) -> FileId {
    FileId::new(n).expect("nonzero file id")
}

/// Wrap a non-zero `u64` as a [`SymbolId`].
#[must_use]
pub fn sid(n: u64) -> SymbolId {
    SymbolId::new(n).expect("nonzero symbol id")
}

/// In-memory [`ReadSnapshot`]. Edge accessors stay empty — docgen and
/// refactor take the graph as a [`GraphIndex`] and only scan the
/// snapshot for symbol/file metadata.
#[derive(Debug)]
pub struct MemSnapshot {
    files: Vec<(FileId, FileRecord)>,
    symbols: Vec<(SymbolId, SymbolRecord)>,
}

fn chunked<T: Clone + 'static>(data: &[T], chunk: usize) -> ChunkStream<'static, T> {
    let chunk = chunk.max(1);
    let parts: Vec<Result<Vec<T>, StorageError>> =
        data.chunks(chunk).map(|c| Ok(c.to_vec())).collect();
    Box::new(parts.into_iter())
}

impl ReadSnapshot for MemSnapshot {
    fn file(&self, id: FileId) -> Result<Option<FileRecord>, StorageError> {
        Ok(self
            .files
            .iter()
            .find(|(f, _)| *f == id)
            .map(|(_, r)| r.clone()))
    }
    fn symbols_in_file(&self, _: FileId) -> Result<Vec<SymbolRecord>, StorageError> {
        Ok(Vec::new())
    }
    fn outgoing_edges(&self, _: SymbolId) -> Result<Vec<(EdgeKey, EdgeRecord)>, StorageError> {
        Ok(Vec::new())
    }
    fn incoming_edges(&self, _: SymbolId) -> Result<Vec<(EdgeKey, EdgeRecord)>, StorageError> {
        Ok(Vec::new())
    }
    fn edges_in_file(&self, _: FileId) -> Result<Vec<EdgeKey>, StorageError> {
        Ok(Vec::new())
    }
    fn iter_files(
        &self,
        chunk: usize,
    ) -> Result<ChunkStream<'_, (FileId, FileRecord)>, StorageError> {
        Ok(chunked(&self.files, chunk))
    }
    fn iter_symbols(
        &self,
        chunk: usize,
    ) -> Result<ChunkStream<'_, (SymbolId, SymbolRecord)>, StorageError> {
        Ok(chunked(&self.symbols, chunk))
    }
    fn iter_edges(&self, _: usize) -> Result<ChunkStream<'_, (EdgeKey, EdgeRecord)>, StorageError> {
        Ok(Box::new(std::iter::empty()))
    }
}

/// Canonical 5-file / 8-symbol / 7-edge fixture repo.
#[derive(Debug)]
pub struct Fixture {
    /// In-RAM graph built in canonical insertion order.
    pub graph: GraphIndex,
    /// Snapshot answering name/kind/path lookups.
    pub snapshot: MemSnapshot,
    /// Module decomposition (one module per source file).
    pub modules: Vec<ModuleSpec>,
}

const FILES: [(u32, &str); 5] = [
    (1, "src/core.rs"),
    (2, "src/api.rs"),
    (3, "src/db.rs"),
    (4, "src/util.rs"),
    (5, "src/types.rs"),
];

/// `(id, canonical_name, kind, defining_file)`.
const SYMBOLS: [(u64, &str, &str, u32); 8] = [
    (1, "core::init", "function", 1),
    (2, "core::run", "function", 1),
    (3, "core::shutdown", "function", 1),
    (4, "api::serve", "function", 2),
    (5, "db::query", "function", 3),
    (6, "db::connect", "function", 3),
    (7, "util::log", "function", 4),
    (8, "types::Config", "struct", 5),
];

/// `(src, dst, kind)` edges of the fixture graph.
#[must_use]
pub fn edges() -> Vec<(u64, u64, EdgeKind)> {
    vec![
        (1, 2, EdgeKind::Calls),
        (4, 2, EdgeKind::Calls),
        (2, 5, EdgeKind::Calls),
        (5, 6, EdgeKind::Calls),
        (6, 2, EdgeKind::Calls),
        (1, 7, EdgeKind::Calls),
        (2, 8, EdgeKind::TypeOf),
    ]
}

fn build_graph(sym_order: &[u64], edge_order: &[(u64, u64, EdgeKind)]) -> GraphIndex {
    let mut g = GraphIndex::new();
    for &s in sym_order {
        g.add_symbol(sid(s));
    }
    for &(a, b, k) in edge_order {
        g.add_edge(sid(a), sid(b), k);
    }
    g
}

/// Snapshot carrying the fixture's file + symbol records.
#[must_use]
pub fn snapshot() -> MemSnapshot {
    let files = FILES
        .iter()
        .map(|&(id, path)| {
            (
                fid(id),
                FileRecord {
                    path: path.to_owned(),
                    lang: Lang::Rust,
                    size: 0,
                    blake3: [0u8; 32],
                    mtime_ns: 0,
                },
            )
        })
        .collect();
    let symbols = SYMBOLS
        .iter()
        .map(|&(id, name, kind, file)| {
            (
                sid(id),
                SymbolRecord {
                    canonical_name: name.to_owned(),
                    kind: kind.to_owned(),
                    defining_file: fid(file),
                    defining_span: Span {
                        file: fid(file),
                        byte_start: 0,
                        byte_end: 0,
                    },
                    visibility: Visibility::Unknown,
                    attributes: Vec::new(),
                },
            )
        })
        .collect();
    MemSnapshot { files, symbols }
}

/// Empty snapshot for the negative `for_project` case.
#[must_use]
pub fn empty_snapshot() -> MemSnapshot {
    MemSnapshot {
        files: Vec::new(),
        symbols: Vec::new(),
    }
}

/// One [`ModuleSpec`] per source file.
#[must_use]
pub fn modules() -> Vec<ModuleSpec> {
    let spec = |name: &str, ids: &[u64]| ModuleSpec {
        name: name.to_owned(),
        members: ids.iter().map(|&i| sid(i)).collect(),
        abstract_members: BTreeSet::new(),
    };
    vec![
        spec("core", &[1, 2, 3]),
        spec("api", &[4]),
        spec("db", &[5, 6]),
        spec("util", &[7]),
        spec("types", &[8]),
    ]
}

/// Locate a module by name; panics when absent.
#[must_use]
pub fn module_named<'a>(modules: &'a [ModuleSpec], name: &str) -> &'a ModuleSpec {
    modules
        .iter()
        .find(|m| m.name == name)
        .expect("module present in fixture")
}

/// Build the canonical fixture (symbols + edges in declared order).
#[must_use]
pub fn core_fixture() -> Fixture {
    let syms: Vec<u64> = (1..=8).collect();
    Fixture {
        graph: build_graph(&syms, &edges()),
        snapshot: snapshot(),
        modules: modules(),
    }
}

fn shuffle<T>(v: &mut [T], state: &mut u64) {
    for i in (1..v.len()).rev() {
        *state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1);
        let j = ((*state >> 33) as usize) % (i + 1);
        v.swap(i, j);
    }
}

/// Build the fixture graph with a `seed`-derived symbol/edge insertion
/// order. Drives the tier-09 step-8 determinism proptest.
#[must_use]
pub fn shuffled_graph(seed: u64) -> GraphIndex {
    let mut syms: Vec<u64> = (1..=8).collect();
    let mut es = edges();
    let mut state = seed;
    shuffle(&mut syms, &mut state);
    shuffle(&mut es, &mut state);
    build_graph(&syms, &es)
}

/// Two-module fixture isolating exactly one misplaced symbol: `a1` lives
/// in `mod_a` but is called three times from `mod_b` and once from its
/// own module.
#[must_use]
pub fn misplaced_fixture() -> (GraphIndex, Vec<ModuleSpec>) {
    let mut g = GraphIndex::new();
    for s in 1u64..=5 {
        g.add_symbol(sid(s));
    }
    g.add_edge(sid(2), sid(1), EdgeKind::Calls);
    g.add_edge(sid(3), sid(1), EdgeKind::Calls);
    g.add_edge(sid(4), sid(1), EdgeKind::Calls);
    g.add_edge(sid(5), sid(1), EdgeKind::Calls);
    let modules = vec![
        ModuleSpec {
            name: "mod_a".to_owned(),
            members: [sid(1), sid(2)].into_iter().collect(),
            abstract_members: BTreeSet::new(),
        },
        ModuleSpec {
            name: "mod_b".to_owned(),
            members: [sid(3), sid(4), sid(5)].into_iter().collect(),
            abstract_members: BTreeSet::new(),
        },
    ];
    (g, modules)
}
