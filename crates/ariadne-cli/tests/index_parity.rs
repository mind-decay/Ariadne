//! Cold byte-parity gate for the shared per-file derivation (tier-07a step 6).
//!
//! `ariadne index` is run over each in-crate framework fixture (the strictest
//! derivation path: decls + calls + renders + hooks + the synthesized SFC
//! `Component` symbol + cross-file edges) and the persisted redb records
//! (files / symbols / edges, sorted) are dumped to a canonical text form and
//! compared against a committed golden. The golden is the pre-refactor CLI
//! output: it was generated from the streaming-committer cold index and then
//! proven byte-identical to the shared-derivation output, so any future drift
//! between the two paths fails here [src: post-v1-roadmap plan.md RD11].
//!
//! `mtime_ns` is excluded from the dump — it is per-run file metadata, not
//! derivation output, so two indexes of the same fixture copied at different
//! instants legitimately differ on it.
//!
//! Two test seams:
//! * `ARIADNE_PARITY_BIN` overrides the indexed binary — point it at a
//!   pre-refactor build to prove old output == golden.
//! * `UPDATE_GOLDENS=1` rewrites the goldens from the current binary.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use ariadne_core::{ReadSnapshot, Storage};
use ariadne_storage::RedbStorage;
use pretty_assertions::assert_eq;

/// Default binary under test; `ARIADNE_PARITY_BIN` overrides it so the same
/// canonical dump can be run against a pre-refactor build.
fn bin() -> String {
    std::env::var("ARIADNE_PARITY_BIN").unwrap_or_else(|_| env!("CARGO_BIN_EXE_ariadne").to_owned())
}

/// Root of the in-crate framework fixture trees.
const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures");
/// Committed golden directory.
const GOLDENS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/goldens");

/// Recursively copy `src` into `dst`, creating `dst` and any sub-dirs.
fn copy_tree(src: &Path, dst: &Path) {
    std::fs::create_dir_all(dst).expect("create destination dir");
    for entry in std::fs::read_dir(src).expect("read fixture dir") {
        let entry = entry.expect("fixture dir entry");
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if entry.file_type().expect("fixture entry file type").is_dir() {
            copy_tree(&from, &to);
        } else {
            std::fs::copy(&from, &to).expect("copy fixture file");
        }
    }
}

/// Run `<bin> <args...>`; fail unless it exits successfully.
fn run_ok(args: &[&str]) {
    let output = Command::new(bin())
        .args(args)
        .output()
        .unwrap_or_else(|e| panic!("spawn `ariadne {}`: {e}", args.join(" ")));
    assert!(
        output.status.success(),
        "`ariadne {}` exited with {}: {}",
        args.join(" "),
        output.status,
        String::from_utf8_lossy(&output.stderr),
    );
}

/// Lowercase hex of a 32-byte hash.
fn hex(bytes: [u8; 32]) -> String {
    let mut s = String::with_capacity(64);
    for b in bytes {
        let _ = write!(s, "{b:02x}");
    }
    s
}

/// Canonical, deterministic dump of every persisted record, sorted by id /
/// key. `mtime_ns` is intentionally omitted (per-run file metadata).
fn dump_index(redb: &Path) -> String {
    let storage = RedbStorage::open(redb).expect("open redb index");
    let snapshot = storage.snapshot().expect("open read snapshot");

    let mut files: Vec<_> = snapshot
        .iter_files(4096)
        .expect("stream files")
        .flat_map(|chunk| chunk.expect("decode file chunk"))
        .collect();
    files.sort_by_key(|(id, _)| id.get());

    let mut symbols: Vec<_> = snapshot
        .iter_symbols(4096)
        .expect("stream symbols")
        .flat_map(|chunk| chunk.expect("decode symbol chunk"))
        .collect();
    symbols.sort_by_key(|(id, _)| id.get());

    let mut edges: Vec<_> = snapshot
        .iter_edges(4096)
        .expect("stream edges")
        .flat_map(|chunk| chunk.expect("decode edge chunk"))
        .collect();
    edges.sort_by_key(|(key, _)| key.to_bytes());

    let mut out = String::new();
    out.push_str("== FILES ==\n");
    for (id, r) in &files {
        let _ = writeln!(
            out,
            "{}\t{}\t{}\t{}\t{}",
            id.get(),
            r.path,
            r.lang.tag(),
            r.size,
            hex(r.blake3),
        );
    }
    out.push_str("== SYMBOLS ==\n");
    for (id, r) in &symbols {
        let _ = writeln!(
            out,
            "{}\t{}\t{}\tfile={}\tspan={}:{}-{}\tvis={}\tattrs={:?}",
            id.get(),
            r.canonical_name,
            r.kind,
            r.defining_file.get(),
            r.defining_span.file.get(),
            r.defining_span.byte_start,
            r.defining_span.byte_end,
            r.visibility.to_byte(),
            r.attributes,
        );
    }
    out.push_str("== EDGES ==\n");
    for (k, r) in &edges {
        let _ = writeln!(
            out,
            "{}\t{}\t{}\tspan={}:{}-{}\tlang={}\tw={}",
            k.src.get(),
            k.kind.to_byte(),
            k.dst.get(),
            r.source_span.file.get(),
            r.source_span.byte_start,
            r.source_span.byte_end,
            r.evidence_lang.tag(),
            r.weight,
        );
    }
    out
}

/// Copy `family`'s fixture into a tempdir, `init` + `index` it, dump the
/// persisted records, and compare against (or regenerate) the golden.
fn check_family(family: &str) {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path();
    copy_tree(&Path::new(FIXTURES).join(family), root);

    run_ok(&["init", root.to_str().expect("utf8 root")]);
    // `--no-scip` pins the syntactic-only derivation: SCIP is default-on
    // (scip-driven-edges D6), so the goldens — which capture the tree-sitter
    // resolver output — must opt out to stay stable regardless of which external
    // indexers are installed [src: docs/adr/0026-default-on-out-of-band-scip.md].
    run_ok(&["index", "--no-scip", root.to_str().expect("utf8 root")]);

    let actual = dump_index(&root.join(".ariadne").join("index.redb"));
    let golden_path = PathBuf::from(GOLDENS).join(format!("parity_{family}.txt"));

    if std::env::var_os("UPDATE_GOLDENS").is_some() {
        std::fs::create_dir_all(golden_path.parent().expect("golden parent"))
            .expect("create goldens dir");
        std::fs::write(&golden_path, &actual).expect("write golden");
        return;
    }

    let golden = std::fs::read_to_string(&golden_path).unwrap_or_else(|e| {
        panic!(
            "missing golden {} ({e}); regenerate with UPDATE_GOLDENS=1",
            golden_path.display()
        )
    });
    assert_eq!(
        golden, actual,
        "cold-index records for `{family}` diverged from the golden",
    );
}

#[test]
fn parity_vue() {
    check_family("vue");
}

#[test]
fn parity_svelte() {
    check_family("svelte");
}

#[test]
fn parity_astro() {
    check_family("astro");
}

#[test]
fn parity_react() {
    check_family("react");
}

// The 7 single-language fixtures (the exit criterion's "7-language" half): each
// is a callee + caller pair whose derivation exercises decls + a cross-file call
// edge on the plain (non-SFC) path. Goldens are the syntactic-only cold index
// (`index --no-scip`), so they are stable regardless of which external SCIP
// indexers happen to be installed [src: docs/adr/0026-default-on-out-of-band-scip.md].

#[test]
fn parity_rust() {
    check_family("rust");
}

#[test]
fn parity_typescript() {
    check_family("typescript");
}

#[test]
fn parity_python() {
    check_family("python");
}

#[test]
fn parity_go() {
    check_family("go");
}

#[test]
fn parity_java() {
    check_family("java");
}

#[test]
fn parity_csharp() {
    check_family("csharp");
}

#[test]
fn parity_c() {
    check_family("c");
}
