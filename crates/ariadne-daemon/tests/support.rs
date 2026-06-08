//! Shared fixtures + cold-path reference builders for the tier-07
//! warm-graph parity tests. Each test binary pulls this in via
//! `mod support;`. The cold references are computed directly through
//! `ariadne-graph` / `ariadne-storage` on the same redb fixture, so a daemon
//! response that matches them matches the v1 cold path
//! [src: .claude/plans/post-v1-roadmap/tier-07-daemon-warm-graph.md step 7].

#![allow(dead_code, clippy::missing_panics_doc)]

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::time::{Duration, Instant};

use ariadne_core::{
    Changeset, CycleBreakRow, EdgeKey, EdgeKind, EdgeRecord, FileId, FileRecord, GodModuleRow,
    Lang, MisplacedRow, OutboundRow, ReadSnapshot, RefactorReport, Span, Storage, SymbolId,
    SymbolRecord, SymbolSummary, Visibility, WriteTxn,
};
use ariadne_daemon::DaemonStatus;
use ariadne_graph::{GraphIndex, ModuleSpec};
use ariadne_storage::RedbStorage;

#[must_use]
pub fn fid(n: u32) -> FileId {
    FileId::new(n).expect("nonzero file id")
}

#[must_use]
pub fn sid(n: u64) -> SymbolId {
    SymbolId::new(n).expect("nonzero symbol id")
}

fn span(file: u32, start: u32, end: u32) -> Span {
    Span {
        file: fid(file),
        byte_start: start,
        byte_end: end,
    }
}

/// Canonical 4-file / 7-symbol fixture, identical in shape to the MCP
/// integration fixture so the daemon and cold paths share goldens.
#[must_use]
pub fn canonical_changeset() -> Changeset {
    let mut cs = Changeset::new();
    for (id, path) in [
        (1, "src/main.rs"),
        (2, "src/lib.rs"),
        (3, "src/util.rs"),
        (4, "src/helper.rs"),
    ] {
        cs = cs.upsert_file(
            fid(id),
            FileRecord {
                path: path.into(),
                lang: Lang::Rust,
                size: 128,
                blake3: [u8::try_from(id).expect("fits u8"); 32],
                mtime_ns: i128::from(id),
            },
        );
    }
    for (id, name, file) in [
        (1u64, "crate::main", 1),
        (2, "crate::run", 2),
        (3, "crate::util::helper", 3),
        (4, "crate::util::leaf", 3),
        (5, "crate::helper::extra", 4),
        (6, "crate::orphan", 2),
        (7, "crate::unused_helper", 3),
    ] {
        cs = cs.upsert_symbol(
            sid(id),
            SymbolRecord {
                canonical_name: name.into(),
                kind: "function".into(),
                defining_file: fid(file),
                defining_span: span(file, 0, 32),
                visibility: Visibility::Unknown,
                attributes: Vec::new(),
                complexity: 0,
            },
        );
    }
    for (src, dst, kind, file) in [
        (1u64, 2, EdgeKind::References, 1),
        (2, 3, EdgeKind::References, 2),
        (2, 4, EdgeKind::References, 2),
        (3, 5, EdgeKind::References, 3),
        (5, 3, EdgeKind::References, 4),
        (1, 6, EdgeKind::Imports, 1),
    ] {
        cs = cs.add_edge(
            EdgeKey {
                src: sid(src),
                kind,
                dst: sid(dst),
            },
            EdgeRecord {
                source_span: span(file, 64, 96),
                evidence_lang: Lang::Rust,
                weight: 1,
            },
        );
    }
    cs
}

/// Vue single-file-component fixture (App renders Card; Card renders Button
/// and uses the `useToggle` hook). Mirrors the MCP component-graph fixture.
#[must_use]
pub fn component_changeset() -> Changeset {
    let mut cs = Changeset::new();
    for (id, path, lang) in [
        (1, "src/App.vue", Lang::Vue),
        (2, "src/Card.vue", Lang::Vue),
        (3, "src/Button.vue", Lang::Vue),
        (4, "src/composables.ts", Lang::TypeScript),
    ] {
        cs = cs.upsert_file(
            fid(id),
            FileRecord {
                path: path.into(),
                lang,
                size: 128,
                blake3: [u8::try_from(id).expect("fits u8"); 32],
                mtime_ns: i128::from(id),
            },
        );
    }
    for (id, name, kind, file) in [
        (1u64, "App", "component", 1u32),
        (2, "Card", "component", 2),
        (3, "Button", "component", 3),
        (4, "useToggle", "function", 4),
    ] {
        cs = cs.upsert_symbol(
            sid(id),
            SymbolRecord {
                canonical_name: name.into(),
                kind: kind.into(),
                defining_file: fid(file),
                defining_span: span(file, 0, 64),
                visibility: Visibility::Unknown,
                attributes: Vec::new(),
                complexity: 0,
            },
        );
    }
    for (src, dst, kind, file) in [
        (1u64, 2, EdgeKind::Renders, 1u32),
        (2, 3, EdgeKind::Renders, 2),
        (2, 4, EdgeKind::UsesHook, 2),
    ] {
        cs = cs.add_edge(
            EdgeKey {
                src: sid(src),
                kind,
                dst: sid(dst),
            },
            EdgeRecord {
                source_span: span(file, 64, 96),
                evidence_lang: if file == 4 {
                    Lang::TypeScript
                } else {
                    Lang::Vue
                },
                weight: 1,
            },
        );
    }
    cs
}

/// Apply `cs` to `<root>/.ariadne/index.redb`, creating it if absent.
/// Returns the committed revision. The handle is dropped before return so
/// the daemon can claim the single-open file.
#[must_use]
pub fn apply(root: &Path, cs: &Changeset) -> u64 {
    let storage = RedbStorage::open(&index_path(root)).expect("open redb");
    let txn = storage.begin_write().expect("begin write");
    txn.apply(cs).expect("apply changeset").0
}

/// Seed the canonical fixture and return its committed revision.
#[must_use]
pub fn seed(root: &Path) -> u64 {
    apply(root, &canonical_changeset())
}

fn index_path(root: &Path) -> std::path::PathBuf {
    root.join(".ariadne").join("index.redb")
}

/// Cold-path reference built directly from the redb fixture, independent of
/// the daemon code path.
#[derive(Debug)]
pub struct Cold {
    pub graph: GraphIndex,
    pub by_name: BTreeMap<String, SymbolId>,
    pub summary: BTreeMap<SymbolId, SymbolSummary>,
    pub paths: BTreeMap<FileId, String>,
    pub file_symbols: BTreeMap<FileId, BTreeSet<SymbolId>>,
    pub sym_file: BTreeMap<SymbolId, FileId>,
}

impl Cold {
    /// `SymbolId` for a canonical name.
    #[must_use]
    pub fn id(&self, name: &str) -> SymbolId {
        self.by_name[name]
    }

    /// Wire summary for a canonical name.
    #[must_use]
    pub fn summ(&self, name: &str) -> SymbolSummary {
        self.summary[&self.by_name[name]].clone()
    }

    /// One `ModuleSpec` per file, gated by `prefix` — mirrors the daemon's
    /// `build_modules`.
    #[must_use]
    pub fn modules(&self, prefix: Option<&str>) -> Vec<ModuleSpec> {
        let mut out = Vec::new();
        for (fid, members) in &self.file_symbols {
            let Some(path) = self.paths.get(fid) else {
                continue;
            };
            if let Some(p) = prefix {
                if !path.starts_with(p) {
                    continue;
                }
            }
            out.push(ModuleSpec {
                name: path.clone(),
                members: members.clone(),
                abstract_members: BTreeSet::new(),
            });
        }
        out
    }
}

/// Build the cold reference from `<root>/.ariadne/index.redb`. Opens redb
/// transiently; safe to call before the daemon is spawned, or while it is
/// idle (the daemon holds the file only during build/refresh).
#[must_use]
pub fn cold(root: &Path) -> Cold {
    let storage = RedbStorage::open(&index_path(root)).expect("open redb");
    let snap = storage.snapshot().expect("snapshot");
    let graph = GraphIndex::build_from_snapshot(&snap).expect("build graph");

    let mut paths = BTreeMap::new();
    for chunk in snap.iter_files(4096).expect("iter files") {
        for (id, rec) in chunk.expect("file chunk") {
            paths.insert(id, rec.path);
        }
    }
    let mut by_name = BTreeMap::new();
    let mut summary = BTreeMap::new();
    let mut file_symbols: BTreeMap<FileId, BTreeSet<SymbolId>> = BTreeMap::new();
    let mut sym_file = BTreeMap::new();
    for chunk in snap.iter_symbols(4096).expect("iter symbols") {
        for (id, rec) in chunk.expect("symbol chunk") {
            by_name.entry(rec.canonical_name.clone()).or_insert(id);
            file_symbols
                .entry(rec.defining_file)
                .or_default()
                .insert(id);
            sym_file.insert(id, rec.defining_file);
            summary.insert(
                id,
                SymbolSummary {
                    id: Some(id.get()),
                    name: rec.canonical_name.clone(),
                    kind: rec.kind.clone(),
                    file: paths.get(&rec.defining_file).cloned().unwrap_or_default(),
                    byte_start: Some(rec.defining_span.byte_start),
                    byte_end: Some(rec.defining_span.byte_end),
                },
            );
        }
    }
    Cold {
        graph,
        by_name,
        summary,
        paths,
        file_symbols,
        sym_file,
    }
}

/// Cold-render a module's markdown via `ariadne-graph::docgen`, holding the
/// redb snapshot live for the scan. Returns `None` when `path` is not a
/// module.
#[must_use]
pub fn cold_doc_module(root: &Path, path: &str) -> Option<String> {
    let reference = cold(root);
    let modules = reference.modules(None);
    let module = modules.iter().find(|m| m.name == path)?;
    let storage = RedbStorage::open(&index_path(root)).expect("open redb");
    let snap = storage.snapshot().expect("snapshot");
    // Mirror the warm catalog's load + sort so the cold render is byte-identical
    // [src: daemon catalog.rs:147-148].
    let mut churn = storage.all_churn().expect("churn");
    churn.sort_by(|a, b| a.path.cmp(&b.path));
    Some(
        ariadne_graph::docgen::for_module(
            &reference.graph,
            &snap,
            module,
            &churn,
            &ariadne_graph::DocScope::default(),
        )
        .expect("render module"),
    )
}

/// Cold-build the refactor report directly through `ariadne-graph::refactor`
/// on the redb fixture — the v1 MCP `refactor_suggestions` reference. Mirrors
/// `crates/ariadne-mcp/src/tools/refactor.rs` (god threshold 8.0).
#[must_use]
pub fn cold_refactor(root: &Path, prefix: Option<&str>) -> RefactorReport {
    const GOD_THRESHOLD: f32 = 8.0;
    let reference = cold(root);
    let modules = reference.modules(prefix);
    let storage = RedbStorage::open(&index_path(root)).expect("open redb");
    let snap = storage.snapshot().expect("snapshot");
    let name_of = |id: SymbolId| {
        reference
            .summary
            .get(&id)
            .map_or_else(|| format!("#{}", id.get()), |m| m.name.clone())
    };

    let god_modules =
        ariadne_graph::refactor::god_modules(&reference.graph, &snap, &modules, GOD_THRESHOLD)
            .expect("god modules")
            .into_iter()
            .map(|g| GodModuleRow {
                module: g.module,
                efferent: g.efferent,
                cohesion: g.cohesion,
                top_outbound: g
                    .top_outbound
                    .into_iter()
                    .map(|(s, edges)| OutboundRow {
                        symbol: name_of(s),
                        edges,
                    })
                    .collect(),
                suggestion: g.suggestion,
            })
            .collect();

    let mut cycle_breaks = Vec::new();
    for cycle in reference.graph.cycle_report().cycles {
        for p in ariadne_graph::refactor::cycle_break_proposals(&reference.graph, &cycle) {
            cycle_breaks.push(CycleBreakRow {
                from: name_of(p.from),
                to: name_of(p.to),
                score: p.score,
                rationale: p.rationale.to_owned(),
            });
        }
    }

    let misplaced_symbols = ariadne_graph::refactor::misplaced_symbols(&reference.graph, &modules)
        .into_iter()
        .map(|m| MisplacedRow {
            symbol: name_of(m.symbol),
            current_module: m.current_module,
            target_module: m.target_module,
            ratio: m.ratio,
        })
        .collect();

    RefactorReport {
        god_modules,
        cycle_breaks,
        misplaced_symbols,
    }
}

/// Cold-render the project markdown via `ariadne-graph::docgen`.
#[must_use]
pub fn cold_doc_project(root: &Path, prefix: Option<&str>) -> String {
    let reference = cold(root);
    let modules = reference.modules(prefix);
    let storage = RedbStorage::open(&index_path(root)).expect("open redb");
    let snap = storage.snapshot().expect("snapshot");
    // Mirror the warm catalog's load + sort so the cold render is byte-identical
    // [src: daemon catalog.rs:147-150].
    let mut churn = storage.all_churn().expect("churn");
    churn.sort_by(|a, b| a.path.cmp(&b.path));
    let mut co_change = storage.all_co_change().expect("co_change");
    co_change.sort_by(|a, b| a.a.cmp(&b.a).then_with(|| a.b.cmp(&b.b)));
    ariadne_graph::docgen::for_project(
        &reference.graph,
        &snap,
        &modules,
        &churn,
        &co_change,
        &ariadne_graph::DocScope::default(),
    )
    .expect("render project")
}

/// Spawn `serve` on a background thread and block until it answers.
#[must_use]
pub fn spawn(root: &Path) -> std::thread::JoinHandle<Result<(), ariadne_daemon::DaemonError>> {
    let serve_root = root.to_path_buf();
    let handle = std::thread::spawn(move || ariadne_daemon::serve(&serve_root));
    wait_until_running(root, Duration::from_secs(5));
    handle
}

/// Stop the daemon and join the serve thread cleanly.
pub fn shutdown(
    root: &Path,
    handle: std::thread::JoinHandle<Result<(), ariadne_daemon::DaemonError>>,
) {
    ariadne_daemon::stop(root).expect("stop");
    handle
        .join()
        .expect("serve thread join")
        .expect("serve returns Ok");
}

fn wait_until_running(root: &Path, timeout: Duration) {
    let deadline = Instant::now() + timeout;
    loop {
        if matches!(
            ariadne_daemon::status(root).expect("status probe"),
            DaemonStatus::Running { .. }
        ) {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "daemon did not reach Running within {timeout:?}",
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}
