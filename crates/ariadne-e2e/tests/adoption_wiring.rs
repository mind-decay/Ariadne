//! Deterministic adoption-wiring gate — `ariadne setup` on a temp project must
//! compose the four installs (tiers 01–04) into one consistent surface, and
//! `ariadne digest` must emit a bounded, non-empty document.
//!
//! This is the integration gate that asserts the four installs compose: the
//! `.mcp.json` `alwaysLoad` (tier-01), the `digest` command (tier-02), the
//! `SessionStart` hook (tier-03), and the `PreToolUse` advisory (tier-04). A
//! foreign `PreToolUse` entry is pre-seeded so the test also proves the merge
//! preserves any pre-existing hook. Offline and sub-second, so it runs on the
//! default `cargo nextest` pass; the behavioral adoption ratio is measured
//! separately by the `#[ignore]`d harness (`adoption_harness.rs`)
//! [src: .claude/plans/ariadne-mcp-adoption/tier-05-adoption-eval.md steps 1-2].

use std::fs;
use std::path::Path;
use std::process::Command;

use ariadne_e2e::domain::{ariadne_binary, run_index, run_setup};
use serde_json::Value;
use tempfile::tempdir;

/// Upper bound on the digest size — the `additionalContext` cap a
/// `SessionStart` hook injects, asserted to keep the bootstrap well clear of
/// truncation (plan.md `<constraints>` R4; tier-05 exit criteria).
const DIGEST_MAX_BYTES: usize = 10_000;

/// `${CLAUDE_PROJECT_DIR}` command string `setup` registers for the
/// `SessionStart` digest hook [src: commands/setup.rs `SESSION_START_COMMAND`].
const SESSION_START_COMMAND: &str = "${CLAUDE_PROJECT_DIR}/.claude/hooks/ariadne-session-start.sh";
/// `${CLAUDE_PROJECT_DIR}` command string `setup` registers for the advisory
/// `PreToolUse` hook [src: commands/setup.rs `ADVISOR_COMMAND`].
const ADVISOR_COMMAND: &str = "${CLAUDE_PROJECT_DIR}/.claude/hooks/ariadne-grep-advisor.sh";
/// The advisory matcher `setup` registers [src: commands/setup.rs
/// `ADVISOR_MATCHER`].
const ADVISOR_MATCHER: &str = "Grep|Glob|Read";

/// A foreign `PreToolUse` entry pre-seeded into the temp project's settings,
/// standing in for this repo's own Bash audit-gate. The merge must leave it
/// intact ("plus any pre-existing", tier-05 step 1).
const FOREIGN_COMMAND: &str = "./.claude/hooks/audit-gate.sh";

/// Small multi-file Rust fixture with a cross-file call, so `ariadne index`
/// produces a non-empty graph and `ariadne digest` renders real composed
/// content rather than the empty-graph fallback (mirrors `mcp_session.rs`).
const UTIL_RS: &str = "pub fn helper(value: i32) -> i32 {\n    value + 1\n}\n\n\
                       pub fn double(value: i32) -> i32 {\n    helper(value) + helper(value)\n}\n";
const MAIN_RS: &str = "fn compute() -> i32 {\n    double(20)\n}\n\n\
                       fn main() {\n    let _ = compute();\n}\n";

#[test]
fn setup_composes_the_full_adoption_wiring() {
    let project = tempdir().expect("create fixture tempdir");
    let root = project.path();
    fs::write(root.join("util.rs"), UTIL_RS).expect("write util.rs");
    fs::write(root.join("main.rs"), MAIN_RS).expect("write main.rs");

    // Pre-seed a foreign PreToolUse hook so the merge's "preserve pre-existing"
    // contract is exercised, not just asserted in prose.
    seed_foreign_settings(root);

    run_setup(root).expect("ariadne setup on fixture");

    assert_mcp_always_load(root);
    assert_settings_wiring(root);
    assert_hook_scripts_executable(root);

    // `ariadne index` so the digest cold path renders real analytics; the
    // exit-criteria digest assert then exercises the composed pipeline, not the
    // empty-graph fallback.
    let report = run_index(root).expect("ariadne index on fixture");
    assert!(
        report.is_non_empty(),
        "fixture produced an empty graph: {report:?}",
    );
    assert_digest_bounded_nonempty(root);
}

/// Write a `.claude/settings.json` carrying only a foreign Bash `PreToolUse`
/// entry, so `setup`'s merge has a pre-existing hook to preserve.
fn seed_foreign_settings(root: &Path) {
    let dir = root.join(".claude");
    fs::create_dir_all(&dir).expect("create .claude dir");
    let seed = serde_json::json!({
        "hooks": {
            "PreToolUse": [
                {
                    "matcher": "Bash",
                    "hooks": [ { "type": "command", "command": FOREIGN_COMMAND } ]
                }
            ]
        }
    });
    fs::write(
        dir.join("settings.json"),
        serde_json::to_string_pretty(&seed).expect("render seed settings"),
    )
    .expect("write seed settings.json");
}

/// `.mcp.json` registers the `ariadne` server with `alwaysLoad: true` (tier-01,
/// D1) — the deferral-exemption that keeps every tool description loaded.
fn assert_mcp_always_load(root: &Path) {
    let config = read_json(&root.join(".mcp.json"));
    let always_load = config
        .pointer("/mcpServers/ariadne/alwaysLoad")
        .and_then(Value::as_bool);
    assert_eq!(
        always_load,
        Some(true),
        "`.mcp.json` ariadne entry missing `alwaysLoad: true`: {config}",
    );
}

/// `.claude/settings.json` carries the `SessionStart` digest hook, the
/// `Grep|Glob|Read` advisory `PreToolUse` entry, and still the pre-seeded
/// foreign Bash entry (tiers 03/04; merge preserves pre-existing).
fn assert_settings_wiring(root: &Path) {
    let config = read_json(&root.join(".claude/settings.json"));

    let session_start = config
        .pointer("/hooks/SessionStart")
        .and_then(Value::as_array)
        .expect("settings.json has a SessionStart array");
    assert!(
        session_start
            .iter()
            .any(|e| entry_has_command(e, SESSION_START_COMMAND)),
        "SessionStart missing the digest hook: {session_start:?}",
    );

    let pre_tool_use = config
        .pointer("/hooks/PreToolUse")
        .and_then(Value::as_array)
        .expect("settings.json has a PreToolUse array");
    assert!(
        pre_tool_use.iter().any(|e| {
            e.get("matcher").and_then(Value::as_str) == Some(ADVISOR_MATCHER)
                && entry_has_command(e, ADVISOR_COMMAND)
        }),
        "PreToolUse missing the `{ADVISOR_MATCHER}` advisory entry: {pre_tool_use:?}",
    );
    assert!(
        pre_tool_use
            .iter()
            .any(|e| entry_has_command(e, FOREIGN_COMMAND)),
        "PreToolUse dropped the pre-existing foreign entry: {pre_tool_use:?}",
    );
}

/// Both installed hook scripts exist and are executable.
fn assert_hook_scripts_executable(root: &Path) {
    for script in [
        ".claude/hooks/ariadne-session-start.sh",
        ".claude/hooks/ariadne-grep-advisor.sh",
    ] {
        let path = root.join(script);
        assert!(path.is_file(), "hook script missing: {}", path.display());
        assert!(
            is_executable(&path),
            "hook script not executable: {}",
            path.display(),
        );
    }
}

/// `ariadne digest` exits 0 and prints a non-empty document under the
/// `additionalContext` cap. `ARIADNE_CLI_AUTOSPAWN=0` forces the synchronous
/// cold path (no daemon to spawn or reap) so, with the index present, the
/// composed digest renders deterministically rather than the fallback.
fn assert_digest_bounded_nonempty(root: &Path) {
    let output = Command::new(ariadne_binary())
        .arg("digest")
        .arg(root)
        .env("ARIADNE_CLI_AUTOSPAWN", "0")
        .output()
        .expect("spawn `ariadne digest`");
    assert!(
        output.status.success(),
        "ariadne digest exited non-zero: {}",
        String::from_utf8_lossy(&output.stderr).trim(),
    );
    assert!(!output.stdout.is_empty(), "digest printed empty stdout");
    assert!(
        output.stdout.len() < DIGEST_MAX_BYTES,
        "digest {} bytes, over the {DIGEST_MAX_BYTES}-byte cap",
        output.stdout.len(),
    );
    let text = String::from_utf8(output.stdout).expect("digest stdout is UTF-8");
    assert!(
        text.contains("## Ariadne project digest"),
        "digest did not render the composed document (fell back?):\n{text}",
    );
}

/// Whether a hook array entry carries `command` in its `hooks` list.
fn entry_has_command(entry: &Value, command: &str) -> bool {
    entry
        .get("hooks")
        .and_then(Value::as_array)
        .is_some_and(|hooks| {
            hooks
                .iter()
                .any(|h| h.get("command").and_then(Value::as_str) == Some(command))
        })
}

/// Read and parse a JSON file, panicking with the path on any failure.
fn read_json(path: &Path) -> Value {
    let text = fs::read_to_string(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("parse {}: {e}", path.display()))
}

/// Whether `path` carries any executable mode bit on Unix; always true on
/// targets without POSIX mode bits (mirrors `setup`'s `set_executable` cfg).
#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    fs::metadata(path).is_ok_and(|m| m.permissions().mode() & 0o111 != 0)
}

/// Non-Unix targets have no POSIX mode bits — `setup` marks nothing executable,
/// so the check is vacuously true.
#[cfg(not(unix))]
fn is_executable(_path: &Path) -> bool {
    true
}
