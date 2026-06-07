//! `ariadne fitness check` — the architecture-fitness gate (block A, A3).
//!
//! A3 has no warm daemon leg (deferred — tier-04 step 5): the command builds
//! the same cold [`Catalog`] the MCP server uses and calls the same
//! `ariadne_mcp::tools::fitness_report::handle` the MCP `fitness_report` tool
//! calls — read `ariadne-fitness.toml` → resolve layer globs → run the pure
//! engine — so the CLI and MCP verdicts are parity by construction (mirroring
//! `api-diff` / ADR-0027). Prints the violations as JSON and reports
//! `success = false` (→ non-zero process exit) when any violation exists, so CI
//! can gate on it [src:
//! .claude/plans/intelligence-platform/block-a/tier-04-fitness.md step 4].

use std::path::Path;

use anyhow::{Context, Result, bail};
use ariadne_mcp::Catalog;
use ariadne_mcp::tools;
use ariadne_storage::RedbStorage;

use crate::domain::index_path;

/// Run `fitness check`: build the cold catalog, run the fitness engine against
/// `ariadne-fitness.toml`, print the JSON report, and return whether the
/// architecture passed (`false` makes `main` exit non-zero — the CI gate).
///
/// # Errors
/// Fails when the index is missing, the catalog cannot be built, or the rules
/// file is missing / malformed.
pub fn run(root: &Path) -> Result<bool> {
    let db_path = index_path(root);
    if !db_path.exists() {
        bail!(
            "no index at {} — run `ariadne index` first",
            db_path.display()
        );
    }
    let storage = RedbStorage::open(&db_path).context("open redb index")?;
    let catalog = Catalog::build(&storage, root.display().to_string()).context("build catalog")?;
    let out = tools::fitness_report::handle(&catalog, root).context("run fitness check")?;
    println!(
        "{}",
        serde_json::to_string_pretty(&out).context("serialize fitness output")?
    );
    Ok(out.ok)
}
