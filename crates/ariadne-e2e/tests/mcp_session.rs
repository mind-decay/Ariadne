//! Scripted MCP transcript — a realistic Claude Code session against the
//! `ariadne serve` stdio server.
//!
//! Builds a small Rust fixture, indexes it, spawns the MCP server, and
//! replays `initialize` → `tools/list` → 50 `tools/call` frames cycling
//! every tool with valid arguments. Asserts no error frame is returned and
//! that response latency stays within the 100 ms p95 budget
//! [src: .claude/plans/ariadne-core/tier-10-cli-e2e.md step 11].
//!
//! Offline + sub-second, so this runs on the default `cargo nextest` pass —
//! unlike the `#[ignore]`d clone-backed suites.

use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

use ariadne_e2e::domain::{McpClient, ariadne_binary, percentile, run_index, run_init, tool_text};
use serde_json::{Value, json};
use tempfile::tempdir;

/// Every tool the `AriadneServer` `#[tool_router]` exposes [src:
/// crates/ariadne-mcp/src/server.rs].
const EXPECTED_TOOLS: &[&str] = &[
    "list_symbols",
    "find_definition",
    "find_references",
    "blast_radius",
    "file_summary",
    "plan_assist",
    "coupling_report",
    "weak_spots",
    "doc_for",
    "project_status",
    "doc_for_module",
    "doc_for_project",
    "refactor_suggestions",
];

/// Fixture: two Rust files with cross-file calls, enough for a non-empty
/// symbol + edge graph.
const UTIL_RS: &str = "pub fn helper(value: i32) -> i32 {\n    value + 1\n}\n\n\
                       pub fn double(value: i32) -> i32 {\n    helper(value) + helper(value)\n}\n";
const MAIN_RS: &str = "fn compute() -> i32 {\n    double(20)\n}\n\n\
                       fn main() {\n    let _ = compute();\n}\n";

#[test]
fn mcp_session_replays_a_realistic_transcript() {
    let project = tempdir().expect("create fixture tempdir");
    let root = project.path();
    write_fixture(root);

    // The MCP server auto-spawns a warm daemon (tier-09) on the first tool call
    // and that detached daemon outlives the `ariadne serve` child. Reap it on
    // scope exit *and* on a panic unwind (e.g. a p95-budget miss) so the test
    // never orphans a background `ariadne daemon` process. Idempotent: a no-op
    // when no daemon was spawned.
    let _daemon = ReapDaemon { root };

    run_init(root).expect("ariadne init on fixture");
    let report = run_index(root).expect("ariadne index on fixture");
    assert!(
        report.is_non_empty(),
        "fixture produced an empty graph: {report:?}",
    );

    let mut client = McpClient::connect(root).expect("connect MCP client");

    // tools/list — the full advertised surface.
    let tools = client.list_tools().expect("tools/list");
    for expected in EXPECTED_TOOLS {
        assert!(
            tools.iter().any(|t| t == expected),
            "tools/list omitted `{expected}`: advertised {tools:?}",
        );
    }

    // Seed a real symbol + file path from list_symbols for the call plan.
    let listed = client
        .call_tool("list_symbols", &json!({}))
        .expect("list_symbols");
    let rows: Vec<Value> = serde_json::from_str(&tool_text(&listed).expect("list_symbols text"))
        .expect("parse list_symbols rows");
    assert!(!rows.is_empty(), "fixture index exposed no symbols");
    let symbol = string_field(&rows[0], "name");
    let file = string_field(&rows[0], "file");

    // A valid-argument call for every tool; cycled to 50 frames.
    let plan: Vec<(&str, Value)> = vec![
        ("project_status", json!({})),
        ("list_symbols", json!({ "query": "comp" })),
        ("find_definition", json!({ "symbol": symbol })),
        ("find_references", json!({ "symbol": symbol })),
        ("blast_radius", json!({ "symbol": symbol })),
        ("file_summary", json!({ "path": file })),
        ("plan_assist", json!({ "symbol": symbol })),
        ("coupling_report", json!({})),
        ("weak_spots", json!({})),
        ("doc_for", json!({ "symbol": symbol })),
        ("doc_for_module", json!({ "path": file })),
        ("doc_for_project", json!({})),
        ("refactor_suggestions", json!({})),
    ];

    let mut latencies: Vec<Duration> = Vec::with_capacity(50);
    for (tool, args) in plan.iter().cycle().take(50) {
        let started = Instant::now();
        client
            .call_tool(tool, args)
            .unwrap_or_else(|e| panic!("tools/call `{tool}` returned an error frame: {e:#}"));
        latencies.push(started.elapsed());
    }

    let p95 = percentile(&mut latencies, 95.0);
    assert!(
        p95 < Duration::from_millis(100),
        "MCP tools/call p95 {p95:?}, over the 100 ms budget",
    );
}

/// Write the two-file Rust fixture into `root`.
fn write_fixture(root: &Path) {
    fs::write(root.join("util.rs"), UTIL_RS).expect("write util.rs");
    fs::write(root.join("main.rs"), MAIN_RS).expect("write main.rs");
}

/// Stops the project's daemon on drop, so the daemon the MCP server auto-spawns
/// is reaped on a clean exit and on a panic unwind alike. `ariadne daemon stop`
/// is idempotent — a no-op when no daemon is running.
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

/// Read a required string field off a `list_symbols` row.
fn string_field(row: &Value, key: &str) -> String {
    row.get(key)
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("list_symbols row missing `{key}`: {row}"))
        .to_owned()
}
