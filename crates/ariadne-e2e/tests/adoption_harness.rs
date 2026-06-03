//! Behavioral adoption harness — measures whether wiring Ariadne (via
//! `ariadne setup`) shifts a headless `claude -p` session away from native
//! `Grep`/`Read` toward the `mcp__ariadne__*` graph tools on codebase
//! questions.
//!
//! For each question in `fixtures/adoption_questions.txt` it drives a headless
//! `claude -p` run in a fixture repo with `--output-format stream-json`, parses
//! the transcript for `tool_use` block names, and tallies `mcp__ariadne__*`
//! against `Grep`/`Read`. It runs the set twice — a baseline repo with no
//! Ariadne wiring and a treated repo after `setup` + `index` — and prints both
//! ratios plus token totals (plan.md D7; tier-05 steps 3-4).
//!
//! `#[ignore]` by construction: it shells out to a real `claude` binary, makes
//! network calls, and the model's tool choice is non-deterministic — never a
//! CI gate (R2, anti-flake). Run manually, output uncaptured:
//!   `cargo nextest run -p ariadne-e2e --run-ignored all \
//!       -E 'test(adoption_ratio_baseline_vs_treated)' --no-capture`
//! [src: .claude/plans/ariadne-mcp-adoption/tier-05-adoption-eval.md
//! `<verification>`; <https://code.claude.com/docs/en/headless> stream-json].

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::channel;
use std::thread;
use std::time::Duration;

use ariadne_e2e::domain::{
    McpClient, ariadne_binary, collect_source_files, run_index, run_setup, tool_text,
};
use serde_json::{Value, json};
use tempfile::{TempDir, tempdir};

/// Upper bound on one headless `claude -p` answer. A run exceeding it is a
/// transport/auth fault, not a slow answer — the harness fails loudly rather
/// than hang the manual measurement.
const PER_CALL_TIMEOUT: Duration = Duration::from_secs(240);

/// The fixture project: a three-file Rust call chain
/// (`helper` ← `double` ← `compute` ← `run_pipeline` ← `main`), enough for the
/// question set's reference/impact/overview asks to have real answers.
const FILES: &[(&str, &str)] = &[
    (
        "src/math.rs",
        "pub fn helper(value: i32) -> i32 {\n    value + 1\n}\n\n\
         pub fn double(value: i32) -> i32 {\n    helper(value) + helper(value)\n}\n",
    ),
    (
        "src/pipeline.rs",
        "use crate::math::double;\n\n\
         pub fn compute() -> i32 {\n    double(20)\n}\n\n\
         pub fn run_pipeline() -> i32 {\n    compute() + compute()\n}\n",
    ),
    (
        "src/main.rs",
        "mod math;\nmod pipeline;\n\n\
         fn main() {\n    let _ = pipeline::run_pipeline();\n}\n",
    ),
];

/// Per-variant tool-call and token tallies parsed from the stream-json
/// transcripts across the whole question set.
#[derive(Debug, Default, Clone, Copy)]
struct Tally {
    /// `mcp__ariadne__*` tool calls — the graph path.
    ariadne: u64,
    /// `mcp__ariadne__search_code` calls — the symbol-pattern search the
    /// advisory now steers Grep/Glob toward (tier-09; a subset of `ariadne`).
    search_code: u64,
    /// `mcp__ariadne__read_symbol` calls — the source-read the advisory now
    /// steers whole-file Reads toward (tier-09; a subset of `ariadne`).
    read_symbol: u64,
    /// `Grep` tool calls — native text search.
    grep: u64,
    /// `Read` tool calls — native file read.
    read: u64,
    /// `Glob` tool calls — native path search (reported, not in the D7 ratio).
    glob: u64,
    /// Summed `input_tokens` from each question's terminal `result` frame.
    input_tokens: u64,
    /// Summed `output_tokens` from each question's terminal `result` frame.
    output_tokens: u64,
}

impl Tally {
    /// Native `Grep` + `Read` calls — the D7 denominator.
    fn grep_read(self) -> u64 {
        self.grep + self.read
    }

    /// Ariadne-to-grep/read ratio (D7). `None` when no grep/read calls fired
    /// (an undefined ratio — reported as such rather than as a fake number).
    fn ratio(self) -> Option<f64> {
        let denom = self.grep_read();
        #[allow(clippy::cast_precision_loss)]
        (denom != 0).then(|| self.ariadne as f64 / denom as f64)
    }

    /// Ariadne share of all three call classes, as a percentage — the
    /// "majority path" metric from the plan `<context>`.
    #[allow(clippy::cast_precision_loss)]
    fn ariadne_share_pct(self) -> Option<f64> {
        let total = self.ariadne + self.grep_read();
        (total != 0).then(|| 100.0 * self.ariadne as f64 / total as f64)
    }
}

#[test]
#[ignore = "behavioral: drives nested `claude -p`, network + non-deterministic — run manually"]
fn adoption_ratio_baseline_vs_treated() {
    let claude = claude_binary();
    let questions = load_questions();
    assert!(!questions.is_empty(), "question fixture is empty");

    // Treated: full Ariadne wiring (`setup`) over a built index.
    let treated_dir = make_fixture();
    let treated = treated_dir.path();
    run_setup(treated).expect("ariadne setup on treated fixture");
    let _reap = ReapDaemon { root: treated };
    let report = run_index(treated).expect("ariadne index on treated fixture");
    assert!(
        report.is_non_empty(),
        "treated fixture produced an empty graph: {report:?}",
    );
    let treated_tally = run_variant(&claude, treated, &questions, true);

    // Baseline: identical source, no Ariadne wiring at all.
    let baseline_dir = make_fixture();
    let baseline = baseline_dir.path();
    let baseline_tally = run_variant(&claude, baseline, &questions, false);

    report_tally("baseline (setup reverted)", &baseline_tally);
    report_tally("treated  (setup applied) ", &treated_tally);
    report_delta(&baseline_tally, &treated_tally);
}

/// Run the whole question set against one repo variant and accumulate the
/// tallies. `treated` selects the MCP wiring: the treated repo loads its own
/// `.mcp.json` (Ariadne server); the baseline loads no MCP servers at all.
fn run_variant(claude: &Path, root: &Path, questions: &[String], treated: bool) -> Tally {
    let mut tally = Tally::default();
    for (i, question) in questions.iter().enumerate() {
        eprintln!(
            "[adoption] {} q{}/{}: {question}",
            if treated { "treated " } else { "baseline" },
            i + 1,
            questions.len(),
        );
        let lines = capture_session(claude, root, question, treated);
        parse_transcript(&lines, &mut tally);
    }
    tally
}

/// Spawn one headless `claude -p` session in `root` and return its stdout
/// lines. `--strict-mcp-config` pins the MCP surface to exactly what the
/// harness controls: the treated repo's `.mcp.json` (the Ariadne server) or, in
/// the baseline, nothing. `--dangerously-skip-permissions` lets the unattended
/// run call tools and fire the installed hooks without prompting.
fn capture_session(claude: &Path, root: &Path, question: &str, treated: bool) -> Vec<String> {
    let mut cmd = Command::new(claude);
    cmd.current_dir(root)
        .arg("-p")
        .arg(question)
        .args(["--output-format", "stream-json"])
        .arg("--verbose")
        .arg("--dangerously-skip-permissions")
        .arg("--strict-mcp-config");
    if treated {
        cmd.arg("--mcp-config").arg(root.join(".mcp.json"));
    }
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());

    let mut child = cmd.spawn().expect("spawn `claude -p`");
    let stdout = child.stdout.take().expect("capture claude stdout");
    let (tx, rx) = channel();
    thread::spawn(move || {
        let lines: Vec<String> = BufReader::new(stdout)
            .lines()
            .map_while(Result::ok)
            .collect();
        let _ = tx.send(lines);
    });
    if let Ok(lines) = rx.recv_timeout(PER_CALL_TIMEOUT) {
        let _ = child.wait();
        lines
    } else {
        let _ = child.kill();
        let _ = child.wait();
        panic!("`claude -p` did not finish within {PER_CALL_TIMEOUT:?}");
    }
}

/// Parse one session's stream-json lines, counting `tool_use` blocks by class
/// and folding the terminal `result` frame's token usage into `tally`.
fn parse_transcript(lines: &[String], tally: &mut Tally) {
    for line in lines {
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            continue; // non-JSON banner / log line
        };
        match value.get("type").and_then(Value::as_str) {
            Some("assistant") => count_tool_uses(&value, tally),
            Some("result") => add_usage(&value, tally),
            _ => {}
        }
    }
}

/// Count the `tool_use` content blocks in one assistant message by class.
fn count_tool_uses(message: &Value, tally: &mut Tally) {
    let Some(content) = message
        .pointer("/message/content")
        .and_then(Value::as_array)
    else {
        return;
    };
    for block in content {
        if block.get("type").and_then(Value::as_str) != Some("tool_use") {
            continue;
        }
        match block.get("name").and_then(Value::as_str) {
            Some(name) if name.starts_with("mcp__ariadne__") => {
                tally.ariadne += 1;
                match name {
                    "mcp__ariadne__search_code" => tally.search_code += 1,
                    "mcp__ariadne__read_symbol" => tally.read_symbol += 1,
                    _ => {}
                }
            }
            Some("Grep") => tally.grep += 1,
            Some("Read") => tally.read += 1,
            Some("Glob") => tally.glob += 1,
            _ => {}
        }
    }
}

/// Fold one terminal `result` frame's `usage` token counts into `tally`.
fn add_usage(result: &Value, tally: &mut Tally) {
    let Some(usage) = result.get("usage") else {
        return;
    };
    tally.input_tokens += usage
        .get("input_tokens")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    tally.output_tokens += usage
        .get("output_tokens")
        .and_then(Value::as_u64)
        .unwrap_or(0);
}

/// Print one variant's tallies as a single `[adoption]` line.
fn report_tally(label: &str, t: &Tally) {
    let ratio = t
        .ratio()
        .map_or_else(|| "n/a (0 grep+read)".to_owned(), |r| format!("{r:.2}"));
    let share = t
        .ariadne_share_pct()
        .map_or_else(|| "n/a".to_owned(), |s| format!("{s:.0}%"));
    println!(
        "[adoption] {label} | ariadne={} (search_code={} read_symbol={}) grep={} read={} glob={} \
         | ratio(ariadne:grep+read)={ratio} ariadne_share={share} \
         | tokens in={} out={}",
        t.ariadne,
        t.search_code,
        t.read_symbol,
        t.grep,
        t.read,
        t.glob,
        t.input_tokens,
        t.output_tokens,
    );
}

/// Print the treated-vs-baseline shift in the ariadne share.
fn report_delta(baseline: &Tally, treated: &Tally) {
    let base = baseline.ariadne_share_pct().unwrap_or(0.0);
    let treat = treated.ariadne_share_pct().unwrap_or(0.0);
    println!(
        "[adoption] delta | ariadne_share baseline={base:.0}% -> treated={treat:.0}% \
         ({:+.0} pts); target: Ariadne the majority path (>50%)",
        treat - base,
    );
}

/// Materialise the fixture project into a fresh tempdir.
fn make_fixture() -> TempDir {
    let dir = tempdir().expect("create fixture tempdir");
    for (rel, body) in FILES {
        let path = dir.path().join(rel);
        std::fs::create_dir_all(path.parent().expect("fixture path has a parent"))
            .expect("create fixture subdir");
        std::fs::write(&path, body).expect("write fixture file");
    }
    dir
}

/// Load the question fixture, dropping blank and `#`-comment lines.
fn load_questions() -> Vec<String> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join("adoption_questions.txt");
    let text =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(str::to_owned)
        .collect()
}

/// Resolve the `claude` binary on `PATH`. The behavioral harness is opt-in, so
/// its absence is a hard failure with a clear message rather than a silent
/// skip — you run it only when you intend to measure.
fn claude_binary() -> PathBuf {
    let probe = Command::new("claude").arg("--version").output();
    assert!(
        probe.is_ok_and(|o| o.status.success()),
        "`claude` not runnable on PATH — the adoption harness needs the Claude Code CLI",
    );
    PathBuf::from("claude")
}

/// Stops the project's daemon on drop, reaping the one the treated repo's MCP
/// server auto-spawns. `ariadne daemon stop` is idempotent (mirrors
/// `mcp_session.rs::ReapDaemon`).
struct ReapDaemon<'a> {
    root: &'a Path,
}

impl Drop for ReapDaemon<'_> {
    fn drop(&mut self) {
        let _ = Command::new(ariadne_binary())
            .args(["daemon", "stop"])
            .arg(self.root)
            .output();
    }
}

/// tier-09 step 6 — deterministic real-tool token-delta re-measure.
///
/// The tier-06 spike (`search_read_spike.rs`) projected a hypothetical
/// search+read path by hand because the tools did not exist yet; tiers 07–08
/// shipped the real `search_code` + `read_symbol`. This re-runs the spike's
/// deterministic byte/token-delta method against the REAL tools, driving them
/// over the MCP stdio port (the production read path) against this repo's live
/// index — no model, no wall-clock, no fabricated rows. The median is printed
/// for recording in the tier notes and compared to the spike's 87.3% estimate.
///
/// Tasks span the spike's three shapes; targets resolve at this repo's revision.
const REMEASURE_TASKS: &[(&str, &str)] = &[
    ("find-definition", "Catalog"),
    ("find-definition", "RedbStorage"),
    ("find-definition", "SymbolSummary"),
    ("search-by-pattern", "doc_for"),
    ("search-by-pattern", "summarize"),
    ("search-by-pattern", "handle"),
    ("read-body", "find_symbol"),
    ("read-body", "build"),
];

#[test]
#[ignore = "deterministic re-measure: drives `ariadne serve` over the workspace's live index — run manually"]
#[allow(
    clippy::cast_possible_wrap,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]
fn real_tool_token_delta_vs_grep() {
    let root = workspace_root();
    assert!(
        root.join(".ariadne/index.redb").is_file(),
        "live index missing under {} — run `ariadne index` first",
        root.display(),
    );

    // Read the indexed source corpus once: the baseline greps it for each symbol
    // name (matching-line bytes only — the spike's conservative grep proxy).
    let files = collect_source_files(&root, 100_000);
    assert!(!files.is_empty(), "no source files under workspace root");
    let corpus: Vec<Vec<u8>> = files.iter().filter_map(|p| std::fs::read(p).ok()).collect();

    // Drive the REAL tools over the MCP stdio port. The daemon is NOT reaped:
    // when one already serves this repo the queries hit it read-only, and the
    // `McpClient` drop kills only the `ariadne serve` child it spawned.
    let mut client = McpClient::connect(&root).expect("connect MCP client to workspace");
    let status = client
        .call_tool("project_status", &json!({}))
        .expect("project_status");
    let revision = serde_json::from_str::<Value>(&tool_text(&status).expect("status text"))
        .ok()
        .and_then(|v| v.get("revision").and_then(Value::as_u64))
        .unwrap_or(0);

    let mut reductions: Vec<i64> = Vec::new();
    println!("[remeasure] real-tool token delta vs grep+whole-file Read (revision {revision})");
    println!("[remeasure] shape | symbol | baseline_tok | proto_tok | reduction");
    for (shape, name) in REMEASURE_TASKS {
        // Real read_symbol (context mode) — resolves the symbol and returns just
        // its span +context from the live file. A vanished target is skipped (the
        // live index drifts; the spike's fixed-revision copy could panic instead).
        let Ok(read) =
            client.call_tool("read_symbol", &json!({ "symbol": name, "mode": "context" }))
        else {
            eprintln!("[remeasure] skip `{name}`: read_symbol did not resolve it");
            continue;
        };
        let read_text = tool_text(&read).expect("read_symbol text");
        let slice: Value = serde_json::from_str(&read_text).expect("parse read_symbol");
        let target_file = slice
            .get("file")
            .and_then(Value::as_str)
            .unwrap_or_default();

        // Real search_code (substring on the name).
        let search = client
            .call_tool("search_code", &json!({ "query": name }))
            .expect("search_code");
        let search_text = tool_text(&search).expect("search_code text");

        // Baseline: Σ grep matching-line bytes for the name across the corpus,
        // plus a whole-file Read of the file read_symbol resolved.
        let grep_bytes: u64 = corpus
            .iter()
            .map(|c| grep_line_bytes(c, name.as_bytes()))
            .sum();
        let whole_file = std::fs::read(root.join(target_file)).map_or(0, |b| b.len() as u64);
        let baseline = grep_bytes + whole_file;
        let proto = search_text.len() as u64 + read_text.len() as u64;
        if baseline == 0 {
            eprintln!("[remeasure] skip `{name}`: zero baseline");
            continue;
        }

        let reduction = ((baseline as i64 - proto as i64) * 1000) / baseline as i64;
        reductions.push(reduction);
        println!(
            "[remeasure] {shape} | {name} | {} | {} | {}%",
            baseline / 4,
            proto / 4,
            fmt_tenths(reduction),
        );
    }

    assert!(
        reductions.len() >= 4,
        "too few tasks resolved against the live index ({}) — re-check targets",
        reductions.len(),
    );
    reductions.sort_unstable();
    let n = reductions.len();
    let median = if n % 2 == 0 {
        (reductions[n / 2 - 1] + reductions[n / 2]) / 2
    } else {
        reductions[n / 2]
    };
    println!(
        "[remeasure] median real-tool reduction across {n} tasks: {}% \
         (tier-06 spike estimate: 87.3%)",
        fmt_tenths(median),
    );
}

/// Baseline grep cost over one file: Σ bytes of lines containing `needle`
/// (matching-line text only, no `file:line:` prefix — the conservative grep
/// proxy from the tier-06 spike).
#[allow(clippy::cast_possible_truncation)]
fn grep_line_bytes(content: &[u8], needle: &[u8]) -> u64 {
    if needle.is_empty() {
        return 0;
    }
    content
        .split(|&b| b == b'\n')
        .filter(|line| line.windows(needle.len()).any(|w| w == needle))
        .map(|line| line.len() as u64)
        .sum()
}

/// Format tenths-of-a-percent as `X.Y`, sign-aware (mirrors the spike).
fn fmt_tenths(t: i64) -> String {
    let a = t.unsigned_abs();
    format!("{}{}.{}", if t < 0 { "-" } else { "" }, a / 10, a % 10)
}

/// Workspace root = `crates/ariadne-e2e/../..`, canonicalised.
fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("canonicalise workspace root")
}
