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
/// The exact command string the `SessionStart` entry must register —
/// `${CLAUDE_PROJECT_DIR}` is substituted by Claude Code to the project root so
/// the hook is portable and cwd-independent [src: tier-03 D3d;
/// <https://code.claude.com/docs/en/hooks> placeholders].
const SESSION_START_COMMAND: &str = "${CLAUDE_PROJECT_DIR}/.claude/hooks/ariadne-session-start.sh";

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

    // The discoverability block must list the full tool surface, including the
    // history-analytics and diff-blast-radius tools later tiers added — guards
    // against the render template drifting behind the shipped MCP tools and
    // silently downgrading a consumer's CLAUDE.md on `setup`.
    for tool in [
        "diff_blast_radius",
        "hotspots",
        "complexity",
        "co_change",
        "search_code",
        "read_symbol",
    ] {
        assert!(
            claude.contains(tool),
            "CLAUDE.md Ariadne block must mention `{tool}`",
        );
    }
}

#[test]
fn setup_writes_always_load_into_ariadne_entry() {
    // Tier-01 D1: exempt the `ariadne` server from MCP Tool Search deferral by
    // writing `"alwaysLoad": true` into its `.mcp.json` entry, leaving any
    // foreign server untouched.
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
        mcp["mcpServers"]["ariadne"]["alwaysLoad"],
        serde_json::json!(true),
        "the ariadne entry must carry `alwaysLoad: true`",
    );
    assert_eq!(
        mcp["mcpServers"]["other"], foreign["mcpServers"]["other"],
        "the foreign entry must survive verbatim, with no `alwaysLoad` added",
    );
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
fn setup_installs_session_start_hook_preserving_existing_hooks() {
    // Tier-03: `setup` installs the SessionStart hook script and registers it in
    // `.claude/settings.json` by deep-merging the `hooks` object — the existing
    // PreToolUse audit-gate must survive the merge semantically (its matcher and
    // command resolve), and a re-run must be byte-idempotent. `serde_json`
    // re-sorts object keys on serialize (`Value` is a `BTreeMap`), so byte
    // identity is not guaranteed for unsorted input — only the keys and values
    // are [src: tier-03 exit_criteria; D3d; audit/tier-03-report.md F2].
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path();

    // Pre-seed an existing settings.json carrying a foreign PreToolUse hook
    // (mirrors this repo's audit-gate) the merge must preserve. Keys are in
    // DELIBERATELY non-alphabetical order (`hooks` before `$schema`, `matcher`
    // before `hooks`, `type` before `command`) so the merge's re-serialization,
    // which sorts keys, is actually exercised — not masked by pre-sorted input
    // [src: audit/tier-03-report.md F2].
    std::fs::create_dir_all(root.join(".claude")).unwrap();
    let existing_raw = "{\n  \
        \"hooks\": {\n    \
            \"PreToolUse\": [\n      {\n        \
                \"matcher\": \"Bash\",\n        \
                \"hooks\": [\n          \
                    { \"type\": \"command\", \"command\": \"./.claude/hooks/audit-gate.sh\" }\n        \
                ]\n      }\n    ]\n  },\n  \
        \"$schema\": \"https://json.schemastore.org/claude-code-settings.json\"\n}\n";
    let existing: Value = serde_json::from_str(existing_raw).expect("seed JSON parses");
    std::fs::write(root.join(".claude/settings.json"), existing_raw).unwrap();

    run_setup(root);

    let settings: Value =
        serde_json::from_str(&std::fs::read_to_string(root.join(".claude/settings.json")).unwrap())
            .expect(".claude/settings.json must be valid JSON");

    // The foreign PreToolUse audit-gate survives the merge semantically: `Value`
    // equality is order-insensitive, so the seed's Bash entry is still a member
    // even though the on-disk key order was normalized (the input was
    // deliberately unsorted). Tier-04 adds a sibling `Grep|Glob|Read` advisory
    // entry, so the array now holds both rather than only the seed — hence
    // membership, not whole-array equality.
    let seed_bash = &existing["hooks"]["PreToolUse"][0];
    assert!(
        settings["hooks"]["PreToolUse"]
            .as_array()
            .is_some_and(|arr| arr.contains(seed_bash)),
        "the existing PreToolUse audit-gate hook must survive the merge semantically",
    );
    // And it still resolves: the audit-gate matcher + command are reachable by
    // navigation after the merge re-sorted the unsorted input's keys.
    let pretooluse = settings["hooks"]["PreToolUse"]
        .as_array()
        .expect("hooks.PreToolUse must be a JSON array");
    assert!(
        pretooluse.iter().any(|entry| {
            entry["matcher"] == serde_json::json!("Bash")
                && entry["hooks"].as_array().is_some_and(|hooks| {
                    hooks
                        .iter()
                        .any(|h| h["command"].as_str() == Some("./.claude/hooks/audit-gate.sh"))
                })
        }),
        "the audit-gate (matcher=Bash, command=audit-gate.sh) must still resolve after the merge",
    );

    // A SessionStart entry now registers the installed script, with no matcher
    // (the entry fires on every session start).
    let session_start = settings["hooks"]["SessionStart"]
        .as_array()
        .expect("hooks.SessionStart must be a JSON array");
    assert!(
        session_start.iter().any(|entry| {
            entry["hooks"].as_array().is_some_and(|hooks| {
                hooks
                    .iter()
                    .any(|h| h["command"].as_str() == Some(SESSION_START_COMMAND))
            })
        }),
        "hooks.SessionStart must register the ariadne-session-start.sh script",
    );

    // The script exists and is executable.
    let script = root.join(".claude/hooks/ariadne-session-start.sh");
    assert!(script.is_file(), "the hook script must exist on disk");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&script).unwrap().permissions().mode();
        assert!(mode & 0o111 != 0, "the hook script must be executable");
    }

    // Idempotent: a second run leaves settings.json byte-identical.
    let first = std::fs::read(root.join(".claude/settings.json")).unwrap();
    run_setup(root);
    let second = std::fs::read(root.join(".claude/settings.json")).unwrap();
    assert_eq!(
        first, second,
        "a second `setup` run must leave settings.json byte-identical",
    );
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
