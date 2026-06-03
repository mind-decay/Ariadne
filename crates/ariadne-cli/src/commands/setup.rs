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

/// Project-root-relative path of the installed `PreToolUse` advisor script.
const ADVISOR_SCRIPT: &str = ".claude/hooks/ariadne-grep-advisor.sh";

/// Command string registered in `.claude/settings.json` for the advisory
/// `PreToolUse` entry; `${CLAUDE_PROJECT_DIR}` is substituted by Claude Code to
/// the project root, mirroring the `SessionStart` entry [src: tier-03 D3d;
/// <https://code.claude.com/docs/en/hooks> placeholders].
const ADVISOR_COMMAND: &str = "${CLAUDE_PROJECT_DIR}/.claude/hooks/ariadne-grep-advisor.sh";

/// Matcher for the advisory entry — a pipe-alternation of exact tool names, so
/// it fires on `Grep`, `Glob`, and `Read` and nothing else; the existing `Bash`
/// audit-gate matcher is a separate array entry left untouched [src:
/// <https://code.claude.com/docs/en/hooks> matcher patterns; plan.md D5].
const ADVISOR_MATCHER: &str = "Grep|Glob|Read";

/// POSIX `sh` template for the `PreToolUse` advisor. Unlike the `SessionStart`
/// hook it shells out to nothing — a pure, dependency-light classifier (no
/// `jq`, no `ariadne`) that reads the `PreToolUse` payload on stdin, applies a
/// tight symbol-shaped heuristic to the search pattern, and either injects
/// advisory context (`permissionDecision:"allow"` + `additionalContext`) or
/// defers. Advisory by construction: it emits only `allow` or `defer`, NEVER
/// `deny`/`ask`, so it cannot block a legitimate search (D5). Any unexpected or
/// unparseable input defers (fail-open; precision over recall, R5). The injected
/// `additionalContext` is a fixed quote-free string, so it interpolates into the
/// JSON safely without `jq` [src: plan.md D5, R5;
/// <https://code.claude.com/docs/en/hooks> `PreToolUse` schema].
const ADVISOR_HOOK: &str = r#"#!/usr/bin/env sh
# Ariadne PreToolUse advisor — for a symbol-shaped Grep/Glob pattern, returns
# permissionDecision:"allow" plus additionalContext naming the Ariadne tool that
# answers it in one call; every other call defers untouched. Installed by
# `ariadne setup`; do not edit by hand. Advisory by construction: it emits only
# "allow" or "defer", NEVER "deny"/"ask", so it can never block a legitimate
# search (D5). Any unexpected input defers (fail-open; precision over recall, R5)
# [src: plan.md D5, R5; https://code.claude.com/docs/en/hooks PreToolUse].

set -u

# Defer: let the tool call through unchanged, with no added context. This is the
# only output besides a precise symbol-shaped match below.
defer() {
  printf '{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"defer"}}\n'
  exit 0
}

# Nudge: allow the call and inject $1 as advisory additionalContext, then exit.
# Each message is a fixed quote-free/backslash-free string, so it interpolates
# into the JSON safely without jq.
nudge() {
  printf '{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"allow","additionalContext":"%s"}}\n' "$1"
  exit 0
}

# stdin carries the PreToolUse payload {"tool_name":...,"tool_input":{...}}. An
# empty or unreadable payload defers.
PAYLOAD=$(cat 2>/dev/null) || defer
[ -n "$PAYLOAD" ] || defer

# Extract the tool name (no jq: a flat string field). Anything we cannot read
# cleanly leaves TOOL empty and defers below.
TOOL=$(printf '%s' "$PAYLOAD" | sed -n 's/.*"tool_name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p')

# Only Grep/Glob carry a search pattern worth classifying. Read takes a file
# path, never a symbol query (and a bare filename like `Makefile` would look
# identifier-shaped), so Read — and any other tool — defers.
case "$TOOL" in
  Grep|Glob) : ;;
  *) defer ;;
esac

# Extract the search pattern up to the first quote. A value containing an
# (escaped) quote — i.e. a quoted phrase — truncates to something the identifier
# test below rejects, so it defers. Intended.
QUERY=$(printf '%s' "$PAYLOAD" | sed -n 's/.*"pattern"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p')
[ -n "$QUERY" ] || defer

# Two factual messages, one per shape (audit F1): a definition-shaped query (a
# `::`-path or a CamelCase type) leads with find_definition; a snake_case query
# leads with find_references/list_symbols. Both name all three nav tools and stay
# quote- and backslash-free so they interpolate without jq [src: plan.md D5].
DEF_CTX="Ariadne's read-only semantic graph can resolve this symbol in one call: find_definition jumps straight to where it is defined, find_references then lists every call site across files, and list_symbols searches symbol names by substring or kind. The graph captures cross-file edges a text grep misses; consider the Ariadne MCP tools before scanning text."
REF_CTX="Ariadne's read-only semantic graph can resolve this symbol in one call: find_references lists every call site across files and list_symbols searches symbol names by substring or kind, while find_definition locates the definition. The graph captures cross-file edges a text grep misses; consider the Ariadne MCP tools before scanning text."

# Symbol-shaped heuristic with a structural floor (precision over recall, R5).
# The pattern must first be a bare identifier or a `::`-path; whitespace phrases,
# quoted strings, regex metacharacters, globs and file paths (with `/` or `.`)
# fail both and defer. A bare identifier then nudges ONLY if it carries a code
# signal — a `::` path, a `_` (snake_case), or a case mix (CamelCase). A bare
# all-lowercase or all-caps word with none of these (error, TODO, render) is
# free-text-shaped and defers: the residual false-positive class R5 trades away
# [src: audit F2].
if printf '%s' "$QUERY" | grep -Eq '^[A-Za-z_][A-Za-z0-9_]*(::[A-Za-z_][A-Za-z0-9_]*)+$'; then
  # Shape A — `::`-separated path (crate::mod::Type): definition-lead.
  nudge "$DEF_CTX"
elif printf '%s' "$QUERY" | grep -Eq '^[A-Za-z_][A-Za-z0-9_]*$'; then
  # Shapes B/C — a bare identifier. Apply the structural floor.
  if printf '%s' "$QUERY" | grep -q '_'; then
    # Shape C — snake_case (has `_`): references-lead.
    nudge "$REF_CTX"
  elif printf '%s' "$QUERY" | grep -Eq '[A-Z]' && printf '%s' "$QUERY" | grep -Eq '[a-z]'; then
    # Shape B — CamelCase / mixed case: definition-lead.
    nudge "$DEF_CTX"
  fi
  # else: bare all-lowercase or all-caps word, no code signal — fall through.
fi

defer
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

    // Step D — install the SessionStart digest hook + the PreToolUse advisor
    // script, then register both in `.claude/settings.json`.
    install_hooks(root)?;

    // Step E — report and point at the next step.
    println!("  .mcp.json        registered the `ariadne` MCP server");
    println!("  CLAUDE.md        wrote the Ariadne discoverability block");
    println!("  settings.json    registered the SessionStart digest + PreToolUse advisory hooks");
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

/// Install both hook scripts (the `SessionStart` digest hook and the
/// `PreToolUse` advisor), then register both in `<root>/.claude/settings.json`.
/// Every step is idempotent.
fn install_hooks(root: &Path) -> Result<()> {
    write_hook_script(root)?;
    write_advisor_script(root)?;
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

/// Write `<root>/.claude/hooks/ariadne-grep-advisor.sh` from the embedded
/// template and mark it executable. Unlike the `SessionStart` script the advisor
/// is a pure classifier with no binary path to resolve, so the template is
/// written verbatim — making a re-run byte-identical. Overwrites any prior copy.
fn write_advisor_script(root: &Path) -> Result<()> {
    let path = root.join(ADVISOR_SCRIPT);
    let dir = path.parent().expect("advisor path has a parent");
    std::fs::create_dir_all(dir).with_context(|| format!("create {}", dir.display()))?;
    std::fs::write(&path, ADVISOR_HOOK).with_context(|| format!("write {}", path.display()))?;
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

/// Deep-merge the Ariadne hook entries into `<root>/.claude/settings.json`,
/// touching only `hooks.SessionStart` (the digest hook) and
/// `hooks.PreToolUse` (the advisory entry, matcher `Grep|Glob|Read`). Foreign
/// entries — notably the Bash audit-gate under `PreToolUse` — are preserved
/// semantically: `serde_json` normalizes object key order on re-serialization
/// (`Value` is a `BTreeMap`), so a hand-authored entry's keys may be re-sorted,
/// but its keys and values survive intact. Idempotent — each prior Ariadne entry
/// is dropped and re-appended, and because the first run normalizes the file a
/// second run yields byte-identical output; foreign entries are left in place
/// [src: plan.md D3d, D5; setup.rs `merge_mcp_json` object-entry pattern].
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
    let hooks = hooks
        .as_object_mut()
        .context("`.claude/settings.json` `hooks` must be a JSON object")?;

    // SessionStart (tier-03): drop any prior Ariadne entry, then append a fresh
    // one. This replaces our own entry in place (idempotent) while leaving
    // foreign entries untouched.
    let session_start = hooks.entry("SessionStart").or_insert_with(|| json!([]));
    let entries = session_start
        .as_array_mut()
        .context("`hooks.SessionStart` must be a JSON array")?;
    entries.retain(|entry| !is_ariadne_session_start(entry));
    entries.push(json!({
        "hooks": [
            { "type": "command", "command": SESSION_START_COMMAND }
        ]
    }));

    // PreToolUse (tier-04): the advisory entry (matcher `Grep|Glob|Read`),
    // alongside any foreign matcher — notably the Bash audit-gate. Same in-place
    // replace, so the audit-gate entry survives the merge [src: plan.md D5;
    // .claude/settings.json].
    let pre_tool_use = hooks.entry("PreToolUse").or_insert_with(|| json!([]));
    let pre_entries = pre_tool_use
        .as_array_mut()
        .context("`hooks.PreToolUse` must be a JSON array")?;
    pre_entries.retain(|entry| !is_ariadne_advisory(entry));
    pre_entries.push(json!({
        "matcher": ADVISOR_MATCHER,
        "hooks": [
            { "type": "command", "command": ADVISOR_COMMAND }
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

/// Whether a `PreToolUse` array entry is the Ariadne advisory — identified by a
/// `command` matching [`ADVISOR_COMMAND`] in its `hooks` list (the matcher is
/// not consulted, so a re-run that changed the matcher still replaces in place
/// rather than duplicating). Foreign matchers (the Bash audit-gate) never match.
fn is_ariadne_advisory(entry: &Value) -> bool {
    entry
        .get("hooks")
        .and_then(Value::as_array)
        .is_some_and(|hooks| {
            hooks
                .iter()
                .any(|h| h.get("command").and_then(Value::as_str) == Some(ADVISOR_COMMAND))
        })
}
