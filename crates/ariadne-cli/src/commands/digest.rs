//! `ariadne digest` — compact, deterministic project digest for session
//! bootstrap.
//!
//! Composes three existing read-only reports through the shared
//! [`crate::commands::query::run_tool`] daemon/cold path — `project_status`,
//! `coupling_report`, and `doc_for_project` — into bounded, agent-shaped
//! Markdown that stays well under the 10 000-char `additionalContext` cap a
//! `SessionStart` hook injects (tier-03). No new domain logic and no
//! inference: the digest is a pure projection of analytics the graph already
//! computes [src: .claude/plans/ariadne-mcp-adoption/tier-02-digest-command.md;
//! plan.md D4; `feedback_no_llm_features`].
//!
//! A slow or cold daemon must never stall session start, so the three round
//! trips run on a worker thread bounded by [`DIGEST_TIMEOUT`]; on timeout, a
//! query error, or an empty graph the command emits a minimal non-empty
//! fallback (plan.md R1/R4).

use std::fmt::Write as _;
use std::path::Path;
use std::sync::mpsc;
use std::time::Duration;

use anyhow::{Context, Result};
use serde_json::Value;

use crate::commands::query::run_tool;

/// Hard ceiling for the whole digest assembly. Kept below the auto-spawn wait
/// the CLI daemon client allows, so a cold machine degrades to the minimal
/// message instead of blocking the session while a daemon comes up
/// [src: plan.md R1; crates/ariadne-cli/src/adapters/daemon_client.rs:34].
const DIGEST_TIMEOUT: Duration = Duration::from_secs(5);

/// How many top-coupled modules the digest lists.
const TOP_MODULES: usize = 8;

/// Character budget for the truncated project-overview slice.
const OVERVIEW_BUDGET: usize = 600;

/// Render the project digest to stdout. Always prints a non-empty document:
/// the composed digest when the graph answers within [`DIGEST_TIMEOUT`], else
/// the minimal fallback. A missing, cold, or empty graph degrades to that
/// fallback rather than erroring, so the command is infallible.
pub fn run(root: &Path) {
    println!("{}", build(root));
}

/// Assemble the digest Markdown, bounding the daemon round-trips by
/// [`DIGEST_TIMEOUT`] and degrading to [`fallback`] on timeout, a query error,
/// or an empty graph.
fn build(root: &Path) -> String {
    match gather_bounded(root, DIGEST_TIMEOUT) {
        Some(data) if !data.is_empty() => data.render(),
        _ => fallback(),
    }
}

/// The three read-only reports the digest composes, each parsed into a JSON
/// [`Value`] from the text `run_tool` returns so the projection reads fields by
/// name without re-deriving the tool output types.
struct DigestData {
    /// `project_status` — revision plus coarse counts.
    status: Value,
    /// `coupling_report` — one row per file-as-module (path-sorted upstream;
    /// the digest re-ranks by total coupling).
    coupling: Value,
    /// `doc_for_project` — full Markdown overview, truncated to a short slice.
    overview: Value,
}

impl DigestData {
    /// Whether the graph carries no indexed symbols — the digest then degrades
    /// to the minimal fallback rather than render empty sections.
    fn is_empty(&self) -> bool {
        u64_field(&self.status, "symbol_count") == 0
    }

    /// Render the bounded, agent-shaped Markdown digest.
    fn render(&self) -> String {
        let mut out = String::new();
        self.write_header(&mut out);
        self.write_top_modules(&mut out);
        self.write_overview(&mut out);
        write_cheat_sheet(&mut out);
        out
    }

    /// Header: revision and coarse counts, framed as a factual statement about
    /// what the graph holds (out-of-band imperative text trips prompt-injection
    /// defenses) [src: plan.md D3].
    fn write_header(&self, out: &mut String) {
        let revision = u64_field(&self.status, "revision");
        let files = u64_field(&self.status, "file_count");
        let symbols = u64_field(&self.status, "symbol_count");
        let edges = u64_field(&self.status, "edge_count");
        out.push_str("## Ariadne project digest\n\n");
        let _ = writeln!(
            out,
            "Ariadne holds a read-only semantic graph of this project at revision {revision}: \
             {files} files, {symbols} symbols, {edges} dependency edges. The graph answers \
             symbol, reference, impact, and architecture questions in one call where grep and \
             Read take many and miss cross-file edges.\n"
        );
    }

    /// Top-coupled modules, ranked by total coupling (Ca + Ce) descending with
    /// a stable path tie-break — `coupling_report` rows arrive path-sorted, not
    /// ranked, so the digest sorts them itself.
    fn write_top_modules(&self, out: &mut String) {
        out.push_str("### Top modules\n\n");
        out.push_str("Most-connected files by coupling (afferent Ca + efferent Ce):\n");
        let mut rows: Vec<&Value> = self
            .coupling
            .get("rows")
            .and_then(Value::as_array)
            .map(|rows| rows.iter().collect())
            .unwrap_or_default();
        rows.sort_by(|a, b| {
            coupling_total(b)
                .cmp(&coupling_total(a))
                .then_with(|| module_name(a).cmp(module_name(b)))
        });
        for row in rows.iter().take(TOP_MODULES) {
            let _ = writeln!(
                out,
                "- `{}` — Ca {}, Ce {}",
                module_name(row),
                u64_field(row, "afferent"),
                u64_field(row, "efferent"),
            );
        }
        out.push('\n');
    }

    /// A short slice of the `doc_for_project` Markdown — the title and the
    /// counts overview, dropping the large `## Layers` diagram and capping the
    /// remainder to [`OVERVIEW_BUDGET`] characters.
    fn write_overview(&self, out: &mut String) {
        let markdown = self
            .overview
            .get("markdown")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let slice = overview_slice(markdown);
        if !slice.is_empty() {
            out.push_str("### Project overview\n\n");
            let _ = writeln!(out, "{slice}\n");
        }
    }
}

/// Run the three tool queries on a worker thread and wait at most `timeout`.
/// Returns `None` on timeout, a query error, or a worker panic — each of which
/// the caller maps to the minimal fallback. The worker is detached: a slow
/// daemon round-trip keeps running but the one-shot CLI process exits after
/// printing, so the orphaned thread is reaped with the process.
fn gather_bounded(root: &Path, timeout: Duration) -> Option<DigestData> {
    let (tx, rx) = mpsc::channel();
    let root = root.to_path_buf();
    std::thread::spawn(move || {
        let _ = tx.send(gather(&root));
    });
    match rx.recv_timeout(timeout) {
        Ok(Ok(data)) => Some(data),
        _ => None,
    }
}

/// Fetch the three composed reports through the shared daemon/cold query path.
///
/// # Errors
/// Propagates the first [`fetch`] failure (bad index, daemon error, or
/// unparseable output), which [`gather_bounded`] maps to the fallback.
fn gather(root: &Path) -> Result<DigestData> {
    Ok(DigestData {
        status: fetch(root, "project_status")?,
        coupling: fetch(root, "coupling_report")?,
        overview: fetch(root, "doc_for_project")?,
    })
}

/// Run one read-only tool with empty arguments through the shared query path
/// and parse its pretty JSON text into a [`Value`] the projection reads by
/// field name. The query path returns declaration-ordered text; the digest
/// reads fields by key, so re-parsing into an order-less `Value` is immaterial
/// here [src: audit/tier-02-report.md F1].
///
/// # Errors
/// Propagates a `run_tool` failure or a JSON parse error.
fn fetch(root: &Path, tool: &str) -> Result<Value> {
    let json = run_tool(root, tool, "{}")?;
    serde_json::from_str(&json).context("parse tool output JSON")
}

/// Fixed question→tool cheat-sheet, phrased as factual statements so the
/// injected context never reads as an out-of-band instruction [src: plan.md
/// D3]. Mirrors the tool families listed in `CLAUDE.md`.
fn write_cheat_sheet(out: &mut String) {
    out.push_str(
        "### When to use which tool\n\n\
         These questions are answered by Ariadne tools, not grep or Read:\n\
         - Where a symbol is defined or used: `find_definition`, `find_references`, `list_symbols`.\n\
         - What a change affects: `blast_radius`, `plan_assist`, `diff_blast_radius`.\n\
         - Structural health and worst modules: `coupling_report`, `weak_spots`, `refactor_suggestions`.\n\
         - Risk from Git churn and complexity: `hotspots`, `complexity`, `co_change`.\n\
         - A summary of a symbol, file, or project: `doc_for`, `doc_for_module`, `doc_for_project`.\n\
         - Whether the index is current: `project_status`.\n",
    );
}

/// Minimal non-empty fallback for a missing, cold, slow, or empty graph. Stays
/// agent-shaped (the same `## Ariadne` framing) and points at the live
/// freshness check [src: tier-02 step 3; plan.md R1].
fn fallback() -> String {
    "## Ariadne\n\n\
     Ariadne's read-only semantic graph is configured for this project but produced no digest \
     this session (the daemon is starting, or the index is empty). `ariadne index` rebuilds the \
     index when symbols are missing; `project_status` reports whether it is current. The Ariadne \
     tools answer symbol, reference, impact, and architecture questions in one call where grep \
     and Read take many.\n"
        .to_owned()
}

/// Read an unsigned-integer JSON field, defaulting to `0` when absent or of an
/// unexpected type.
fn u64_field(value: &Value, key: &str) -> u64 {
    value.get(key).and_then(Value::as_u64).unwrap_or(0)
}

/// A coupling row's module path, or the empty string when absent.
fn module_name(row: &Value) -> &str {
    row.get("module")
        .and_then(Value::as_str)
        .unwrap_or_default()
}

/// Total coupling (afferent + efferent) for a `coupling_report` row.
fn coupling_total(row: &Value) -> u64 {
    u64_field(row, "afferent") + u64_field(row, "efferent")
}

/// Trim the `doc_for_project` Markdown to a short overview: everything before
/// the `## Layers` diagram section, capped to [`OVERVIEW_BUDGET`] characters.
fn overview_slice(markdown: &str) -> String {
    let head = markdown
        .split_once("\n## Layers")
        .map_or(markdown, |(before, _)| before)
        .trim();
    truncate_chars(head, OVERVIEW_BUDGET)
}

/// Truncate `s` to at most `max` characters on a UTF-8 boundary, appending an
/// ellipsis when content was dropped.
fn truncate_chars(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_owned();
    }
    let mut out: String = s.chars().take(max).collect();
    out.push('…');
    out
}
