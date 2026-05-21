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

    // Step D — report and point at the next step.
    println!("  .mcp.json        registered the `ariadne` MCP server");
    println!("  CLAUDE.md        wrote the Ariadne discoverability block");
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
    let entry = json!({
        "command": exe.to_string_lossy(),
        "args": ["serve", "--watch"],
        "env": {},
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
        "- Impact — `blast_radius`, `plan_assist`. Use when scoping a change (\"what",
        "  breaks if I change `X`?\").",
        "- Architecture — `coupling_report`, `weak_spots`, `refactor_suggestions`. Use",
        "  when assessing structural health (\"what are the worst modules?\").",
        "- Docs — `doc_for`, `doc_for_module`, `doc_for_project`. Use when summarizing a",
        "  symbol, file, or the whole project (\"document the `X` module\").",
        "- Freshness — `project_status`. Use to confirm the index is current (\"is the",
        "  index up to date?\").",
        END_MARKER,
    ]
    .join("\n")
}
