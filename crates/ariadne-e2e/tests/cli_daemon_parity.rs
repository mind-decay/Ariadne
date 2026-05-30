//! `ariadne query` daemon-client parity — tier-10 (RD6).
//!
//! Drives the real `ariadne query` subcommand down both routes the tier wires:
//! the warm daemon and the cold in-process fallback. Asserts the two routes
//! return byte-identical JSON across several tools, and proves the warm route
//! is genuinely daemon-served — not a silent cold fallback — by deleting the
//! on-disk redb index while the daemon is up. The daemon dropped its storage
//! handle after building the warm catalog, so the graph lives in RAM and a
//! query with auto-spawn disabled can only be answered by the daemon; the cold
//! path bails on a missing index. `.ariadne/` is watcher-ignored
//! [src: crates/ariadne-watcher/src/adapters/ignore.rs:14], so the delete does
//! not perturb the live daemon.
//!
//! This is the tier-10 step-1 red→green coverage the audit (F1) required: the
//! daemon-served leg fails (cold bail on an absent index) without the CLI
//! daemon path, and passes with it. Offline + sub-second, so it runs on the
//! default `cargo nextest` pass.

use std::fs;
use std::path::Path;
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

use ariadne_e2e::domain::{ariadne_binary, run_index, run_init, run_query};
use serde_json::Value;
use tempfile::tempdir;

/// Auto-spawn switch the CLI daemon client reads; `"0"` disables the spawn
/// retry so the cold fallback is deterministic
/// [src: crates/ariadne-cli/src/adapters/daemon_client.rs:32].
const AUTOSPAWN_ENV: &str = "ARIADNE_CLI_AUTOSPAWN";

/// Fixture: two Rust files with cross-file calls — a non-empty symbol + edge
/// graph (mirrors `mcp_session.rs`).
const UTIL_RS: &str = "pub fn helper(value: i32) -> i32 {\n    value + 1\n}\n\n\
                       pub fn double(value: i32) -> i32 {\n    helper(value) + helper(value)\n}\n";
const MAIN_RS: &str = "fn compute() -> i32 {\n    double(20)\n}\n\n\
                       fn main() {\n    let _ = compute();\n}\n";

#[test]
fn cli_query_warm_and_cold_routes_agree_and_warm_is_daemon_served() {
    let project = tempdir().expect("create fixture tempdir");
    let root = project.path();
    fs::write(root.join("util.rs"), UTIL_RS).expect("write util.rs");
    fs::write(root.join("main.rs"), MAIN_RS).expect("write main.rs");

    run_init(root).expect("ariadne init on fixture");
    let report = run_index(root).expect("ariadne index on fixture");
    assert!(
        report.is_non_empty(),
        "fixture produced an empty graph: {report:?}",
    );

    // Seed a real symbol from the cold `list_symbols` output for the
    // symbol-keyed tools (mirrors `mcp_session.rs`).
    let listed = query_cold(root, "list_symbols", "{}");
    let symbol = first_symbol_name(&listed);
    let sym_args = format!(
        "{{\"symbol\":{}}}",
        serde_json::to_string(&symbol).expect("encode symbol arg"),
    );

    // A representative slice of the tier's 13-arm map: a list, the symbol-keyed
    // navigation + impact tools, and a scope-keyed analytics tool.
    let plan: Vec<(&str, String)> = vec![
        ("list_symbols", "{}".to_owned()),
        ("find_definition", sym_args.clone()),
        ("find_references", sym_args.clone()),
        ("blast_radius", sym_args),
        ("coupling_report", "{}".to_owned()),
    ];

    // Cold route: no daemon running, auto-spawn disabled — the in-process path.
    let cold: Vec<String> = plan
        .iter()
        .map(|(tool, args)| query_cold(root, tool, args))
        .collect();

    // Warm route: bring the daemon up, then route the same queries through it.
    // A reachable daemon answers over the socket without spawning, so the
    // helper's default (auto-spawn enabled) routes to the live daemon. The
    // guard reaps the detached daemon on scope exit *or* an assertion panic, so
    // a failing leg never orphans a background `ariadne daemon` process.
    let _daemon = DaemonGuard::start(root);
    let warm: Vec<String> = plan
        .iter()
        .map(|(tool, args)| {
            run_query(root, tool, args).unwrap_or_else(|e| panic!("warm `{tool}`: {e:#}"))
        })
        .collect();

    for ((tool, _), (cold_json, warm_json)) in plan.iter().zip(cold.iter().zip(warm.iter())) {
        assert_eq!(
            cold_json, warm_json,
            "warm and cold routes disagree for `{tool}`",
        );
    }

    // Daemon-served proof: drop the on-disk index. The warm graph is in RAM, so
    // a query with auto-spawn disabled can only be served by the daemon — the
    // cold path bails on the missing index. Without the CLI daemon path this
    // leg is red.
    let index = root.join(".ariadne").join("index.redb");
    fs::remove_file(&index).expect("remove redb index");
    assert!(
        !index.exists(),
        "index.redb must be gone for the daemon-served proof"
    );
    let served = query_cold(root, "list_symbols", "{}");
    assert_eq!(
        served, cold[0],
        "daemon must serve `list_symbols` from the warm graph after the index is deleted",
    );
}

/// Run `ariadne query <tool> <args> --root <root>` with auto-spawn disabled,
/// returning its stdout. Fails the test on a non-zero exit.
fn query_cold(root: &Path, tool: &str, args_json: &str) -> String {
    let output = Command::new(ariadne_binary())
        .args(["query", tool, args_json, "--root"])
        .arg(root)
        .env(AUTOSPAWN_ENV, "0")
        .output()
        .expect("spawn `ariadne query`");
    assert!(
        output.status.success(),
        "cold `ariadne query {tool}` failed: {}",
        String::from_utf8_lossy(&output.stderr).trim(),
    );
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// First named symbol in a `list_symbols` JSON array.
fn first_symbol_name(list_symbols_json: &str) -> String {
    let rows: Vec<Value> =
        serde_json::from_str(list_symbols_json).expect("parse list_symbols JSON array");
    rows.iter()
        .find_map(|row| row.get("name").and_then(Value::as_str))
        .map(str::to_owned)
        .expect("list_symbols exposed no named symbol")
}

/// Owns the lifetime of a background `ariadne daemon` for one project root.
/// `Drop` stops it, so the daemon is reaped on a clean scope exit and on an
/// assertion-panic unwind alike — a failing test never orphans a detached
/// daemon process.
struct DaemonGuard<'a> {
    root: &'a Path,
}

impl<'a> DaemonGuard<'a> {
    /// Spawn the daemon and poll until it reports running. The guard is armed
    /// before the readiness poll, so a startup-timeout panic still reaps.
    fn start(root: &'a Path) -> Self {
        let guard = Self { root };
        // A non-zero exit means "already running"; the status poll is the real
        // readiness gate, so the start result is intentionally ignored.
        let _ = Command::new(ariadne_binary())
            .args(["daemon", "start"])
            .arg(root)
            .output();
        let deadline = Instant::now() + Duration::from_secs(30);
        while !daemon_running(root) {
            assert!(
                Instant::now() < deadline,
                "daemon did not report running within 30s",
            );
            thread::sleep(Duration::from_millis(50));
        }
        guard
    }
}

impl Drop for DaemonGuard<'_> {
    fn drop(&mut self) {
        let _ = Command::new(ariadne_binary())
            .args(["daemon", "stop"])
            .arg(self.root)
            .output();
    }
}

/// Whether `ariadne daemon status <root>` reports a running daemon.
fn daemon_running(root: &Path) -> bool {
    let Ok(output) = Command::new(ariadne_binary())
        .args(["daemon", "status"])
        .arg(root)
        .output()
    else {
        return false;
    };
    String::from_utf8_lossy(&output.stdout).contains("running")
}
