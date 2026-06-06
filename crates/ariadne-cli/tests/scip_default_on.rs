//! scip-driven-edges tier-04: SCIP is DEFAULT-ON, but a run where every indexer
//! binary is absent is a degraded WARNING — never a failure — and the index
//! still completes on the precise tree-sitter resolver (plan D6, R1; exit #1)
//! [src: docs/adr/0026-default-on-out-of-band-scip.md].
//!
//! Hermetic by construction: the `index` child runs with an empty `PATH`, so no
//! SCIP indexer binary (`rust-analyzer`, `scip-*`) is reachable regardless of
//! what the host has installed — every driver detects as missing.

use std::path::Path;
use std::process::Command;

use serde_json::Value;

/// The cargo-built `ariadne` binary. Invoked by absolute path, so an empty
/// `PATH` on the child hides the SCIP indexers without disabling the CLI itself.
fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_ariadne")
}

/// Run `ariadne index <root>` (default-on SCIP) with `PATH` pointed at an empty
/// directory, parse the JSON-line summary from stdout, and fail loudly on a
/// non-zero exit — the degraded run must succeed, not bail.
fn index_without_indexers(root: &Path, empty_path: &Path) -> Value {
    let output = Command::new(bin())
        .args(["index", root.to_str().expect("utf8 root")])
        .env("PATH", empty_path)
        .output()
        .expect("spawn `ariadne index`");
    assert!(
        output.status.success(),
        "default-on `ariadne index` with no indexer on PATH must succeed (degraded, \
         not fail): {}",
        String::from_utf8_lossy(&output.stderr),
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let line = stdout
        .lines()
        .rev()
        .find(|l| l.trim_start().starts_with('{'))
        .expect("`ariadne index` printed a JSON summary");
    serde_json::from_str(line).expect("parse index summary JSON")
}

/// Default-on SCIP with every indexer binary absent: the run degrades to the
/// precise resolver — a warning, never a failure — and still produces a
/// populated graph (the cross-file call edge the resolver derives).
#[test]
fn default_on_index_degrades_to_resolver_without_indexers() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path();

    // A `Cargo.toml` makes the rust-analyzer driver `detect`, so with the binary
    // hidden it reports MISSING (the degraded warning) rather than being skipped
    // outright [src: crates/ariadne-scip/src/indexer/rust_analyzer.rs detect].
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"degraded-fixture\"\nversion = \"0.0.0\"\nedition = \"2021\"\n",
    )
    .expect("write Cargo.toml");
    // A callee + same-file caller: the precise resolver resolves the free call
    // with no SCIP help, so a degraded run still yields a non-empty graph.
    std::fs::write(
        root.join("lib.rs"),
        "pub fn helper() {}\nfn caller() { helper(); }\n",
    )
    .expect("write fixture");

    let init = Command::new(bin())
        .args(["init", root.to_str().expect("utf8 root")])
        .output()
        .expect("spawn `ariadne init`");
    assert!(
        init.status.success(),
        "init: {}",
        String::from_utf8_lossy(&init.stderr),
    );

    // An empty directory as `PATH` hides rust-analyzer / scip-* on any host, so
    // every indexer detects as missing → degraded warning, never a hard failure.
    let empty = root.join("empty-path");
    std::fs::create_dir_all(&empty).expect("create empty PATH dir");

    let summary = index_without_indexers(root, &empty);

    assert_eq!(
        summary["scip_successes"].as_array().map(Vec::len),
        Some(0),
        "no indexer binary is reachable, so no SCIP indexer can succeed: {summary}",
    );
    assert!(
        summary["scip_missing"]
            .as_array()
            .is_some_and(|a| !a.is_empty()),
        "every indexer binary is hidden, so the degraded run must report them \
         missing (the warning, not a failure): {summary}",
    );
    assert!(
        summary["symbols"].as_u64().unwrap_or(0) >= 2,
        "helper + caller must both be indexed on the resolver in degraded mode: {summary}",
    );
    assert!(
        summary["edges"].as_u64().unwrap_or(0) >= 1,
        "the precise resolver must still derive the call edge in degraded mode: {summary}",
    );
}
