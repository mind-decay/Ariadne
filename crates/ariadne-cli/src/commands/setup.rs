//! `ariadne setup` — one-shot project onboarding.
//!
//! Reuses `ariadne init` for the `.ariadne/` config scaffolding, then wires
//! two out-of-binary surfaces: the `ariadne` entry in the project's
//! `.mcp.json` so Claude Code spawns the MCP server, and a marker-delimited
//! discoverability block in `CLAUDE.md` so the agent prefers the Ariadne
//! tools. Both writes are non-destructive and idempotent. Runs no index —
//! that stays an explicit `ariadne index` step
//! [src: .claude/plans/ariadne-core/tier-16-setup-command.md].

use std::path::Path;

use anyhow::{Context, Result};
use serde_json::{Value, json};

/// Opening delimiter of the Ariadne block in `CLAUDE.md`.
const BEGIN_MARKER: &str = "<!-- BEGIN ARIADNE -->";
/// Closing delimiter of the Ariadne block in `CLAUDE.md`.
const END_MARKER: &str = "<!-- END ARIADNE -->";

/// Project-root-relative path of the installed `SessionStart` hook script.
const SESSION_START_SCRIPT: &str = ".claude/hooks/ariadne-session-start.sh";

/// Command string registered in `.claude/settings.json` for the `SessionStart`
/// entry. Claude Code substitutes `${CLAUDE_PROJECT_DIR}` with the project root
/// and also exports it into the hook's environment, so the entry is portable
/// across checkouts and independent of the hook's working directory
/// [src: <https://code.claude.com/docs/en/hooks> placeholders; plan.md D3].
const SESSION_START_COMMAND: &str = "${CLAUDE_PROJECT_DIR}/.claude/hooks/ariadne-session-start.sh";

/// POSIX `sh` template for the `SessionStart` hook. `__ARIADNE_BIN__` is
/// replaced with the absolute `ariadne` path at install time. The hook runs
/// `ariadne digest "$CLAUDE_PROJECT_DIR"` and emits the digest as factual
/// `additionalContext`; it is fail-open — a missing binary, an empty digest, or
/// any error prints a minimal factual fallback and exits 0, never a non-zero
/// exit or malformed JSON (both surface as a `hook error` and defeat the
/// bootstrap) [src: plan.md D3a/D3b/D3c;
/// <https://code.claude.com/docs/en/hooks> output schema].
const SESSION_START_HOOK: &str = r#"#!/usr/bin/env sh
# Ariadne SessionStart hook — injects the project digest as factual
# `additionalContext` before the first prompt (<=10k chars). Installed by
# `ariadne setup`; do not edit by hand. Fail-open: any failure prints a minimal
# factual fallback and exits 0, never a non-zero exit or malformed JSON (both
# surface as a `hook error` and defeat the bootstrap)
# [src: https://code.claude.com/docs/en/hooks SessionStart; plan.md D3b/D3c].

set -u

# Absolute `ariadne` path resolved by `ariadne setup` at install time (mirrors
# the `.mcp.json` `command` entry), so the hook works when `ariadne` is not on
# PATH.
BIN='__ARIADNE_BIN__'

# Claude Code exports CLAUDE_PROJECT_DIR into the hook environment; fall back to
# the current directory when the script is run by hand.
DIR="${CLAUDE_PROJECT_DIR:-.}"

# Minimal factual fallback, phrased as project state rather than an instruction
# (out-of-band imperative text trips prompt-injection defenses) [src: plan.md D3].
FALLBACK="Ariadne's read-only semantic graph is configured for this project. The Ariadne MCP tools answer symbol, reference, impact, and architecture questions in one call where grep and Read take many; project_status reports whether the index is current."

# Run the digest, capturing stdout. A missing binary or a non-zero exit leaves
# DIGEST empty, which falls back below.
DIGEST=""
if [ -x "$BIN" ]; then
  DIGEST=$("$BIN" digest "$DIR" 2>/dev/null) || DIGEST=""
fi
[ -n "$DIGEST" ] || DIGEST="$FALLBACK"

# Build the JSON with jq so quotes, backslashes, and newlines in the digest are
# escaped correctly; hand-interpolating the payload into a literal {...} is the
# parse-failure bug class [src: plan.md D3a]. Without jq the hook is a silent
# no-op (exit 0) rather than a malformed-JSON `hook error`.
command -v jq >/dev/null 2>&1 || exit 0
jq -n --arg ctx "$DIGEST" \
  '{hookSpecificOutput:{hookEventName:"SessionStart",additionalContext:$ctx}}'
exit 0
"#;

/// Scaffold config, merge `.mcp.json`, refresh the `CLAUDE.md` block, report.
///
/// # Errors
/// Propagates `init` failures and `.mcp.json` / `CLAUDE.md` IO or parse
/// failures.
pub fn run(root: &Path) -> Result<()> {
    // Step A — config scaffolding (idempotent) [src: init.rs:14-38].
    crate::commands::init::run(root)?;

    // Step B — register the `ariadne` MCP server.
    merge_mcp_json(root)?;

    // Step C — refresh the CLAUDE.md discoverability block.
    write_claude_block(root)?;

    // Step D — install the SessionStart hook and register it in settings.json.
    install_session_start_hook(root)?;

    // Step E — report and point at the next step.
    println!("  .mcp.json        registered the `ariadne` MCP server");
    println!("  CLAUDE.md        wrote the Ariadne discoverability block");
    println!("  settings.json    registered the SessionStart digest hook");
    println!("next: run `ariadne index` to build the index");
    Ok(())
}

/// Insert (or replace) the `ariadne` key in `<root>/.mcp.json`, leaving any
/// foreign `mcpServers` entry untouched. Creates the file when absent.
fn merge_mcp_json(root: &Path) -> Result<()> {
    let path = root.join(".mcp.json");
    let mut config: Value = match std::fs::read_to_string(&path) {
        Ok(text) => {
            serde_json::from_str(&text).with_context(|| format!("parse {}", path.display()))?
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => json!({}),
        Err(e) => return Err(e).with_context(|| format!("read {}", path.display())),
    };

    // `command` is the absolute path of the running binary, not the bare
    // string `"ariadne"` — robust when `ariadne` is not on `PATH`.
    let exe = std::env::current_exe().context("resolve the running `ariadne` binary")?;
    let exe = std::path::absolute(&exe).context("make the binary path absolute")?;
    // `alwaysLoad` exempts the `ariadne` server from MCP Tool Search deferral
    // so all tool descriptions load every session regardless of
    // `ENABLE_TOOL_SEARCH`; without it only tool names load at decision time
    // and the trigger-phrase descriptions never reach the agent
    // [src: https://code.claude.com/docs/en/mcp "Exempt a server from
    // deferral"; .claude/plans/ariadne-mcp-adoption/plan.md D1].
    let entry = json!({
        "command": exe.to_string_lossy(),
        "args": ["serve", "--watch"],
        "env": {},
        "alwaysLoad": true,
    });

    let servers = config
        .as_object_mut()
        .context("`.mcp.json` root must be a JSON object")?
        .entry("mcpServers")
        .or_insert_with(|| json!({}));
    servers
        .as_object_mut()
        .context("`.mcp.json` `mcpServers` must be a JSON object")?
        .insert("ariadne".to_owned(), entry);

    let mut rendered = serde_json::to_string_pretty(&config).context("serialize .mcp.json")?;
    rendered.push('\n');
    std::fs::write(&path, rendered).with_context(|| format!("write {}", path.display()))?;
    Ok(())
}

/// Write the marker-delimited Ariadne block into `<root>/CLAUDE.md`. When the
/// markers already exist the span between them (inclusive) is replaced in
/// place; otherwise the block is appended. Every byte outside the markers is
/// preserved verbatim, so a re-run is idempotent.
fn write_claude_block(root: &Path) -> Result<()> {
    let path = root.join("CLAUDE.md");
    // Mirror `merge_mcp_json`: only a missing file is "empty"; any other read
    // error propagates rather than being clobbered by the write below.
    let existing = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(e).with_context(|| format!("read {}", path.display())),
    };
    let block = render_block();

    let updated = match (existing.find(BEGIN_MARKER), existing.find(END_MARKER)) {
        (Some(begin), Some(end)) if end > begin => {
            let end = end + END_MARKER.len();
            format!("{}{block}{}", &existing[..begin], &existing[end..])
        }
        _ => {
            let mut out = existing;
            if !out.is_empty() {
                if !out.ends_with('\n') {
                    out.push('\n');
                }
                out.push('\n');
            }
            out.push_str(&block);
            out.push('\n');
            out
        }
    };
    std::fs::write(&path, updated).with_context(|| format!("write {}", path.display()))?;
    Ok(())
}

/// Render the Ariadne `CLAUDE.md` block, markers included, no trailing
/// newline. Wording tracks the tier-15 server-instructions rewrite.
fn render_block() -> String {
    [
        BEGIN_MARKER,
        "## Ariadne code intelligence",
        "",
        "The Ariadne MCP server is configured for this project (`.mcp.json`). It exposes",
        "a read-only semantic graph — symbols, references, and dependency edges — kept",
        "current with the code.",
        "",
        "Prefer the Ariadne MCP tools over `grep` / `Read` for any question about",
        "symbols, references, impact, or architecture: the graph answers in one call",
        "where text search needs many and misses cross-file edges.",
        "",
        "- Navigate — `list_symbols`, `find_definition`, `find_references`. Use when",
        "  locating a symbol or its call sites (\"where is `X` defined?\").",
        "- Impact — `blast_radius`, `plan_assist`, `diff_blast_radius`. Use when scoping a",
        "  change (\"what breaks if I change `X`?\", \"what does my current diff affect?\").",
        "- Architecture — `coupling_report`, `weak_spots`, `refactor_suggestions`. Use",
        "  when assessing structural health (\"what are the worst modules?\").",
        "- History analytics — `hotspots`, `complexity`, `co_change`. Use when triaging",
        "  risk from Git churn × complexity (\"what's the riskiest code?\", \"what changes",
        "  together?\").",
        "- Docs — `doc_for`, `doc_for_module`, `doc_for_project`. Use when summarizing a",
        "  symbol, file, or the whole project (\"document the `X` module\").",
        "- Freshness — `project_status`. Use to confirm the index is current (\"is the",
        "  index up to date?\").",
        END_MARKER,
    ]
    .join("\n")
}

/// Install the `SessionStart` hook: write the executable script, then register
/// it in `<root>/.claude/settings.json`. Both steps are idempotent.
fn install_session_start_hook(root: &Path) -> Result<()> {
    write_hook_script(root)?;
    merge_settings_json(root)?;
    Ok(())
}

/// Write `<root>/.claude/hooks/ariadne-session-start.sh` from the embedded
/// template, substituting the absolute `ariadne` binary path, and mark it
/// executable. Overwrites any prior copy so re-running `setup` refreshes the
/// resolved binary path.
fn write_hook_script(root: &Path) -> Result<()> {
    let path = root.join(SESSION_START_SCRIPT);
    let dir = path.parent().expect("hook path has a parent");
    std::fs::create_dir_all(dir).with_context(|| format!("create {}", dir.display()))?;

    // Absolute path of the running binary, not the bare string `"ariadne"` —
    // robust when `ariadne` is not on `PATH` (mirrors `merge_mcp_json`).
    let exe = std::env::current_exe().context("resolve the running `ariadne` binary")?;
    let exe = std::path::absolute(&exe).context("make the binary path absolute")?;
    // POSIX single-quote escaping: a `'` in the path becomes `'\''` so the
    // `BIN='…'` assignment stays well-formed for any install location.
    let bin = exe.to_string_lossy().replace('\'', "'\\''");
    let script = SESSION_START_HOOK.replace("__ARIADNE_BIN__", &bin);

    std::fs::write(&path, script).with_context(|| format!("write {}", path.display()))?;
    set_executable(&path)?;
    Ok(())
}

/// Mark `path` executable (`chmod +x`) on Unix; a no-op on other platforms.
#[cfg(unix)]
fn set_executable(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut perms = std::fs::metadata(path)
        .with_context(|| format!("stat {}", path.display()))?
        .permissions();
    perms.set_mode(perms.mode() | 0o755);
    std::fs::set_permissions(path, perms).with_context(|| format!("chmod {}", path.display()))
}

/// Mark `path` executable — no-op on non-Unix targets (no POSIX mode bits).
#[cfg(not(unix))]
fn set_executable(_path: &Path) -> Result<()> {
    Ok(())
}

/// Deep-merge the `SessionStart` hook entry into `<root>/.claude/settings.json`,
/// touching only `hooks.SessionStart`: every other event (notably the
/// `PreToolUse` audit-gate) is preserved semantically — `serde_json` normalizes
/// object key order on re-serialization (`Value` is a `BTreeMap`), so a
/// hand-authored entry's keys may be re-sorted, but its keys and values survive
/// intact. Idempotent — a prior Ariadne entry is dropped and re-appended, and
/// because the first run normalizes the file a second run yields byte-identical
/// output; foreign `SessionStart` entries are left in place
/// [src: plan.md D3d; setup.rs `merge_mcp_json` object-entry pattern].
fn merge_settings_json(root: &Path) -> Result<()> {
    let path = root.join(".claude/settings.json");
    let mut config: Value = match std::fs::read_to_string(&path) {
        Ok(text) => {
            serde_json::from_str(&text).with_context(|| format!("parse {}", path.display()))?
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => json!({}),
        Err(e) => return Err(e).with_context(|| format!("read {}", path.display())),
    };

    let hooks = config
        .as_object_mut()
        .context("`.claude/settings.json` root must be a JSON object")?
        .entry("hooks")
        .or_insert_with(|| json!({}));
    let session_start = hooks
        .as_object_mut()
        .context("`.claude/settings.json` `hooks` must be a JSON object")?
        .entry("SessionStart")
        .or_insert_with(|| json!([]));
    let entries = session_start
        .as_array_mut()
        .context("`hooks.SessionStart` must be a JSON array")?;

    // Drop any prior Ariadne entry, then append a fresh one. This replaces our
    // own entry in place (idempotent) while leaving foreign entries untouched.
    entries.retain(|entry| !is_ariadne_session_start(entry));
    entries.push(json!({
        "hooks": [
            { "type": "command", "command": SESSION_START_COMMAND }
        ]
    }));

    let dir = path.parent().expect("settings path has a parent");
    std::fs::create_dir_all(dir).with_context(|| format!("create {}", dir.display()))?;
    let mut rendered =
        serde_json::to_string_pretty(&config).context("serialize .claude/settings.json")?;
    rendered.push('\n');
    std::fs::write(&path, rendered).with_context(|| format!("write {}", path.display()))?;
    Ok(())
}

/// Whether a `SessionStart` array entry is the Ariadne hook — identified by a
/// `command` matching [`SESSION_START_COMMAND`] in its `hooks` list.
fn is_ariadne_session_start(entry: &Value) -> bool {
    entry
        .get("hooks")
        .and_then(Value::as_array)
        .is_some_and(|hooks| {
            hooks
                .iter()
                .any(|h| h.get("command").and_then(Value::as_str) == Some(SESSION_START_COMMAND))
        })
}
