//! Go SCIP ingest test — drives the native `scip-go` driver
//! ([`ScipGoIndexer`]).
//!
//! `detect` is a pure filesystem probe and always runs. The `run` test
//! shells out to the real `scip-go` binary over the committed
//! `fixtures/go` module; it is skipped (not failed) when `scip-go` is not
//! installed, matching the crate's "missing indexer degrades, never
//! crashes" contract.
//!
//! Plan ref: `.claude/plans/post-v1-roadmap/tier-01-go-native-scip.md`.

mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

use ariadne_core::Lang;
use ariadne_scip::{ScipGoIndexer, ScipIndexer};

use crate::common::{SymBp, synth_bytes};

/// Resolve the `scip-go` binary: `PATH` first, then `$(go env GOPATH)/bin`
/// (its `go install` destination). Returns `None` when it is absent.
fn scip_go_binary() -> Option<PathBuf> {
    if let Some(paths) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&paths) {
            let candidate = dir.join("scip-go");
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    let output = Command::new("go").args(["env", "GOPATH"]).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let gopath = String::from_utf8(output.stdout).ok()?;
    let candidate = Path::new(gopath.trim()).join("bin").join("scip-go");
    candidate.is_file().then_some(candidate)
}

/// `fixtures/go` — the committed minimal Go module.
fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/go")
}

#[test]
fn detect_fires_on_go_mod() {
    let with_mod = tempfile::tempdir().expect("tempdir");
    std::fs::write(with_mod.path().join("go.mod"), "module demo\n\ngo 1.21\n")
        .expect("write go.mod");
    assert!(
        ScipGoIndexer::new().detect(with_mod.path()),
        "ScipGoIndexer must detect-fire on a directory containing go.mod",
    );

    let without_mod = tempfile::tempdir().expect("tempdir");
    assert!(
        !ScipGoIndexer::new().detect(without_mod.path()),
        "ScipGoIndexer must not detect-fire without a go.mod",
    );
}

#[test]
fn run_over_fixture_yields_symbols() {
    let Some(binary) = scip_go_binary() else {
        eprintln!(
            "scip-go not installed; skipping the native-indexer run test \
             (install: go install github.com/scip-code/scip-go/cmd/scip-go@latest)",
        );
        return;
    };

    let out_dir = tempfile::tempdir().expect("tempdir");
    let out = out_dir.path().join("go.scip");

    // Pin the module version so the run does not depend on the enclosing
    // repository's VCS state; this also exercises the `--module-version`
    // override (tier-01 step 4).
    let indexer = ScipGoIndexer::with_binary(binary).with_module_version("0.0.0-ariadne-fixture");
    indexer
        .run(&fixture_root(), &out)
        .expect("scip-go must index the fixture module");

    let bytes = std::fs::read(&out).expect("scip-go must write the SCIP index");
    let doc = indexer
        .parse(&bytes)
        .expect("the emitted SCIP index must decode");

    let symbols: usize = doc.index.documents.iter().map(|d| d.symbols.len()).sum();
    assert!(
        symbols >= 1,
        "scip-go over the fixture module must emit at least one symbol; got {symbols}",
    );
}

#[test]
fn parse_decodes_synthesized_go_index() {
    // Offline coverage of `ScipGoIndexer::parse`: a synthesized SCIP index
    // — no `scip-go` toolchain involved — must decode through the driver
    // into a `ScipDoc` tagged `Lang::Go` with every symbol intact. Keeps
    // the `parse` path under test on build hosts where `scip-go` is absent
    // and `run_over_fixture_yields_symbols` early-returns (audit tier-01 F2).
    let bytes = synth_bytes(
        "scip-go",
        "demo/demo.go",
        "Go",
        &[
            SymBp {
                raw: "scip-go gomod demo 0.0.0 `demo`/Greeter#",
                occurrences: 3,
                relationships: 1,
            },
            SymBp {
                raw: "scip-go gomod demo 0.0.0 `demo`/Greeter#Greet().",
                occurrences: 2,
                relationships: 0,
            },
        ],
    );

    let doc = ScipGoIndexer::new()
        .parse(&bytes)
        .expect("synthesized SCIP bytes must decode through ScipGoIndexer::parse");

    assert_eq!(
        doc.lang,
        Lang::Go,
        "the driver must tag the decoded doc Lang::Go",
    );
    let symbols: usize = doc.index.documents.iter().map(|d| d.symbols.len()).sum();
    assert_eq!(
        symbols, 2,
        "both synthesized symbols must survive the prost decode; got {symbols}",
    );
}
