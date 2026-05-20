//! Shared fixture builders for the MCP integration tests.
//!
//! Each test binary pulls this module in via `mod support;`. Tests share
//! the helpers that seed a redb-backed `.ariadne/index.redb` with a
//! deterministic 4-file / 6-symbol / 6-edge graph plus the spawn helper
//! that wires an rmcp `TokioChildProcess` client to the binary.

#![allow(dead_code, clippy::missing_panics_doc)]

use std::path::PathBuf;
use std::time::Duration;

use ariadne_core::{
    Changeset, EdgeKey, EdgeKind, EdgeRecord, FileId, FileRecord, Lang, Span, Storage, SymbolId,
    SymbolRecord, WriteTxn,
};
use ariadne_storage::RedbStorage;
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
    let symbols = [
        (1u64, "crate::main", "function", 1),
        (2, "crate::run", "function", 2),
        (3, "crate::util::helper", "function", 3),
        (4, "crate::util::leaf", "function", 3),
        (5, "crate::helper::extra", "function", 4),
        (6, "crate::orphan", "function", 2),
    ];
    for (id, name, kind, file) in symbols {
        cs = cs.upsert_symbol(
            sid(id),
            SymbolRecord {
                canonical_name: name.into(),
                kind: kind.into(),
                defining_file: fid(file),
                defining_span: span(file, 0, 32),
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
