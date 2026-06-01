//! Shared fixture builders for the MCP integration tests.
//!
//! Each test binary pulls this module in via `mod support;`. Tests share
//! the helpers that seed a redb-backed `.ariadne/index.redb` with a
//! deterministic 4-file / 6-symbol / 6-edge graph plus the spawn helper
//! that wires an rmcp `TokioChildProcess` client to the binary.

#![allow(dead_code, clippy::missing_panics_doc)]

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use ariadne_core::{
    Changeset, CoChangePair, DaemonQuery, DaemonRequest, DaemonResponse, EdgeKey, EdgeKind,
    EdgeRecord, FileChurn, FileId, FileRecord, Lang, Span, Storage, SymbolChurn, SymbolId,
    SymbolRecord, Visibility, WriteTxn,
};
use ariadne_storage::RedbStorage;
use interprocess::local_socket::prelude::*;
use interprocess::local_socket::{GenericFilePath, ListenerOptions};
use rmcp::ServiceExt;
use rmcp::model::{CallToolResult, RawContent};
use rmcp::service::RunningService;
use rmcp::transport::{ConfigureCommandExt, TokioChildProcess};
use tempfile::TempDir;
use tokio::process::Command;

#[must_use]
pub fn fid(n: u32) -> FileId {
    FileId::new(n).expect("nonzero file id")
}

#[must_use]
pub fn sid(n: u64) -> SymbolId {
    SymbolId::new(n).expect("nonzero symbol id")
}

/// Seed a redb DB under `<dir>/.ariadne/index.redb` with the canonical
/// 4-file fixture. Returns the project root + the tempdir guard.
#[must_use]
pub fn seed_tiny_project() -> (PathBuf, TempDir) {
    let dir = tempfile::tempdir().expect("tempdir");
    let project_root = dir.path().to_path_buf();
    let storage_path = project_root.join(".ariadne").join("index.redb");
    let storage = RedbStorage::open(&storage_path).expect("open redb");
    let cs = canonical_changeset();
    let txn = storage.begin_write().expect("begin");
    txn.apply(&cs).expect("apply changeset");
    drop(storage);
    (project_root, dir)
}

/// Seed a project whose `.ariadne/index.redb` has bootstrapped-but-empty
/// tables — no files, symbols, or edges. Drives the negative
/// `doc_for_project` case (tier-09 `<verification>`).
#[must_use]
pub fn seed_empty_project() -> (PathBuf, TempDir) {
    let dir = tempfile::tempdir().expect("tempdir");
    let project_root = dir.path().to_path_buf();
    let storage_path = project_root.join(".ariadne").join("index.redb");
    let storage = RedbStorage::open(&storage_path).expect("open redb");
    drop(storage);
    (project_root, dir)
}

/// Seed a project with three high-efferent files — a library file
/// (`src/hub.rs`), an integration-test file (`tests/big_suite.rs`), and
/// a build script (`build.rs`) — each fanning out to eighteen leaf
/// symbols (efferent 18, comfortably above `GOD_THRESHOLD`). Drives the
/// `weak_spots` god-module exclusion test: only the library file is a
/// true architecture smell; the test file and build script are excluded.
#[must_use]
pub fn seed_god_module_project() -> (PathBuf, TempDir) {
    let dir = tempfile::tempdir().expect("tempdir");
    let project_root = dir.path().to_path_buf();
    let storage_path = project_root.join(".ariadne").join("index.redb");
    let storage = RedbStorage::open(&storage_path).expect("open redb");
    let cs = god_module_changeset();
    let txn = storage.begin_write().expect("begin");
    txn.apply(&cs).expect("apply changeset");
    drop(storage);
    (project_root, dir)
}

/// Seed a Vue single-file-component project: three `.vue` components
/// (`App` renders `Card`; `Card` renders `Button` and uses the `useToggle`
/// composable) plus the `composables.ts` the hook is defined in. Drives the
/// `file_summary` component-graph golden (tier-09 step 4).
#[must_use]
pub fn seed_component_project() -> (PathBuf, TempDir) {
    let dir = tempfile::tempdir().expect("tempdir");
    let project_root = dir.path().to_path_buf();
    let storage_path = project_root.join(".ariadne").join("index.redb");
    let storage = RedbStorage::open(&storage_path).expect("open redb");
    let cs = component_changeset();
    let txn = storage.begin_write().expect("begin");
    txn.apply(&cs).expect("apply changeset");
    drop(storage);
    (project_root, dir)
}

/// Seed a project carrying the Block-C analytics substrate: two Rust files
/// whose symbols have non-zero cyclomatic `complexity`, plus persisted
/// per-file churn, one co-change pair, and one per-symbol churn record. The
/// churn / symbol-churn vectors are written deliberately out of key order so
/// the catalog's load-time sort (tier-15a D3) is exercised. Drives the
/// tier-15a catalog-projection test and the 15b/15c analytics goldens
/// [src: .claude/plans/post-v1-roadmap/tier-15a-analytics-catalog-projection.md].
#[must_use]
pub fn seed_analytics_project() -> (PathBuf, TempDir) {
    let dir = tempfile::tempdir().expect("tempdir");
    let project_root = dir.path().to_path_buf();
    let storage_path = project_root.join(".ariadne").join("index.redb");
    let storage = RedbStorage::open(&storage_path).expect("open redb");
    let txn = storage.begin_write().expect("begin");
    txn.apply(&analytics_changeset()).expect("apply changeset");
    storage
        .replace_history(&analytics_churn(), &analytics_pairs())
        .expect("replace history");
    storage
        .replace_symbol_churn(&analytics_symbol_churn())
        .expect("replace symbol churn");
    drop(storage);
    (project_root, dir)
}

fn analytics_changeset() -> Changeset {
    let mut cs = Changeset::new();
    for (id, path) in [(1u32, "src/alpha.rs"), (2, "src/beta.rs")] {
        cs = cs.upsert_file(
            fid(id),
            FileRecord {
                path: path.into(),
                lang: Lang::Rust,
                size: 128,
                blake3: [u8::try_from(id).expect("file id fits u8"); 32],
                mtime_ns: i128::from(id),
            },
        );
    }
    for (id, name, file, complexity) in
        [(1u64, "crate::alpha", 1u32, 7u32), (2, "crate::beta", 2, 3)]
    {
        cs = cs.upsert_symbol(
            sid(id),
            SymbolRecord {
                canonical_name: name.into(),
                kind: "function".into(),
                defining_file: fid(file),
                defining_span: span(file, 0, 64),
                visibility: Visibility::Unknown,
                attributes: Vec::new(),
                complexity,
            },
        );
    }
    cs
}

/// File churn written beta-before-alpha to prove the load sorts by path.
fn analytics_churn() -> Vec<FileChurn> {
    vec![
        FileChurn {
            path: "src/beta.rs".into(),
            commits: 4,
            author_keys: vec![[1u8; 8], [2u8; 8]],
            last_changed_ns: 200,
        },
        FileChurn {
            path: "src/alpha.rs".into(),
            commits: 9,
            author_keys: vec![[1u8; 8]],
            last_changed_ns: 100,
        },
    ]
}

fn analytics_pairs() -> Vec<CoChangePair> {
    vec![CoChangePair {
        a: "src/alpha.rs".into(),
        b: "src/beta.rs".into(),
        count: 3,
    }]
}

/// Symbol churn written sid(2)-before-sid(1) to prove the load sorts by id.
fn analytics_symbol_churn() -> Vec<SymbolChurn> {
    vec![
        SymbolChurn {
            symbol: sid(2),
            commits: 2,
        },
        SymbolChurn {
            symbol: sid(1),
            commits: 5,
        },
    ]
}

fn component_changeset() -> Changeset {
    let mut cs = Changeset::new();
    let files = [
        (1, "src/App.vue", Lang::Vue),
        (2, "src/Card.vue", Lang::Vue),
        (3, "src/Button.vue", Lang::Vue),
        (4, "src/composables.ts", Lang::TypeScript),
    ];
    for (id, path, lang) in files {
        cs = cs.upsert_file(
            fid(id),
            FileRecord {
                path: path.into(),
                lang,
                size: 128,
                blake3: [u8::try_from(id).expect("file id fits u8"); 32],
                mtime_ns: i128::from(id),
            },
        );
    }
    let symbols = [
        (1u64, "App", "component", 1u32),
        (2, "Card", "component", 2),
        (3, "Button", "component", 3),
        (4, "useToggle", "function", 4),
    ];
    for (id, name, kind, file) in symbols {
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
    // `src` is the rendering/using component, `file` the SFC the render or
    // hook site sits in — the source span of the resolved edge.
    let edges = [
        (1u64, 2, EdgeKind::Renders, 1u32),
        (2, 3, EdgeKind::Renders, 2),
        (2, 4, EdgeKind::UsesHook, 2),
    ];
    for (src, dst, kind, file) in edges {
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

fn god_module_changeset() -> Changeset {
    let mut cs = Changeset::new();
    // File 1 = library target, file 2 = integration-test target, file 4
    // = build script; all three fan out to the eighteen leaf symbols in
    // file 3.
    let files = [
        (1, "src/hub.rs"),
        (2, "tests/big_suite.rs"),
        (3, "src/leaves.rs"),
        (4, "build.rs"),
    ];
    for (id, path) in files {
        cs = cs.upsert_file(
            fid(id),
            FileRecord {
                path: path.into(),
                lang: Lang::Rust,
                size: 256,
                blake3: [u8::try_from(id).expect("file id fits u8"); 32],
                mtime_ns: i128::from(id),
            },
        );
    }
    // Hub symbols: sid(1) in src/hub.rs, sid(2) in tests/big_suite.rs,
    // sid(21) in build.rs.
    let hubs = [
        (1u64, "crate::hub_fn", 1u32),
        (2, "crate::big_suite_fn", 2),
        (21, "crate::build_main", 4),
    ];
    for (id, name, file) in hubs {
        cs = cs.upsert_symbol(
            sid(id),
            SymbolRecord {
                canonical_name: name.into(),
                kind: "function".into(),
                defining_file: fid(file),
                defining_span: span(file, 0, 64),
                visibility: Visibility::Unknown,
                attributes: Vec::new(),
                complexity: 0,
            },
        );
    }
    // Eighteen leaf symbols (sid 3..=20) in src/leaves.rs.
    for n in 3u64..=20 {
        cs = cs.upsert_symbol(
            sid(n),
            SymbolRecord {
                canonical_name: format!("crate::leaf_{n}"),
                kind: "function".into(),
                defining_file: fid(3),
                defining_span: span(3, 0, 16),
                visibility: Visibility::Unknown,
                attributes: Vec::new(),
                complexity: 0,
            },
        );
    }
    // Each hub fans out to all leaves: efferent = 18 > GOD_THRESHOLD.
    for (hub, hub_file) in [(1u64, 1u32), (2, 2), (21, 4)] {
        for leaf in 3u64..=20 {
            cs = cs.add_edge(
                EdgeKey {
                    src: sid(hub),
                    kind: EdgeKind::References,
                    dst: sid(leaf),
                },
                EdgeRecord {
                    source_span: span(hub_file, 64, 96),
                    evidence_lang: Lang::Rust,
                    weight: 1,
                },
            );
        }
    }
    cs
}

fn span(file: u32, start: u32, end: u32) -> Span {
    Span {
        file: fid(file),
        byte_start: start,
        byte_end: end,
    }
}

fn canonical_changeset() -> Changeset {
    let mut cs = Changeset::new();
    let files = [
        (1, "src/main.rs"),
        (2, "src/lib.rs"),
        (3, "src/util.rs"),
        (4, "src/helper.rs"),
    ];
    for (id, path) in files {
        cs = cs.upsert_file(
            fid(id),
            FileRecord {
                path: path.into(),
                lang: Lang::Rust,
                size: 128,
                blake3: [u8::try_from(id).expect("file id fits u8"); 32],
                mtime_ns: i128::from(id),
            },
        );
    }
    // sid(1) `crate::main` exercises the tier-05 Rust root classifier
    // (Rust `fn main` convention). sid(7) `crate::unused_helper` is the
    // genuinely dead non-root: fan-in=0 with no visibility / attribute
    // signal, so it surfaces in `dead_symbols` after the root exemption.
    let symbols = [
        (1u64, "crate::main", "function", 1),
        (2, "crate::run", "function", 2),
        (3, "crate::util::helper", "function", 3),
        (4, "crate::util::leaf", "function", 3),
        (5, "crate::helper::extra", "function", 4),
        (6, "crate::orphan", "function", 2),
        (7, "crate::unused_helper", "function", 3),
    ];
    for (id, name, kind, file) in symbols {
        cs = cs.upsert_symbol(
            sid(id),
            SymbolRecord {
                canonical_name: name.into(),
                kind: kind.into(),
                defining_file: fid(file),
                defining_span: span(file, 0, 32),
                visibility: Visibility::Unknown,
                attributes: Vec::new(),
                complexity: 0,
            },
        );
    }
    let edges = [
        (1u64, 2, EdgeKind::References, 1),
        (2, 3, EdgeKind::References, 2),
        (2, 4, EdgeKind::References, 2),
        (3, 5, EdgeKind::References, 3),
        (5, 3, EdgeKind::References, 4),
        (1, 6, EdgeKind::Imports, 1),
    ];
    for (src, dst, kind, file) in edges {
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

/// Spawn the `ariadne-mcp` test binary against `root` and return a ready
/// rmcp client peer. The binary path comes from cargo's
/// `CARGO_BIN_EXE_<name>` env var so we never invoke `cargo run` during
/// tests (avoids races on the build lock).
pub async fn spawn_client(root: &std::path::Path) -> RunningService<rmcp::RoleClient, ()> {
    let exe = env!("CARGO_BIN_EXE_ariadne-mcp");
    let root = root.to_path_buf();
    let child = TokioChildProcess::new(Command::new(exe).configure(|cmd| {
        cmd.arg("serve")
            .arg("--root")
            .arg(&root)
            .env("RUST_LOG", "warn")
            // Disable daemon auto-spawn so the routing path is deterministic:
            // a present stub daemon is used, an absent one falls straight back
            // to the cold path (no throwaway `daemon start` child per call).
            .env("ARIADNE_MCP_AUTOSPAWN", "0")
            .kill_on_drop(true);
    }))
    .expect("spawn ariadne-mcp child");
    tokio::time::timeout(Duration::from_secs(15), ().serve(child))
        .await
        .expect("rmcp initialize timeout")
        .expect("rmcp initialize error")
}

/// Pull the text payload out of an MCP `CallToolResult`. Every tier-08
/// tool returns its JSON encoded in a single `Content::text(..)` block,
/// so the tests share this projector.
#[must_use]
pub fn extract_text(result: &CallToolResult) -> String {
    let block = result.content.first().expect("at least one content block");
    match &block.raw {
        RawContent::Text(t) => t.text.clone(),
        other => panic!("expected text content, got {other:?}"),
    }
}

/// A stub daemon listening on `<root>/.ariadne/daemon.sock`. Drop removes the
/// socket file; the accept thread is reaped when the (nextest-isolated) test
/// process exits.
#[derive(Debug)]
pub struct StubDaemon {
    socket: PathBuf,
}

impl Drop for StubDaemon {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.socket);
    }
}

/// Bind a stub daemon at `<root>/.ariadne/daemon.sock` that frames responses
/// in the same length-prefixed postcard format as the real daemon
/// (`ariadne-daemon` codec): each accepted connection carries one
/// [`DaemonRequest`] in and one [`DaemonResponse`] out. `Ping` is answered
/// `Pong`; every other query is delegated to `responder`. This is the test's
/// independent protocol oracle — it does not reuse the client's framing.
pub fn spawn_stub_daemon<F>(root: &Path, responder: F) -> StubDaemon
where
    F: Fn(&DaemonQuery) -> DaemonResponse + Send + 'static,
{
    let socket = root.join(".ariadne").join("daemon.sock");
    let name = socket
        .clone()
        .to_fs_name::<GenericFilePath>()
        .expect("socket fs name");
    let listener = ListenerOptions::new()
        .name(name)
        .create_sync()
        .expect("bind stub daemon socket");
    std::thread::spawn(move || {
        for conn in listener.incoming() {
            let Ok(mut stream) = conn else { continue };
            let Ok(payload) = read_frame(&mut stream) else {
                continue;
            };
            let Ok(req) = postcard::from_bytes::<DaemonRequest>(&payload) else {
                continue;
            };
            let resp = match &req.query {
                DaemonQuery::Ping => DaemonResponse::Pong,
                other => responder(other),
            };
            let bytes = postcard::to_stdvec(&resp).expect("encode response");
            let _ = write_frame(&mut stream, &bytes);
        }
    });
    StubDaemon { socket }
}

fn write_frame<W: Write>(w: &mut W, payload: &[u8]) -> std::io::Result<()> {
    let len = u32::try_from(payload.len()).expect("frame fits u32");
    w.write_all(&len.to_be_bytes())?;
    w.write_all(payload)?;
    w.flush()
}

fn read_frame<R: Read>(r: &mut R) -> std::io::Result<Vec<u8>> {
    let mut len_buf = [0u8; 4];
    r.read_exact(&mut len_buf)?;
    let len = u32::from_be_bytes(len_buf) as usize;
    let mut payload = vec![0u8; len];
    r.read_exact(&mut payload)?;
    Ok(payload)
}
