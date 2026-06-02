//! Regression for audit INFO item F1: `ariadne query` must print object keys in
//! struct-declaration order (`revision` first), not the alphabetical order a
//! round-trip through an order-less `serde_json::Value` (a `BTreeMap` without
//! the `preserve_order` feature) produces. Pins step 2's "no behavior change to
//! query" promise against the tier-02 digest refactor
//! [src: .claude/plans/ariadne-mcp-adoption/audit/tier-02-report.md F1].

use std::path::Path;
use std::process::Command;

/// Binary under test (the workspace `ariadne` build).
fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_ariadne")
}

/// Auto-spawn switch; `"0"` resolves the query through the cold in-process path
/// so no daemon is started and the output is deterministic
/// [src: crates/ariadne-cli/src/adapters/daemon_client.rs:32].
const AUTOSPAWN_ENV: &str = "ARIADNE_CLI_AUTOSPAWN";

/// Fixture: one Rust file with a single function — a non-empty graph so
/// `project_status` reports real counts.
const MAIN_RS: &str = "pub fn helper(value: i32) -> i32 {\n    value + 1\n}\n";

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

/// Run `ariadne query <tool> <args> --root <root>` cold, returning its stdout.
fn run_query(root: &Path, tool: &str, args_json: &str) -> String {
    let output = Command::new(bin())
        .args(["query", tool, args_json, "--root"])
        .arg(root)
        .env(AUTOSPAWN_ENV, "0")
        .output()
        .expect("spawn `ariadne query`");
    assert!(
        output.status.success(),
        "`ariadne query {tool}` exited with {}: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr).trim(),
    );
    String::from_utf8_lossy(&output.stdout).into_owned()
}

#[test]
fn query_preserves_struct_declaration_key_order() {
    let project = tempfile::tempdir().expect("create fixture tempdir");
    let root = project.path();
    std::fs::write(root.join("main.rs"), MAIN_RS).expect("write main.rs");

    run_ok(&["init", root.to_str().expect("utf8 root")]);
    run_ok(&["index", root.to_str().expect("utf8 root")]);

    let json = run_query(root, "project_status", "{}");

    let revision = json
        .find("\"revision\"")
        .unwrap_or_else(|| panic!("query output missing `revision` key:\n{json}"));
    let edge_count = json
        .find("\"edge_count\"")
        .unwrap_or_else(|| panic!("query output missing `edge_count` key:\n{json}"));
    assert!(
        revision < edge_count,
        "`ariadne query project_status` keys are not in declaration order \
         (`revision` must precede `edge_count`); got alphabetical Value ordering:\n{json}",
    );
}
