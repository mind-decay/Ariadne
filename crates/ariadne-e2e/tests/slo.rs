//! Combined SLO release gate — the v1 ship/no-ship decision.
//!
//! Clones a ≥100K-file corpus spanning ≥3 real OSS repositories, then asserts
//! the three v1 performance budgets in turn: cold full-index < 60 s,
//! incremental-update apply p95 < 500 ms, query p95 < 100 ms. A breach fails
//! the test loudly — per the tier, v1 does not ship and a follow-up tier is
//! opened for the failure mode; the bench is never silenced
//! [src: .claude/plans/ariadne-core/tier-10-cli-e2e.md step 1 + `<verification>`].
//!
//! `#[ignore]` — the corpus is multiple GB of shallow clones. Run explicitly:
//! `cargo nextest run -p ariadne-e2e --run-ignored all`.

use std::fs::OpenOptions;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc::channel;
use std::thread;
use std::time::{Duration, Instant};

use ariadne_e2e::domain::{
    McpClient, PerfBudget, ariadne_binary, collect_source_files, percentile, repo_spec,
    run_index_measured, run_init, shallow_clone, tool_text,
};
use serde_json::{Value, json};
use tempfile::tempdir;

/// SLO corpus: `(manifest key, clone subdirectory)`. `torvalds/linux`
/// carries the corpus past the 100K genuinely-indexed-file floor
/// [src: .claude/plans/ariadne-core/tier-12-parallel-cold-index.md step 7];
/// the four JS-framework repos add `.jsx`/`.tsx`/`.vue`/`.svelte`/`.astro`
/// files so the gate measures the multi-region SFC parse against the v1
/// SLOs (tier-09 step 6, risk R-SLO)
/// [src: .claude/plans/js-framework-support/tier-09-component-graph-e2e.md step 6].
const CORPUS: &[(&str, &str)] = &[
    ("go", "kubernetes"),
    ("typescript", "vscode"),
    ("csharp", "dotnet-runtime"),
    ("c", "linux"),
    ("react", "react-dev"),
    ("vue", "vitepress"),
    ("svelte", "svelte-realworld"),
    ("astro", "astro-docs"),
];

/// Cold-index peak-RSS ceiling — 4 GiB, plan risk R1
/// [src: .claude/plans/ariadne-core/plan.md `<constraints>`, `<risks>` R1].
const PEAK_RSS_BUDGET: u64 = 4 * 1024 * 1024 * 1024;

/// Warm-query SLO — RD6 tightens the query budget to p95 < 10 ms once the
/// query is served by the always-warm daemon, versus the 100 ms v1 cold path
/// [src: .claude/plans/post-v1-roadmap/plan.md `<constraints>`, RD6;
///  .claude/plans/post-v1-roadmap/tier-10-cli-daemon-client-slo.md exit #2].
const WARM_QUERY_BUDGET: Duration = Duration::from_millis(10);

#[test]
#[ignore = "clones a multi-GB OSS corpus; the v1 release gate — run via --run-ignored"]
fn slo_release_gate() {
    let corpus = tempdir().expect("create corpus tempdir");
    let root = corpus.path();
    for (lang, subdir) in CORPUS {
        let spec = repo_spec(lang).unwrap_or_else(|e| panic!("manifest entry `{lang}`: {e:#}"));
        eprintln!("[slo] cloning `{lang}` fixture -> {subdir}");
        shallow_clone(&spec, &root.join(subdir))
            .unwrap_or_else(|e| panic!("shallow-clone `{lang}`: {e:#}"));
    }

    run_init(root).expect("ariadne init on corpus");
    eprintln!("[slo] indexing corpus (cold) ...");
    let report = run_index_measured(root).expect("ariadne index on corpus");

    // --- cold-index SLO ----------------------------------------------------
    eprintln!(
        "[slo] cold index: {} files, {} symbols, {} edges, {} langs in {:?}, \
         peak RSS {} MiB",
        report.files,
        report.symbols,
        report.edges,
        report.langs.len(),
        report.cold_index(),
        report.peak_rss_bytes / (1024 * 1024),
    );
    assert!(
        report.files >= 100_000,
        "corpus holds {} files, under the 100K-file SLO floor",
        report.files,
    );
    assert!(
        report.langs.len() >= 3,
        "corpus spans {} languages, under the 3-language floor: {:?}",
        report.langs.len(),
        report.langs,
    );
    assert!(report.is_non_empty(), "corpus produced an empty graph");
    assert!(
        report.cold_index() < PerfBudget::V1.cold_index,
        "cold index took {:?}, over the {:?} SLO",
        report.cold_index(),
        PerfBudget::V1.cold_index,
    );
    assert!(
        report.peak_rss_bytes < PEAK_RSS_BUDGET,
        "cold index peak RSS {} MiB, over the 4 GiB ceiling (R1)",
        report.peak_rss_bytes / (1024 * 1024),
    );

    // --- incremental-update SLO -------------------------------------------
    let mut incremental = measure_incremental(root);
    assert!(
        incremental.len() >= 50,
        "incremental probe captured only {} apply samples; need >= 50",
        incremental.len(),
    );
    let inc_p95 = percentile(&mut incremental, 95.0);
    eprintln!(
        "[slo] incremental apply p95: {inc_p95:?} over {} samples",
        incremental.len(),
    );
    assert!(
        inc_p95 < PerfBudget::V1.incremental_p95,
        "incremental apply p95 {inc_p95:?}, over the {:?} SLO",
        PerfBudget::V1.incremental_p95,
    );

    // --- query SLO ---------------------------------------------------------
    let mut query = measure_query(root);
    let query_p95 = percentile(&mut query, 95.0);
    eprintln!(
        "[slo] query p95: {query_p95:?} over {} samples",
        query.len(),
    );
    assert!(
        query_p95 < PerfBudget::V1.query_p95,
        "query p95 {query_p95:?}, over the {:?} SLO",
        PerfBudget::V1.query_p95,
    );

    // --- warm-query SLO (RD6) ----------------------------------------------
    // Bring the warm daemon up explicitly, then re-measure the query path:
    // with a daemon serving, every `blast_radius` round-trip is answered from
    // the in-RAM warm graph over IPC instead of a per-session cold rebuild,
    // and must clear the tightened 10 ms budget. This stage runs *after* the
    // v1 cold/incremental/query stages, so the existing gate is extended, not
    // weakened (tier-10 step 6).
    ensure_daemon(root);
    let mut warm = measure_query(root);
    let warm_p95 = percentile(&mut warm, 95.0);
    eprintln!(
        "[slo] warm query p95: {warm_p95:?} over {} samples",
        warm.len(),
    );

    // daemon RSS probe (R1): the warm graph must fit under the 4 GiB ceiling.
    let rss = daemon_rss_bytes(root);
    eprintln!("[slo] daemon RSS: {} MiB", rss / (1024 * 1024));
    stop_daemon(root);

    assert!(
        warm_p95 < WARM_QUERY_BUDGET,
        "warm query p95 {warm_p95:?}, over the {WARM_QUERY_BUDGET:?} warm SLO (RD6)",
    );
    assert!(
        rss > 0,
        "daemon RSS probe returned 0 bytes — the daemon PID was not resolvable",
    );
    assert!(
        rss < PEAK_RSS_BUDGET,
        "daemon RSS {} MiB, over the 4 GiB ceiling (R1)",
        rss / (1024 * 1024),
    );
}

/// Bring the warm daemon up for `root`, tolerating an already-running daemon
/// (the tier-09 MCP auto-spawn may have started one during the v1 query
/// stage). Polls `ariadne daemon status` until it reports running.
fn ensure_daemon(root: &Path) {
    // A non-zero exit here means "already running"; the status poll is the
    // real readiness gate, so the start result is intentionally ignored.
    let _ = Command::new(ariadne_binary())
        .args(["daemon", "start"])
        .arg(root)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if daemon_pid(root).is_some() {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "daemon did not report running within 30s",
        );
        thread::sleep(Duration::from_millis(50));
    }
}

/// Stop the warm daemon for `root` (idempotent).
fn stop_daemon(root: &Path) {
    let _ = Command::new(ariadne_binary())
        .args(["daemon", "stop"])
        .arg(root)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

/// Resolve the running daemon's PID from `ariadne daemon status` stdout
/// (`daemon running (pid N)`), or `None` when no daemon is up.
fn daemon_pid(root: &Path) -> Option<u32> {
    let output = Command::new(ariadne_binary())
        .args(["daemon", "status"])
        .arg(root)
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&output.stdout);
    let marker = text.find("pid ")?;
    text[marker + 4..]
        .trim_start()
        .split(|c: char| !c.is_ascii_digit())
        .next()?
        .parse()
        .ok()
}

/// Peak RSS of the running daemon process, in bytes, via `ps -o rss=` (the
/// figure is in KiB on both macOS and Linux). `0` when the PID is
/// unresolvable, which the warm stage treats as a probe failure.
fn daemon_rss_bytes(root: &Path) -> u64 {
    let Some(pid) = daemon_pid(root) else {
        return 0;
    };
    let Ok(output) = Command::new("ps")
        .args(["-o", "rss=", "-p", &pid.to_string()])
        .output()
    else {
        return 0;
    };
    String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse::<u64>()
        .map_or(0, |kib| kib.saturating_mul(1024))
}

/// Spawn `ariadne watch`, mutate distinct source files, and collect the
/// per-edit apply latencies the watcher reports on stderr.
fn measure_incremental(root: &Path) -> Vec<Duration> {
    let mut child = Command::new(ariadne_binary())
        .arg("watch")
        .arg(root)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn `ariadne watch`");
    let stderr = child.stderr.take().expect("capture watch stderr");
    let (tx, rx) = channel();
    thread::spawn(move || {
        for line in BufReader::new(stderr).lines().map_while(Result::ok) {
            if tx.send(line).is_err() {
                break;
            }
        }
    });

    // Block on the watcher-ready banner before mutating anything.
    let ready_by = Instant::now() + Duration::from_secs(180);
    loop {
        let remaining = ready_by.saturating_duration_since(Instant::now());
        assert!(!remaining.is_zero(), "`ariadne watch` never reported ready");
        match rx.recv_timeout(remaining) {
            Ok(line) if line.contains("watching") => break,
            Ok(_) => {}
            Err(reason) => panic!("`ariadne watch` never reported ready: {reason}"),
        }
    }

    // Mutate distinct files, spaced so the debouncer emits each separately.
    let files = collect_source_files(root, 200);
    assert!(
        files.len() >= 50,
        "corpus exposed only {} source files to probe",
        files.len(),
    );
    for path in files.iter().take(160) {
        if let Ok(mut handle) = OpenOptions::new().append(true).open(path) {
            let _ = handle.write_all(b"\n");
        }
        thread::sleep(Duration::from_millis(70));
    }

    // Let the final debounce window flush, then drain reported apply samples.
    thread::sleep(Duration::from_secs(5));
    let mut samples = Vec::new();
    while let Ok(line) = rx.recv_timeout(Duration::from_millis(200)) {
        if let Some(micros) = parse_apply_micros(&line) {
            samples.push(Duration::from_micros(micros));
        }
    }
    let _ = child.kill();
    let _ = child.wait();
    samples
}

/// Parse the `(<n> us apply)` suffix the watcher's logging sink prints.
fn parse_apply_micros(line: &str) -> Option<u64> {
    let head = line.get(..line.find(" us apply)")?)?;
    head.rsplit('(').next()?.trim().parse().ok()
}

/// Drive 100 `blast_radius` queries through a warm MCP catalog, timing each
/// round-trip — the query SLO measures a warm catalog, not process spawn.
fn measure_query(root: &Path) -> Vec<Duration> {
    let mut client = McpClient::connect(root).expect("connect MCP client");
    let listed = client
        .call_tool("list_symbols", &json!({ "limit": 256 }))
        .expect("list_symbols");
    let names = symbol_names(&listed);
    assert!(
        !names.is_empty(),
        "list_symbols returned no symbols to query"
    );

    let mut samples = Vec::with_capacity(100);
    for symbol in names.iter().cycle().take(100) {
        let started = Instant::now();
        client
            .call_tool("blast_radius", &json!({ "symbol": symbol }))
            .unwrap_or_else(|e| panic!("blast_radius `{symbol}`: {e:#}"));
        samples.push(started.elapsed());
    }
    samples
}

/// Pull canonical names out of a `list_symbols` MCP result frame.
fn symbol_names(result: &Value) -> Vec<String> {
    let text = tool_text(result).expect("list_symbols result text");
    let rows: Vec<Value> = serde_json::from_str(&text).expect("parse list_symbols rows");
    rows.iter()
        .filter_map(|row| row.get("name").and_then(Value::as_str))
        .map(str::to_owned)
        .collect()
}
