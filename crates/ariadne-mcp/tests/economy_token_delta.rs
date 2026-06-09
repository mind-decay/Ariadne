//! Block 1 tier-05 — response-economy token-delta harness, `#[ignore]`d,
//! deterministic.
//!
//! Confirms the block's premise (a default page cap + concise verbosity bounds
//! every growable tool well under the 25k-token MCP cap) with a real
//! measurement: over a byte-copy of the live self-index it drives each of the
//! ten capped cold `tools::*::handle` twice — a baseline (`verbosity:detailed`,
//! `limit:u32::MAX`, no cursor → the un-capped result) and the default budget
//! (`verbosity:concise`, default cap, no cursor) — and records the per-tool
//! response-byte reduction. It then asserts every default page is ≤25k tokens
//! (BR6); a failure means lower that tool's default `limit`, never weaken the
//! assertion [src: .claude/plans/data-fidelity-arc/block-1/tier-05-harness-advisory.md
//! <steps> 1-2; plan.md D4,BR6; context-efficient-read tier-04 harness precedent].
//!
//! This drives the REAL cold handlers (the `ariadne-mcp` crate's own tools), so
//! the ≤25k assertion is over the shipped output, not a re-derivation. It lives
//! in `ariadne-mcp/tests/` because the architecture rule bars `ariadne-e2e` from
//! linking the driving `ariadne-mcp` crate [src: tests/architecture.rs:121-140].
//!
//! Deterministic: it opens a byte-copy of `.ariadne/index.redb` (a running
//! daemon holds the live file's lock and `open` writes on bootstrap), builds the
//! same cold [`Catalog`] the production server builds, and feeds fixed inputs.
//! No wall-clock, no model, no network; the same index renders the same numbers.
//! The token proxy is `bytes / 4` (raw bytes reported alongside so the relative
//! delta does not hinge on the divisor — it cancels), mirroring the
//! `outline_token_delta` harness method.
//!
//! Byte-count casts below never approach the type bounds, and the per-tool
//! `<x>_base` / `<x>_def` binding pairs are a deliberate, readable convention;
//! the pedantic cast + naming lints are allowed locally for this throwaway
//! measurement harness.
#![allow(
    clippy::cast_possible_wrap,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::too_many_lines,
    clippy::similar_names
)]

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use ariadne_core::LineHunk;
use ariadne_mcp::Catalog;
use ariadne_mcp::tools;
use ariadne_mcp::types::{
    BlastRadiusInput, CoChangeInput, CouplingInput, FindReferencesInput, Grain, GrainScopeInput,
    RefactorInput, Verbosity, WeakSpotsInput,
};
use ariadne_storage::RedbStorage;

/// MCP per-tool result token budget. A default page that exceeds it is a BR6
/// failure: lower the tool's default `limit`, never raise this cap.
const TOKEN_CAP: usize = 25_000;

/// A hot, stable core symbol (~165 reference sites) — drives `find_references`
/// and `blast_radius` to a large un-capped baseline. The harness asserts it
/// resolves, so a name drift fails loudly rather than silently measuring an
/// empty set.
const HOT_SYMBOL: &str = "SymbolId";

/// A representative single-file changeset for the two diff-aware tools: a small,
/// stable, low-fan-in source file. Its symbols seed a bounded blast radius, so
/// the diff tools' default page stays well under the cap while still exercising
/// the real cold join (git-free: the hunks are synthesized deterministically,
/// never read from the working tree's HEAD diff).
const DIFF_PATHS: &[&str] = &["crates/ariadne-graph/src/roots.rs"];

/// One measured tool: its name plus the un-capped baseline and default-budget
/// response sizes in bytes.
struct Row {
    tool: &'static str,
    baseline: usize,
    default: usize,
}

/// Workspace root = `crates/ariadne-mcp/../..`, canonicalised (deterministic).
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

/// Serialize a tool's baseline + default outputs (the exact `wire` projection —
/// `serde_json::to_string` into the single text content block) and record their
/// byte sizes. Both arguments are the same output type, so concise field
/// omission shows up purely as a smaller `default` serialization.
fn measure<T: serde::Serialize>(tool: &'static str, baseline: &T, default: &T) -> Row {
    Row {
        tool,
        baseline: serde_json::to_string(baseline)
            .expect("serialize baseline")
            .len(),
        default: serde_json::to_string(default)
            .expect("serialize default")
            .len(),
    }
}

#[test]
#[ignore = "harness: opens the live .ariadne/index.redb; run explicitly via --run-ignored"]
fn economy_token_delta() {
    let root = workspace_root();
    let index = root.join(".ariadne/index.redb");
    assert!(index.is_file(), "live index missing: {}", index.display());

    // Copy the live snapshot to a tempdir and open the copy (a running daemon
    // holds the live file's lock; `open` writes on bootstrap).
    let tmp = tempfile::tempdir().expect("tempdir");
    let copy = tmp.path().join("index.redb");
    std::fs::copy(&index, &copy).expect("copy live index");
    let storage = RedbStorage::open(&copy).expect("open copied index");
    let cat = Catalog::build(&storage, root.display().to_string()).expect("build cold catalog");
    let revision = cat.revision;

    assert!(
        cat.find_symbol(HOT_SYMBOL).is_some(),
        "hot symbol `{HOT_SYMBOL}` absent from the index; pick another or refresh the index",
    );

    // Synthetic full-file hunks for the diff-aware tools: `[1, u32::MAX]` covers
    // every line, so every symbol defined in each changed file becomes a seed.
    // The handlers re-read the on-disk bytes and drop a file whose hash diverged
    // from the index, so a mid-edit working tree degrades to `unresolved`, never
    // a wrong seed.
    let hunks: Vec<LineHunk> = DIFF_PATHS
        .iter()
        .map(|p| LineHunk {
            path: (*p).to_owned(),
            start_line: 1,
            end_line: u32::MAX,
        })
        .collect();
    let changed: Vec<String> = DIFF_PATHS.iter().map(|p| (*p).to_owned()).collect();

    let base_limit = Some(u32::MAX);

    let mut rows: Vec<Row> = Vec::new();

    // 1. find_references — single list of reference sites for a hot symbol.
    let fr_base = tools::find_references::handle(
        &cat,
        &storage,
        &FindReferencesInput {
            symbol: HOT_SYMBOL.into(),
            limit: base_limit,
            cursor: None,
            verbosity: Verbosity::Detailed,
        },
    )
    .expect("find_references baseline");
    let fr_def = tools::find_references::handle(
        &cat,
        &storage,
        &FindReferencesInput {
            symbol: HOT_SYMBOL.into(),
            limit: None,
            cursor: None,
            verbosity: Verbosity::Concise,
        },
    )
    .expect("find_references default");
    rows.push(measure("find_references", &fr_base, &fr_def));

    // 2. blast_radius — two lists (must/may) of dependents of a hot symbol.
    let br_base = tools::blast_radius::handle(
        &cat,
        &BlastRadiusInput {
            symbol: HOT_SYMBOL.into(),
            depth: None,
            kinds: None,
            limit: base_limit,
            cursor: None,
            verbosity: Verbosity::Detailed,
        },
    )
    .expect("blast_radius baseline");
    let br_def = tools::blast_radius::handle(
        &cat,
        &BlastRadiusInput {
            symbol: HOT_SYMBOL.into(),
            depth: None,
            kinds: None,
            limit: None,
            cursor: None,
            verbosity: Verbosity::Concise,
        },
    )
    .expect("blast_radius default");
    rows.push(measure("blast_radius", &br_base, &br_def));

    // 3. coupling_report — per-file Martin metrics (metric-only: concise ==
    // detailed; the cap is the sole economy win).
    let cp_base = tools::coupling_report::handle(
        &cat,
        &CouplingInput {
            prefix: None,
            limit: base_limit,
            cursor: None,
            verbosity: Verbosity::Detailed,
        },
    )
    .expect("coupling_report baseline");
    let cp_def = tools::coupling_report::handle(
        &cat,
        &CouplingInput {
            prefix: None,
            limit: None,
            cursor: None,
            verbosity: Verbosity::Concise,
        },
    )
    .expect("coupling_report default");
    rows.push(measure("coupling_report", &cp_base, &cp_def));

    // 4. weak_spots — three lists (cycles / god modules / dead code).
    let ws_base = tools::weak_spots::handle(
        &cat,
        &WeakSpotsInput {
            prefix: None,
            limit: base_limit,
            cursor: None,
            verbosity: Verbosity::Detailed,
        },
    )
    .expect("weak_spots baseline");
    let ws_def = tools::weak_spots::handle(
        &cat,
        &WeakSpotsInput {
            prefix: None,
            limit: None,
            cursor: None,
            verbosity: Verbosity::Concise,
        },
    )
    .expect("weak_spots default");
    rows.push(measure("weak_spots", &ws_base, &ws_def));

    // 5. co_change — the worst pre-block offender at low thresholds (733k tok).
    let cc = |limit, verbosity| CoChangeInput {
        prefix: None,
        min_revs: Some(1),
        min_shared_commits: Some(1),
        min_degree: Some(0.0),
        limit,
        cursor: None,
        verbosity,
    };
    let cc_base = tools::co_change::handle(&cat, &cc(base_limit, Verbosity::Detailed))
        .expect("co_change baseline");
    let cc_def =
        tools::co_change::handle(&cat, &cc(None, Verbosity::Concise)).expect("co_change default");
    rows.push(measure("co_change", &cc_base, &cc_def));

    // 6. hotspots (symbol grain) — the 311k offender; concise drops the embedded
    // symbol's id/offsets.
    let hs = |limit, verbosity| GrainScopeInput {
        prefix: None,
        grain: Grain::Symbol,
        limit,
        cursor: None,
        verbosity,
    };
    let hs_base =
        tools::hotspots::handle(&cat, &hs(base_limit, Verbosity::Detailed)).expect("hotspots base");
    let hs_def =
        tools::hotspots::handle(&cat, &hs(None, Verbosity::Concise)).expect("hotspots default");
    rows.push(measure("hotspots", &hs_base, &hs_def));

    // 7. complexity (symbol grain) — the 291k offender.
    let cx = |limit, verbosity| GrainScopeInput {
        prefix: None,
        grain: Grain::Symbol,
        limit,
        cursor: None,
        verbosity,
    };
    let cx_base = tools::complexity::handle(&cat, &cx(base_limit, Verbosity::Detailed))
        .expect("complexity base");
    let cx_def =
        tools::complexity::handle(&cat, &cx(None, Verbosity::Concise)).expect("complexity default");
    rows.push(measure("complexity", &cx_base, &cx_def));

    // 8. refactor_suggestions — three lists (god / cycle-break / misplaced);
    // metric-only rows, so concise == detailed.
    let rf_base = tools::refactor::handle(
        &cat,
        &storage,
        &RefactorInput {
            prefix: None,
            limit: base_limit,
            cursor: None,
            verbosity: Verbosity::Detailed,
        },
    )
    .expect("refactor baseline");
    let rf_def = tools::refactor::handle(
        &cat,
        &storage,
        &RefactorInput {
            prefix: None,
            limit: None,
            cursor: None,
            verbosity: Verbosity::Concise,
        },
    )
    .expect("refactor default");
    rows.push(measure("refactor_suggestions", &rf_base, &rf_def));

    // 9. diff_blast_radius — nested shape (seeds + aggregate must/may) over the
    // synthetic single-file changeset.
    let db_base = tools::diff_blast::handle(
        &cat,
        &storage,
        &root,
        &hunks,
        &changed,
        None,
        None,
        base_limit,
        None,
        Verbosity::Detailed,
    )
    .expect("diff_blast baseline");
    let db_def = tools::diff_blast::handle(
        &cat,
        &storage,
        &root,
        &hunks,
        &changed,
        None,
        None,
        None,
        None,
        Verbosity::Concise,
    )
    .expect("diff_blast default");
    rows.push(measure("diff_blast_radius", &db_base, &db_def));

    // 10. affected_tests — two lists (tests / seeds) over the same changeset.
    let at_base = tools::affected_tests::handle(
        &cat,
        &storage,
        &root,
        &hunks,
        &changed,
        None,
        None,
        base_limit,
        None,
        Verbosity::Detailed,
    )
    .expect("affected_tests baseline");
    let at_def = tools::affected_tests::handle(
        &cat,
        &storage,
        &root,
        &hunks,
        &changed,
        None,
        None,
        None,
        None,
        Verbosity::Concise,
    )
    .expect("affected_tests default");
    rows.push(measure("affected_tests", &at_base, &at_def));

    assert_eq!(
        rows.len(),
        10,
        "every one of the 10 growable tools is measured"
    );

    // Assert the default page of every tool is within the 25k-token cap (BR6),
    // then record the per-tool reductions + median.
    let mut table = String::new();
    let mut reductions: Vec<i64> = Vec::new();
    for r in &rows {
        let default_tokens = r.default / 4;
        assert!(
            default_tokens <= TOKEN_CAP,
            "tool `{}` default page is {default_tokens} tokens (> {TOKEN_CAP}); \
             lower its default `limit`, do not weaken this assertion (BR6)",
            r.tool,
        );
        let reduction = if r.baseline == 0 {
            0
        } else {
            ((r.baseline as i64 - r.default as i64) * 1000) / r.baseline as i64
        };
        reductions.push(reduction);
        writeln!(
            table,
            "| `{}` | {} | {} | {} | {} | {}% |",
            r.tool,
            r.baseline,
            r.baseline / 4,
            r.default,
            r.default / 4,
            fmt_tenths(reduction),
        )
        .unwrap();
    }

    reductions.sort_unstable();
    let n = reductions.len();
    let median = if n % 2 == 1 {
        reductions[n / 2]
    } else {
        (reductions[n / 2 - 1] + reductions[n / 2]) / 2
    };

    let report = render_report(revision, &table, median);
    let out = root.join(".claude/plans/data-fidelity-arc/block-1/economy-token-delta.md");
    std::fs::write(&out, &report).expect("write report");

    assert!(report.contains("Median reduction"), "report missing median");
    assert!(
        report.contains(&revision.to_string()),
        "report missing revision"
    );
}

/// Render the deterministic markdown report (no timestamp / wall-clock), the
/// sibling of `outline-token-delta.md`.
fn render_report(revision: u64, table: &str, median: i64) -> String {
    format!(
        "# Token-delta: response economy across the 10 growable MCP tools\n\n\
         Generated by `crates/ariadne-mcp/tests/economy_token_delta.rs` (`#[ignore]`d).\n\
         Deterministic data artifact — no timestamp, no wall-clock, no model call.\n\n\
         ## Snapshot\n\n\
         - Index revision: `{revision}`\n\
         - Tools measured: 10\n\
         - Token cap (BR6): 25000 tokens per default page\n\n\
         ## Method\n\n\
         - **Baseline** (un-capped): the cold `tools::*::handle` output at\n\
         `verbosity:detailed`, `limit:u32::MAX`, no cursor — the whole result.\n\
         - **Default** (economy): the same handler at `verbosity:concise` and the\n\
         default page cap (50), no cursor — the first page a caller gets.\n\
         - **Token proxy**: `tokens = bytes / 4` (the `wire` projection is\n\
         `serde_json::to_string` into one text block); raw bytes reported\n\
         alongside so the relative delta does not hinge on the divisor.\n\
         - `co_change` is measured at low thresholds (the worst pre-block\n\
         offender); the diff-aware tools run over a representative single-file\n\
         changeset.\n\n\
         ## Per-tool cost\n\n\
         | Tool | Baseline bytes | Baseline tok | Default bytes | Default tok | Reduction |\n\
         |------|---------------:|-------------:|--------------:|------------:|----------:|\n\
         {table}\n\
         ## Result\n\n\
         Median reduction across 10 tools: **{median}%**. Every tool's default\n\
         page is within the 25000-token cap (BR6), so the default page size (50)\n\
         is validated, not assumed.\n",
        median = fmt_tenths(median),
    )
}
