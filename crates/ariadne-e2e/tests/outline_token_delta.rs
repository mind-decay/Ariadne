//! context-efficient-read tier-04 token-delta re-measure — `#[ignore]`d,
//! deterministic harness.
//!
//! Confirms the plan's premise (a folded skeleton is far cheaper than a
//! whole-file read) with a real measurement: over a fixed set of multi-symbol
//! files in THIS repo it compares the baseline cost of a native whole-file
//! `Read` against the prototype cost of `read_outline` — the same pure
//! [`ariadne_graph::assemble`] use case the MCP tool drives — and records the
//! per-file + median response-token reduction. Target ≥50% median; reported,
//! not gated (the spike's anti-flake convention) [src:
//! .claude/plans/context-efficient-read/plan.md D8; tier-04 <steps> 6].
//!
//! It opens a byte-copy of the live `.ariadne/index.redb` (a running daemon
//! holds the lock and `open` writes on bootstrap), replicates the production
//! catalog projection over the driven `ariadne-storage` adapter — it cannot
//! link the driving `ariadne-mcp` crate (architecture rule 4) — and feeds the
//! per-file symbol spans to the assembler. Deterministic: no wall-clock, no
//! model, no network; the same index renders the same numbers.
//!
//! Byte-count casts below never approach the type bounds; the pedantic cast
//! lints are allowed locally for the throwaway measurement arithmetic.
#![allow(
    clippy::cast_possible_wrap,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use ariadne_core::{Lang, ReadSnapshot, Storage};
use ariadne_graph::{OutlineOptions, OutlineRequest, OutlineSymbol, assemble};
use ariadne_storage::RedbStorage;

/// Fixed candidate set: long-lived, multi-symbol source files in this repo. The
/// harness measures each that resolves in the index with ≥2 symbols and
/// readable bytes; a fixed floor of [`MIN_FILES`] guards against silently
/// measuring an empty set.
const CANDIDATES: &[&str] = &[
    "crates/ariadne-core/src/domain/records.rs",
    "crates/ariadne-core/src/domain/types/lang.rs",
    "crates/ariadne-cli/src/config.rs",
    "crates/ariadne-cli/src/commands/setup.rs",
    "crates/ariadne-graph/src/hotspot.rs",
    "crates/ariadne-graph/src/docgen.rs",
    "crates/ariadne-graph/src/doc_model.rs",
    "crates/ariadne-mcp/src/server.rs",
];

/// Minimum files that must resolve before the measurement is trusted.
const MIN_FILES: usize = 4;

/// Mirror the MCP `read_outline` projection options: keep private symbols and
/// cap rendered top-level symbols at the production ceiling
/// [src: crates/ariadne-mcp/src/tools/read_outline.rs `MAX_OUTLINE_SYMBOLS`].
const MAX_OUTLINE_SYMBOLS: usize = 800;

/// Per-symbol span projection the assembler consumes (the catalog name→span
/// shape, restricted to one file).
#[derive(Clone)]
struct Sym {
    name: String,
    kind: String,
    byte_start: u32,
    byte_end: u32,
    visibility: ariadne_core::Visibility,
}

/// Workspace root = `crates/ariadne-e2e/../..`, canonicalised (deterministic).
fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("canonicalise workspace root")
}

/// Format tenths-of-a-percent as `X.Y`, sign-aware.
fn fmt_tenths(t: i64) -> String {
    let a = t.unsigned_abs();
    format!("{}{}.{}", if t < 0 { "-" } else { "" }, a / 10, a % 10)
}

#[test]
#[ignore = "harness: opens the live .ariadne/index.redb; run explicitly via --run-ignored"]
#[allow(clippy::too_many_lines)] // throwaway measurement harness; the index
// projection + per-file loop read top-to-bottom (mirrors search_read_spike).
fn outline_token_delta() {
    let root = workspace_root();
    let index = root.join(".ariadne/index.redb");
    assert!(index.is_file(), "live index missing: {}", index.display());

    // Copy the live snapshot to a tempdir and open the copy (daemon holds the
    // live file's lock; `open` writes on bootstrap).
    let tmp = tempfile::tempdir().expect("tempdir");
    let copy = tmp.path().join("index.redb");
    std::fs::copy(&index, &copy).expect("copy live index");
    let storage = RedbStorage::open(&copy).expect("open copied index");
    let revision = storage.revision().0;
    let snap = storage.snapshot().expect("snapshot");

    // FileId → (path, lang), then symbols grouped by defining file (the catalog
    // projection the `read_outline` tool replicates).
    let mut files: BTreeMap<ariadne_core::FileId, (String, Lang)> = BTreeMap::new();
    for chunk in snap.iter_files(4096).expect("iter_files") {
        for (id, rec) in chunk.expect("file chunk") {
            files.insert(id, (rec.path, rec.lang));
        }
    }
    let mut by_file: BTreeMap<String, (Lang, Vec<Sym>)> = BTreeMap::new();
    for chunk in snap.iter_symbols(4096).expect("iter_symbols") {
        for (_id, rec) in chunk.expect("symbol chunk") {
            let Some((path, lang)) = files.get(&rec.defining_file) else {
                continue;
            };
            by_file
                .entry(path.clone())
                .or_insert_with(|| (*lang, Vec::new()))
                .1
                .push(Sym {
                    name: rec.canonical_name,
                    kind: rec.kind,
                    byte_start: rec.defining_span.byte_start,
                    byte_end: rec.defining_span.byte_end,
                    visibility: rec.visibility,
                });
        }
    }

    // Measure each candidate that resolves with ≥2 symbols and readable bytes,
    // in fixed (sorted) path order.
    let mut paths: Vec<&str> = CANDIDATES.to_vec();
    paths.sort_unstable();

    let mut rows = String::new();
    let mut reductions: Vec<i64> = Vec::new();
    for rel in paths {
        let Some((lang, syms)) = by_file.get(rel) else {
            continue;
        };
        if syms.len() < 2 {
            continue;
        }
        let Ok(bytes) = std::fs::read(root.join(rel)) else {
            continue;
        };
        let baseline = bytes.len() as u64; // whole-file `Read`.
        assert!(baseline > 0, "empty source file `{rel}`");

        let req = OutlineRequest {
            source: bytes,
            symbols: syms
                .iter()
                .map(|s| OutlineSymbol {
                    name: s.name.clone(),
                    kind: s.kind.clone(),
                    byte_start: s.byte_start,
                    byte_end: s.byte_end,
                    visibility: s.visibility,
                })
                .collect(),
            lang: *lang,
            options: OutlineOptions {
                include_private: true,
                max_symbols: MAX_OUTLINE_SYMBOLS,
            },
        };
        let proto = assemble(&req).skeleton.len() as u64; // `read_outline` output.

        let reduction = ((baseline as i64 - proto as i64) * 1000) / baseline as i64;
        reductions.push(reduction);
        writeln!(
            rows,
            "| `{}` | {} | {} | {} | {} | {} | {}% |",
            rel,
            syms.len(),
            baseline,
            baseline / 4,
            proto,
            proto / 4,
            fmt_tenths(reduction),
        )
        .unwrap();
    }

    assert!(
        reductions.len() >= MIN_FILES,
        "only {} of {} candidate files resolved in the index (need ≥{MIN_FILES}); \
         is the index current?",
        reductions.len(),
        CANDIDATES.len(),
    );

    reductions.sort_unstable();
    let n = reductions.len();
    let median = if n % 2 == 1 {
        reductions[n / 2]
    } else {
        (reductions[n / 2 - 1] + reductions[n / 2]) / 2
    };

    let report = render_report(revision, n, &rows, median);
    let out = root.join(".claude/plans/context-efficient-read/outline-token-delta.md");
    std::fs::write(&out, &report).expect("write report");

    assert!(report.contains("Median reduction"), "report missing median");
    assert!(
        report.contains(&revision.to_string()),
        "report missing revision"
    );
}

/// Render the deterministic markdown report (no timestamp / wall-clock).
fn render_report(revision: u64, files: usize, rows: &str, median: i64) -> String {
    let verdict = if median >= 500 {
        format!(
            "**Target met.** Median reduction {}% ≥ 50% (D8); reported, not gated.",
            fmt_tenths(median)
        )
    } else {
        format!(
            "**Below target.** Median reduction {}% < 50% (D8); reported, not gated.",
            fmt_tenths(median)
        )
    };
    format!(
        "# Token-delta: whole-file Read vs read_outline\n\n\
         Generated by `crates/ariadne-e2e/tests/outline_token_delta.rs` (`#[ignore]`d).\n\
         Deterministic data artifact — no timestamp, no wall-clock, no model call.\n\n\
         ## Snapshot\n\n\
         - Index revision: `{revision}`\n\
         - Files measured: {files}\n\n\
         ## Method\n\n\
         - **Baseline** (no Ariadne): bytes of a native whole-file `Read`.\n\
         - **Prototype** (`read_outline`): bytes of the folded skeleton from the pure\n\
         `ariadne_graph::assemble` use case the MCP tool drives (include_private=true,\n\
         max_symbols={MAX_OUTLINE_SYMBOLS}).\n\
         - **Token proxy**: `tokens = bytes / 4`; raw bytes reported alongside so the\n\
         relative delta does not hinge on the divisor (it cancels).\n\n\
         ## Per-file cost\n\n\
         | File | Symbols | Baseline bytes | Baseline tok | Outline bytes | Outline tok | Reduction |\n\
         |------|--------:|---------------:|-------------:|--------------:|------------:|----------:|\n\
         {rows}\n\
         ## Result\n\n\
         Median reduction across {files} files: **{median}%**.\n\n\
         {verdict}\n",
        median = fmt_tenths(median),
    )
}
