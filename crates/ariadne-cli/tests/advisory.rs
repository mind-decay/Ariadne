//! `ariadne setup` `PreToolUse` advisory hook — drives the real `ariadne`
//! binary to install the advisor script + register the `PreToolUse` entry, then
//! exercises the installed script with representative payloads on stdin and
//! asserts the symbol-shaped classification. The advisory is non-blocking: a
//! symbol-shaped query returns `permissionDecision:"allow"` plus
//! `additionalContext` naming the Ariadne tool; everything else defers with no
//! context; it must never return `deny`/`ask` [src: tier-04 `<steps>`; plan.md
//! D5, R5; <https://code.claude.com/docs/en/hooks> `PreToolUse` schema].

#![cfg(unix)]

use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Command, Stdio};

use serde_json::Value;

/// Built `ariadne` binary under test [src: ariadne-cli `Cargo.toml` `[[bin]]`].
const BIN: &str = env!("CARGO_BIN_EXE_ariadne");
/// Project-root-relative path of the installed advisor script.
const ADVISOR_SCRIPT: &str = ".claude/hooks/ariadne-grep-advisor.sh";
/// The exact command string the advisory `PreToolUse` entry must register —
/// `${CLAUDE_PROJECT_DIR}` is substituted by Claude Code to the project root so
/// the hook is portable and cwd-independent [src: tier-03 D3d;
/// <https://code.claude.com/docs/en/hooks> placeholders].
const ADVISOR_COMMAND: &str = "${CLAUDE_PROJECT_DIR}/.claude/hooks/ariadne-grep-advisor.sh";

/// Run `ariadne setup <root>`; fail the test unless it exits successfully.
fn run_setup(root: &Path) {
    let status = Command::new(BIN)
        .arg("setup")
        .arg(root)
        .status()
        .expect("spawn `ariadne setup`");
    assert!(status.success(), "`ariadne setup` exited with {status}");
}

/// Pipe `payload` to the installed advisor script on stdin and return stdout.
/// The script is a pure classifier — no `ariadne` binary, no `jq` — so it must
/// exit 0 and emit a single JSON object regardless of input (fail-open).
fn run_advisor(script: &Path, payload: &str) -> String {
    let mut child = Command::new(script)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn advisor script");
    {
        let mut stdin = child.stdin.take().expect("advisor stdin");
        stdin.write_all(payload.as_bytes()).expect("write payload");
    } // drop stdin → EOF so the script's `cat` returns
    let out = child.wait_with_output().expect("advisor output");
    assert!(
        out.status.success(),
        "advisor must exit 0 (fail-open), got {} — stderr: {}",
        out.status,
        String::from_utf8_lossy(&out.stderr),
    );
    String::from_utf8(out.stdout).expect("advisor stdout is utf-8")
}

/// Install the advisor into a fresh temp project and return its path.
fn install_advisor() -> (tempfile::TempDir, std::path::PathBuf) {
    let tmp = tempfile::tempdir().expect("tempdir");
    run_setup(tmp.path());
    let script = tmp.path().join(ADVISOR_SCRIPT);
    assert!(script.is_file(), "the advisor script must exist on disk");
    (tmp, script)
}

#[test]
fn advisor_nudges_symbol_shaped_grep() {
    // A bare identifier and a `::`-separated path are both symbol-shaped: the
    // advisor must `allow` and inject context naming a navigation tool.
    let (_tmp, script) = install_advisor();
    for pattern in ["Catalog", "find_references", "crate::commands::setup"] {
        let payload = format!(r#"{{"tool_name":"Grep","tool_input":{{"pattern":"{pattern}"}}}}"#);
        let out = run_advisor(&script, &payload);
        let v: Value = serde_json::from_str(out.trim()).expect("advisor emits valid JSON");
        let hso = &v["hookSpecificOutput"];
        assert_eq!(
            hso["permissionDecision"], "allow",
            "symbol-shaped pattern `{pattern}` must be allowed with context, got: {out}",
        );
        let ctx = hso["additionalContext"]
            .as_str()
            .expect("a symbol-shaped match must carry additionalContext");
        assert!(
            ctx.contains("find_references") || ctx.contains("list_symbols"),
            "additionalContext must name a navigation tool, got: {ctx}",
        );
    }
}

#[test]
fn advisor_leads_with_definition_for_type_shapes() {
    // F1: a `::`-path or a CamelCase type is definition-shaped — the injected
    // context must lead with `find_definition` (its index precedes
    // `find_references`) [src: tier-04 audit F1; <steps> 2].
    let (_tmp, script) = install_advisor();
    for pattern in ["Catalog", "crate::commands::setup", "fooBar"] {
        let payload = format!(r#"{{"tool_name":"Grep","tool_input":{{"pattern":"{pattern}"}}}}"#);
        let out = run_advisor(&script, &payload);
        let v: Value = serde_json::from_str(out.trim()).expect("advisor emits valid JSON");
        let hso = &v["hookSpecificOutput"];
        assert_eq!(
            hso["permissionDecision"], "allow",
            "definition-shaped `{pattern}` must nudge, got: {out}",
        );
        let ctx = hso["additionalContext"]
            .as_str()
            .expect("a nudge must carry additionalContext");
        let def = ctx.find("find_definition").expect("names find_definition");
        let refs = ctx.find("find_references").expect("names find_references");
        assert!(
            def < refs,
            "a definition-shaped query must lead with find_definition, got: {ctx}",
        );
    }
}

#[test]
fn advisor_leads_with_references_for_snake_case() {
    // F1: a snake_case identifier is reference-shaped — the injected context must
    // lead with `find_references` (its index precedes `find_definition`)
    // [src: tier-04 audit F1; <steps> 2].
    let (_tmp, script) = install_advisor();
    for pattern in ["find_references", "merge_settings_json"] {
        let payload = format!(r#"{{"tool_name":"Grep","tool_input":{{"pattern":"{pattern}"}}}}"#);
        let out = run_advisor(&script, &payload);
        let v: Value = serde_json::from_str(out.trim()).expect("advisor emits valid JSON");
        let hso = &v["hookSpecificOutput"];
        assert_eq!(
            hso["permissionDecision"], "allow",
            "snake_case `{pattern}` must nudge, got: {out}",
        );
        let ctx = hso["additionalContext"]
            .as_str()
            .expect("a nudge must carry additionalContext");
        let refs = ctx.find("find_references").expect("names find_references");
        let def = ctx.find("find_definition").expect("names find_definition");
        assert!(
            refs < def,
            "a snake_case query must lead with find_references, got: {ctx}",
        );
    }
}

#[test]
fn advisor_defers_bare_free_text_word() {
    // F2 (structural floor): a bare all-lowercase or all-caps word with no
    // `_`/`::`/case-mix is free-text-shaped (error, TODO, render) and must defer
    // even though it matches the bare-identifier regex — the residual
    // false-positive class R5 trades away [src: tier-04 audit F2; plan.md R5].
    let (_tmp, script) = install_advisor();
    for pattern in ["error", "TODO", "todo", "render", "info", "X"] {
        let payload = format!(r#"{{"tool_name":"Grep","tool_input":{{"pattern":"{pattern}"}}}}"#);
        let out = run_advisor(&script, &payload);
        let v: Value = serde_json::from_str(out.trim()).expect("advisor emits valid JSON");
        let hso = &v["hookSpecificOutput"];
        assert_eq!(
            hso["permissionDecision"], "defer",
            "a bare free-text word `{pattern}` must defer, got: {out}",
        );
        assert!(
            hso.get("additionalContext").is_none(),
            "a deferred word must carry no additionalContext, got: {out}",
        );
    }
}

#[test]
fn advisor_defers_quoted_log_string() {
    // A whitespace-containing phrase (a log string / free text) is not
    // symbol-shaped: pass through with no added context.
    let (_tmp, script) = install_advisor();
    let payload = r#"{"tool_name":"Grep","tool_input":{"pattern":"failed to connect to daemon"}}"#;
    let out = run_advisor(&script, payload);
    let v: Value = serde_json::from_str(out.trim()).expect("advisor emits valid JSON");
    let hso = &v["hookSpecificOutput"];
    assert_eq!(
        hso["permissionDecision"], "defer",
        "a quoted log string must defer, got: {out}",
    );
    assert!(
        hso.get("additionalContext").is_none(),
        "a deferred query must carry no additionalContext, got: {out}",
    );
}

#[test]
fn advisor_defers_non_source_paths() {
    // A glob over docs and a `Read` of a markdown file are not symbol lookups:
    // both defer untouched.
    let (_tmp, script) = install_advisor();
    for payload in [
        r#"{"tool_name":"Glob","tool_input":{"pattern":"**/*.md"}}"#,
        r#"{"tool_name":"Read","tool_input":{"file_path":"README.md"}}"#,
    ] {
        let out = run_advisor(&script, payload);
        let v: Value = serde_json::from_str(out.trim()).expect("advisor emits valid JSON");
        assert_eq!(
            v["hookSpecificOutput"]["permissionDecision"], "defer",
            "non-source path must defer, got: {out}",
        );
    }
}

#[test]
fn advisor_never_denies_or_asks() {
    // D5: the advisory must never block a legitimate search. Across every shape
    // — symbol, phrase, glob, path, empty, garbage — the decision is only ever
    // `allow` or `defer`, never `deny`/`ask`.
    let (_tmp, script) = install_advisor();
    for payload in [
        r#"{"tool_name":"Grep","tool_input":{"pattern":"Catalog"}}"#,
        r#"{"tool_name":"Grep","tool_input":{"pattern":"a phrase with spaces"}}"#,
        r#"{"tool_name":"Glob","tool_input":{"pattern":"src/**/*.rs"}}"#,
        r#"{"tool_name":"Read","tool_input":{"file_path":"/etc/hosts"}}"#,
        r#"{"tool_name":"Bash","tool_input":{"command":"ls"}}"#,
        "",
        "not json at all",
    ] {
        let out = run_advisor(&script, payload);
        assert!(
            !out.contains("\"deny\"") && !out.contains("\"ask\""),
            "advisor must never deny/ask (payload {payload:?}), got: {out}",
        );
    }
}

#[test]
fn setup_installs_pretooluse_advisory_preserving_audit_gate() {
    // Tier-04: `setup` registers the advisory `PreToolUse` entry (matcher
    // `Grep|Glob|Read`) by deep-merging into `hooks.PreToolUse` — the existing
    // Bash audit-gate must survive the merge semantically, and a re-run must be
    // byte-idempotent [src: tier-04 exit_criteria; <steps> 4].
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path();
    std::fs::create_dir_all(root.join(".claude")).unwrap();
    // Pre-seed the repo's Bash audit-gate hook (keys deliberately unsorted so
    // the merge's re-serialization is exercised).
    let existing_raw = "{\n  \
        \"hooks\": {\n    \
            \"PreToolUse\": [\n      {\n        \
                \"matcher\": \"Bash\",\n        \
                \"hooks\": [\n          \
                    { \"type\": \"command\", \"command\": \"./.claude/hooks/audit-gate.sh\" }\n        \
                ]\n      }\n    ]\n  }\n}\n";
    std::fs::write(root.join(".claude/settings.json"), existing_raw).unwrap();

    run_setup(root);

    let settings: Value =
        serde_json::from_str(&std::fs::read_to_string(root.join(".claude/settings.json")).unwrap())
            .expect(".claude/settings.json must be valid JSON");
    let pre = settings["hooks"]["PreToolUse"]
        .as_array()
        .expect("hooks.PreToolUse must be a JSON array");

    // The Bash audit-gate still resolves after the merge.
    assert!(
        pre.iter().any(|e| {
            e["matcher"] == "Bash"
                && e["hooks"].as_array().is_some_and(|h| {
                    h.iter()
                        .any(|x| x["command"].as_str() == Some("./.claude/hooks/audit-gate.sh"))
                })
        }),
        "the Bash audit-gate must survive the merge",
    );
    // The advisory entry registers the advisor script for Grep|Glob|Read.
    assert!(
        pre.iter().any(|e| {
            e["matcher"] == "Grep|Glob|Read"
                && e["hooks"].as_array().is_some_and(|h| {
                    h.iter()
                        .any(|x| x["command"].as_str() == Some(ADVISOR_COMMAND))
                })
        }),
        "hooks.PreToolUse must register the advisor for Grep|Glob|Read, got: {settings:#}",
    );

    // The script exists and is executable.
    let script = root.join(ADVISOR_SCRIPT);
    assert!(script.is_file(), "advisor script must exist");
    let mode = std::fs::metadata(&script).unwrap().permissions().mode();
    assert!(mode & 0o111 != 0, "advisor script must be executable");

    // Idempotent: a second run leaves settings.json byte-identical.
    let first = std::fs::read(root.join(".claude/settings.json")).unwrap();
    run_setup(root);
    let second = std::fs::read(root.join(".claude/settings.json")).unwrap();
    assert_eq!(
        first, second,
        "a second `setup` run must leave settings.json byte-identical",
    );
}
