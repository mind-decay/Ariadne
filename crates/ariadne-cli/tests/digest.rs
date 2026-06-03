//! `ariadne digest` golden-shape + length-bound test (tier-02).
//!
//! Indexes a small cross-file Rust fixture, runs the real `ariadne digest`
//! subcommand through the cold in-process path (auto-spawn disabled, so no
//! daemon is started and the result is deterministic), and asserts the emitted
//! Markdown carries the agent-shaped sections and stays well under the
//! 10 000-char `additionalContext` cap a `SessionStart` hook injects
//! [src: .claude/plans/ariadne-mcp-adoption/tier-02-digest-command.md
//! `exit_criteria`; plan.md R4].

use std::path::Path;
use std::process::Command;

/// Default binary under test (the workspace `ariadne` build).
fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_ariadne")
}

/// Auto-spawn switch the CLI daemon client reads; `"0"` disables the spawn
/// retry so the digest resolves through the cold in-process path
/// [src: crates/ariadne-cli/src/adapters/daemon_client.rs:32].
const AUTOSPAWN_ENV: &str = "ARIADNE_CLI_AUTOSPAWN";

/// Fixture: two Rust files with a cross-file call — a non-empty symbol + edge
/// graph so `coupling_report` lists real modules.
const UTIL_RS: &str = "pub fn helper(value: i32) -> i32 {\n    value + 1\n}\n\n\
                       pub fn double(value: i32) -> i32 {\n    helper(value) + helper(value)\n}\n";
const MAIN_RS: &str = "fn compute() -> i32 {\n    double(20)\n}\n\n\
                       fn main() {\n    let _ = compute();\n}\n";

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

/// Run `ariadne digest <root>` with auto-spawn disabled, returning its stdout.
fn run_digest(root: &Path) -> String {
    let output = Command::new(bin())
        .arg("digest")
        .arg(root)
        .env(AUTOSPAWN_ENV, "0")
        .output()
        .expect("spawn `ariadne digest`");
    assert!(
        output.status.success(),
        "`ariadne digest` exited with {}: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr).trim(),
    );
    String::from_utf8_lossy(&output.stdout).into_owned()
}

#[test]
fn digest_emits_bounded_agent_shaped_markdown() {
    let project = tempfile::tempdir().expect("create fixture tempdir");
    let root = project.path();
    std::fs::write(root.join("util.rs"), UTIL_RS).expect("write util.rs");
    std::fs::write(root.join("main.rs"), MAIN_RS).expect("write main.rs");

    run_ok(&["init", root.to_str().expect("utf8 root")]);
    run_ok(&["index", root.to_str().expect("utf8 root")]);

    let digest = run_digest(root);

    assert!(
        digest.contains("## Ariadne"),
        "digest missing `## Ariadne` heading:\n{digest}",
    );
    assert!(
        digest.contains("revision"),
        "digest missing a revision line:\n{digest}",
    );
    assert!(
        digest.contains("Top modules"),
        "digest missing the `Top modules` section:\n{digest}",
    );
    assert!(
        digest.contains("When to use which tool"),
        "digest missing the `When to use which tool` cheat-sheet:\n{digest}",
    );
    assert!(
        !digest.contains("![architecture]"),
        "digest leaked the project overview's sidecar SVG reference \
         (overview_slice must drop the `## Architecture` diagram):\n{digest}",
    );
    assert!(!digest.trim().is_empty(), "digest was empty");
    assert!(
        digest.len() < 10_000,
        "digest exceeded the 10k cap: {} bytes",
        digest.len(),
    );
}
