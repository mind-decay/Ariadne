//! `ariadne setup` integration test — drives the real `ariadne` binary and
//! asserts the one-shot onboarding writes `.ariadne/` config, merges
//! `.mcp.json`, and refreshes the `CLAUDE.md` marker block: non-destructively
//! (foreign content survives) and idempotently (a second run is a no-op)
//! [src: .claude/plans/ariadne-core/tier-16-setup-command.md `<steps>` 1].

use std::path::Path;
use std::process::Command;

use serde_json::Value;

/// Built `ariadne` binary under test [src: ariadne-cli `Cargo.toml` `[[bin]]`].
const BIN: &str = env!("CARGO_BIN_EXE_ariadne");
const BEGIN: &str = "<!-- BEGIN ARIADNE -->";
const END: &str = "<!-- END ARIADNE -->";

/// Run `ariadne setup <root>`; fail the test unless it exits successfully.
fn run_setup(root: &Path) {
    let status = Command::new(BIN)
        .arg("setup")
        .arg(root)
        .status()
        .expect("spawn `ariadne setup`");
    assert!(status.success(), "`ariadne setup` exited with {status}");
}

/// Count non-overlapping occurrences of `needle` in `haystack`.
fn count(haystack: &str, needle: &str) -> usize {
    haystack.matches(needle).count()
}

/// Read the three setup artifacts as raw bytes, in a fixed order.
fn snapshot(root: &Path) -> Vec<Vec<u8>> {
    [".ariadne/config.toml", ".mcp.json", "CLAUDE.md"]
        .iter()
        .map(|rel| std::fs::read(root.join(rel)).expect("read setup artifact"))
        .collect()
}

#[test]
fn setup_writes_all_three_artifacts() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path();

    run_setup(root);

    assert!(
        root.join(".ariadne/config.toml").is_file(),
        ".ariadne/config.toml must exist",
    );
    assert!(root.join(".mcp.json").is_file(), ".mcp.json must exist");
    assert!(root.join("CLAUDE.md").is_file(), "CLAUDE.md must exist");

    let mcp: Value =
        serde_json::from_str(&std::fs::read_to_string(root.join(".mcp.json")).unwrap())
            .expect(".mcp.json must be valid JSON");
    assert_eq!(
        mcp["mcpServers"]["ariadne"]["args"],
        serde_json::json!(["serve", "--watch"]),
        "ariadne MCP entry must carry `serve --watch`",
    );

    let claude = std::fs::read_to_string(root.join("CLAUDE.md")).unwrap();
    assert_eq!(count(&claude, BEGIN), 1, "BEGIN marker must appear once");
    assert_eq!(count(&claude, END), 1, "END marker must appear once");
}

#[test]
fn setup_preserves_foreign_mcp_entry() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path();
    let foreign = serde_json::json!({
        "mcpServers": {
            "other": { "command": "other-bin", "args": ["x"], "env": {} }
        }
    });
    std::fs::write(
        root.join(".mcp.json"),
        serde_json::to_string_pretty(&foreign).unwrap(),
    )
    .unwrap();

    run_setup(root);

    let mcp: Value =
        serde_json::from_str(&std::fs::read_to_string(root.join(".mcp.json")).unwrap()).unwrap();
    assert_eq!(
        mcp["mcpServers"]["other"], foreign["mcpServers"]["other"],
        "a pre-existing foreign mcpServers entry must survive verbatim",
    );
    assert_eq!(
        mcp["mcpServers"]["ariadne"]["args"],
        serde_json::json!(["serve", "--watch"]),
        "the ariadne entry must still be inserted alongside it",
    );
}

#[test]
fn setup_preserves_user_claude_prose_and_refreshes_block_in_place() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path();
    let prose = "# My Project\n\nUser-authored guidance that must survive.\n";
    std::fs::write(root.join("CLAUDE.md"), prose).unwrap();

    run_setup(root);
    run_setup(root);

    let claude = std::fs::read_to_string(root.join("CLAUDE.md")).unwrap();
    assert!(
        claude.contains("User-authored guidance that must survive."),
        "user CLAUDE.md prose must survive setup",
    );
    assert_eq!(count(&claude, BEGIN), 1, "re-run must not duplicate BEGIN");
    assert_eq!(count(&claude, END), 1, "re-run must not duplicate END");
}

#[test]
fn setup_is_byte_idempotent() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path();
    // Pre-seed both caller-owned files so the merge paths are exercised.
    std::fs::write(
        root.join(".mcp.json"),
        "{\n  \"mcpServers\": {\n    \"other\": { \"command\": \"x\" }\n  }\n}\n",
    )
    .unwrap();
    std::fs::write(root.join("CLAUDE.md"), "# Prose\n").unwrap();

    run_setup(root);
    let first = snapshot(root);
    run_setup(root);
    let second = snapshot(root);

    assert_eq!(
        first, second,
        "a second consecutive `setup` run must leave all three artifacts byte-identical",
    );
}
