//! tier-06 search/read spike — THROWAWAY, `#[ignore]`d harness.
//!
//! Measures the deterministic response-token delta between the reflexive
//! `grep` + whole-file-`Read` path and a `search-hit` + exact-span path, to
//! gate tiers 07–09 (plan.md D11). It opens a byte-copy of THIS repo's live
//! `.ariadne/index.redb`, replicates the production mcp `Catalog` projection
//! over the driven `ariadne-storage` adapter (it cannot link the driving
//! `ariadne-mcp` crate — architecture rule 4), runs a fixed task set, and
//! writes `spike-search-read.md`.
//!
//! Deviation note (recorded for audit): the plan's step 2 says to open
//! `.ariadne/index.redb` directly. A live daemon holds that file's redb lock
//! and `RedbStorage::open` writes on open (`bootstrap`), so a direct open
//! would both fail and mutate the live index. The harness instead copies the
//! file to a tempdir and opens the copy — byte-identical data, same revision,
//! a real run, no live-index mutation.
//!
//! Numeric casts below are between `u64`/`i64` over byte counts that never
//! approach the type bounds; pedantic cast lints are allowed locally for the
//! throwaway measurement arithmetic.
#![allow(
    clippy::cast_possible_wrap,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::naive_bytecount
)]

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use ariadne_core::{ReadSnapshot, Storage};
use ariadne_storage::RedbStorage;
use regex::RegexBuilder;
use serde_json::json;

/// How a task's prototype arm searches the symbol map.
#[derive(Clone, Copy, Debug)]
enum Search {
    /// Lowercase `contains`, like `list_symbols`.
    Substr(&'static str),
    /// Symbol-name regex, like the planned `search_code`.
    Regex(&'static str),
}

/// One fixed measurement task over real symbols in this repo.
#[derive(Clone, Copy, Debug)]
struct Task {
    /// Human intent label.
    intent: &'static str,
    /// One of the three shapes: find-definition / search-by-pattern / read-body.
    shape: &'static str,
    /// Plain substring a reflexive agent would feed `grep` (baseline arm).
    grep: &'static str,
    /// Symbol search the prototype arm runs.
    search: Search,
    /// Canonical name the agent ultimately inspects (must resolve).
    target: &'static str,
}

/// 10 fixed tasks spanning the three shapes; each `target` is asserted to
/// resolve in the live catalog (no fabricated row).
const TASKS: &[Task] = &[
    Task {
        intent: "find-def Catalog",
        shape: "find-definition",
        grep: "Catalog",
        search: Search::Substr("Catalog"),
        target: "Catalog",
    },
    Task {
        intent: "find-def summarize",
        shape: "find-definition",
        grep: "summarize",
        search: Search::Substr("summarize"),
        target: "summarize",
    },
    Task {
        intent: "find-def RedbStorage",
        shape: "find-definition",
        grep: "RedbStorage",
        search: Search::Substr("RedbStorage"),
        target: "RedbStorage",
    },
    Task {
        intent: "find-def SymbolSummary",
        shape: "find-definition",
        grep: "SymbolSummary",
        search: Search::Substr("SymbolSummary"),
        target: "SymbolSummary",
    },
    Task {
        intent: "pattern ^handle",
        shape: "search-by-pattern",
        grep: "handle",
        search: Search::Regex("^handle"),
        target: "handle",
    },
    Task {
        intent: "pattern _report$",
        shape: "search-by-pattern",
        grep: "report",
        search: Search::Regex("_report$"),
        target: "co_change_report",
    },
    Task {
        intent: "pattern ^iter_",
        shape: "search-by-pattern",
        grep: "iter_",
        search: Search::Regex("^iter_"),
        target: "iter_files",
    },
    Task {
        intent: "pattern ^doc_for",
        shape: "search-by-pattern",
        grep: "doc_for",
        search: Search::Regex("^doc_for"),
        target: "doc_for",
    },
    Task {
        intent: "read-body build",
        shape: "read-body",
        grep: "build",
        search: Search::Substr("build"),
        target: "build",
    },
    Task {
        intent: "read-body find_symbol",
        shape: "read-body",
        grep: "find_symbol",
        search: Search::Substr("find_symbol"),
        target: "find_symbol",
    },
];

/// Production `list_symbols` default cap.
const SEARCH_LIMIT: usize = 64;
/// Context lines added each side of a span in the prototype read arm (`±N`, D9).
const CTX_LINES: usize = 3;

/// Per-symbol metadata the spike needs (the catalog name→span projection).
#[derive(Clone, Debug)]
struct Lite {
    name: String,
    kind: String,
    path: String,
    byte_start: usize,
    byte_end: usize,
}

/// 0-based index of the line containing `off` = newline count in `bytes[..off]`.
fn line_index(bytes: &[u8], off: usize) -> usize {
    let end = off.min(bytes.len());
    bytes[..end].iter().filter(|&&b| b == b'\n').count()
}

/// True if `line` contains the `needle` byte-substring (non-empty).
fn line_has(line: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty() && line.windows(needle.len()).any(|w| w == needle)
}

/// Baseline grep cost over one file: Σ bytes of lines containing `needle`
/// (matching-line text only — no `file:line:` prefix, a conservative
/// understatement of what `grep -rn` actually emits).
fn grep_line_bytes(content: &[u8], needle: &[u8]) -> u64 {
    content
        .split(|&b| b == b'\n')
        .filter(|line| line_has(line, needle))
        .map(|line| line.len() as u64)
        .sum()
}

/// Prototype read cost: bytes of `[line(byte_start) − CTX .. line(byte_end) + CTX]`.
fn body_with_context(content: &[u8], byte_start: usize, byte_end: usize) -> u64 {
    let lines: Vec<&[u8]> = content.split(|&b| b == b'\n').collect();
    if lines.is_empty() {
        return 0;
    }
    let lo = line_index(content, byte_start).saturating_sub(CTX_LINES);
    let hi = (line_index(content, byte_end) + CTX_LINES).min(lines.len() - 1);
    let body: u64 = lines[lo..=hi].iter().map(|l| l.len() as u64).sum();
    body + (hi - lo) as u64 // re-add the joining newlines
}

/// Run the prototype search arm; serialise hits in the `SymbolSummary` shape
/// (name, kind, file, 1-based line range), capped at `SEARCH_LIMIT`.
fn search_hits_json(
    symbols: &[Lite],
    search: Search,
    contents: &BTreeMap<String, Vec<u8>>,
) -> String {
    let regex = match search {
        Search::Regex(re) => Some(RegexBuilder::new(re).build().expect("valid spike regex")),
        Search::Substr(_) => None,
    };
    let mut hits = Vec::new();
    for sym in symbols {
        let matched = match search {
            Search::Substr(q) => sym.name.to_lowercase().contains(&q.to_lowercase()),
            Search::Regex(_) => regex.as_ref().is_some_and(|r| r.is_match(&sym.name)),
        };
        if !matched {
            continue;
        }
        let (ls, le) = contents.get(&sym.path).map_or((0, 0), |c| {
            (
                line_index(c, sym.byte_start) + 1,
                line_index(c, sym.byte_end) + 1,
            )
        });
        hits.push(json!({
            "name": sym.name,
            "kind": sym.kind,
            "file": sym.path,
            "line_start": ls,
            "line_end": le,
        }));
        if hits.len() >= SEARCH_LIMIT {
            break;
        }
    }
    serde_json::Value::Array(hits).to_string()
}

/// Format tenths-of-a-percent as `X.Y`, sign-aware.
fn fmt_tenths(t: i64) -> String {
    let a = t.unsigned_abs();
    format!("{}{}.{}", if t < 0 { "-" } else { "" }, a / 10, a % 10)
}

/// Workspace root = `crates/ariadne-e2e/../..`, canonicalised (deterministic).
fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("canonicalise workspace root")
}

#[test]
#[ignore = "spike: opens the live .ariadne/index.redb; run explicitly via --run-ignored"]
fn search_read_spike() {
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

    // Replicate the catalog projection: FileId→path, then symbols in SymbolId
    // order (so by-name `.first()` == production `find_symbol`).
    let mut paths: BTreeMap<ariadne_core::FileId, String> = BTreeMap::new();
    for chunk in snap.iter_files(4096).expect("iter_files") {
        for (id, rec) in chunk.expect("file chunk") {
            paths.insert(id, rec.path);
        }
    }
    let mut symbols: Vec<Lite> = Vec::new();
    let mut by_name: BTreeMap<String, usize> = BTreeMap::new();
    for chunk in snap.iter_symbols(4096).expect("iter_symbols") {
        for (_id, rec) in chunk.expect("symbol chunk") {
            let path = paths.get(&rec.defining_file).cloned().unwrap_or_default();
            let idx = symbols.len();
            by_name.entry(rec.canonical_name.clone()).or_insert(idx);
            symbols.push(Lite {
                name: rec.canonical_name,
                kind: rec.kind,
                path,
                byte_start: rec.defining_span.byte_start as usize,
                byte_end: rec.defining_span.byte_end as usize,
            });
        }
    }
    assert!(!paths.is_empty(), "snapshot has no files");
    assert!(!symbols.is_empty(), "snapshot has no symbols");

    // Read every indexed source file once (corpus both arms operate over).
    let mut contents: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for path in paths.values() {
        if let Ok(bytes) = std::fs::read(root.join(path)) {
            contents.insert(path.clone(), bytes);
        }
    }

    // Per-task measurement.
    let mut rows = String::new();
    let mut reductions: Vec<i64> = Vec::with_capacity(TASKS.len());
    for task in TASKS {
        let idx = *by_name
            .get(task.target)
            .unwrap_or_else(|| panic!("target `{}` not in catalog", task.target));
        let sym = &symbols[idx];
        let target_src = contents
            .get(&sym.path)
            .unwrap_or_else(|| panic!("target file `{}` unreadable", sym.path));

        // Baseline: grep matching-line text across the corpus + whole-file Read.
        let grep_bytes: u64 = contents
            .values()
            .map(|c| grep_line_bytes(c, task.grep.as_bytes()))
            .sum();
        let baseline = grep_bytes + target_src.len() as u64;

        // Prototype: serialised search hits + exact span ±3 context lines.
        let hits_bytes = search_hits_json(&symbols, task.search, &contents).len() as u64;
        let proto = hits_bytes + body_with_context(target_src, sym.byte_start, sym.byte_end);

        assert!(baseline > 0, "baseline cost is zero for `{}`", task.intent);
        let reduction = ((baseline as i64 - proto as i64) * 1000) / baseline as i64;
        reductions.push(reduction);

        writeln!(
            rows,
            "| {} | {} | {} | `{}` | {} | {} | {} | {} | {}% |",
            task.intent,
            task.shape,
            search_label(task.search),
            sym.name,
            baseline,
            baseline / 4,
            proto,
            proto / 4,
            fmt_tenths(reduction),
        )
        .unwrap();
    }

    reductions.sort_unstable();
    let n = reductions.len();
    let median = (reductions[n / 2 - 1] + reductions[n / 2]) / 2;

    let report = render_report(revision, paths.len(), symbols.len(), &rows, median);
    let out = root.join(".claude/plans/ariadne-mcp-adoption/spike-search-read.md");
    std::fs::write(&out, &report).expect("write report");

    assert!(!report.is_empty(), "report is empty");
    assert!(
        report.contains("Median reduction"),
        "report missing median line"
    );
    assert!(report.contains("Verdict"), "report missing verdict line");
    assert!(
        report.contains(&revision.to_string()),
        "report missing revision"
    );
}

/// Short label for the search arm shown in the table.
fn search_label(s: Search) -> String {
    match s {
        Search::Substr(q) => format!("substr `{q}`"),
        Search::Regex(re) => format!("regex `{re}`"),
    }
}

/// Render the deterministic markdown report (no timestamp / wall-clock).
fn render_report(revision: u64, files: usize, syms: usize, rows: &str, median: i64) -> String {
    let verdict = if median >= 400 {
        format!(
            "**Verdict: PROCEED to tiers 07–09.** Median reduction {}% ≥ 40% threshold (D11).",
            fmt_tenths(median)
        )
    } else {
        format!(
            "**Verdict: CANCEL tiers 07–09.** Median reduction {}% < 40% threshold (D11).",
            fmt_tenths(median)
        )
    };
    format!(
        "# Spike: search + read vs grep + whole-file Read\n\n\
         Generated by `crates/ariadne-e2e/tests/search_read_spike.rs` (throwaway, `#[ignore]`d).\n\
         Deterministic data artifact — no timestamp, no wall-clock, no model call.\n\n\
         ## Snapshot\n\n\
         - Index revision: `{revision}`\n\
         - Indexed files: {files}\n\
         - Indexed symbols: {syms}\n\n\
         ## Method\n\n\
         - **Baseline** (reflexive, no Ariadne): Σ bytes of `grep` matching-line text for the plain\n\
         query across all indexed source files, plus a whole-file `Read` of the file defining the\n\
         target symbol. Models the line-range-unaware reflexive path D11 targets; matching-line text\n\
         only (no `file:line:` prefix), so the baseline is conservatively understated.\n\
         - **Prototype** (search + span): JSON-serialised symbol search hits in the `SymbolSummary`\n\
         shape (name, kind, file, 1-based line range), capped at {SEARCH_LIMIT}, plus the target's\n\
         exact span widened by ±{CTX_LINES} context lines (D9 `context(±N)`).\n\
         - **Token proxy**: `tokens = bytes / 4` (OpenAI English rule of thumb); raw bytes reported\n\
         alongside so the verdict does not hinge on the divisor — the gate is a relative delta whose\n\
         scaling cancels.\n\
         - Both arms operate over the same indexed-file corpus; targets are asserted to resolve.\n\n\
         ## Per-task cost\n\n\
         | Intent | Shape | Search | Resolved symbol | Baseline bytes | Baseline tok | Proto bytes | Proto tok | Reduction |\n\
         |--------|-------|--------|-----------------|---------------:|-------------:|------------:|----------:|----------:|\n\
         {rows}\n\
         ## Result\n\n\
         Median reduction across {task_count} tasks: **{median}%**.\n\n\
         {verdict}\n",
        median = fmt_tenths(median),
        task_count = TASKS.len(),
    )
}
