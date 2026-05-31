//! `ariadne index` — run the cold-index pipeline and commit to redb.

use std::path::Path;

use anyhow::{Context, Result};
use ariadne_core::Storage;
use ariadne_git::{HistoryOptions, walk_history};
use ariadne_storage::RedbStorage;

use crate::config::Config;
use crate::domain::{index_path, run_index};

/// Load the project config, run the full cold pipeline, print the per-phase
/// timing breakdown + parse sub-phase breakdown on stderr, and the JSON-line
/// summary on stdout. `scip` gates the external SCIP indexers
/// [src: tier-12 steps 1-2; tier-13 step 1].
///
/// # Errors
/// Propagates config-load, walk, parse, and storage failures.
pub fn run(root: &Path, fresh: bool, scip: bool) -> Result<()> {
    let config = Config::load(root)?;
    let (summary, phases, parse_sub) = run_index(root, &config, fresh, scip)?;
    eprintln!(
        "[index] phases (ms): walk={} parse={} resolve={} commit={} scip={}",
        phases.walk, phases.parse, phases.resolve, phases.commit, phases.scip,
    );
    eprintln!(
        "[index] parse (ms, summed over workers): read={} parse={} extract={}",
        parse_sub.read, parse_sub.parse, parse_sub.extract,
    );
    ingest_history(root, &config)?;
    println!("{}", serde_json::to_string(&summary)?);
    Ok(())
}

/// Walk Git history (after the symbol commit) and persist file churn +
/// co-change. Wired here at the composition root so the daemon never depends
/// on `ariadne-git` (RD7). A non-Git project is skipped — there is no history
/// to ingest, not a failure; genuine traversal errors propagate.
///
/// # Errors
/// Propagates Git-walk and storage failures.
fn ingest_history(root: &Path, config: &Config) -> Result<()> {
    if !root.join(".git").exists() {
        return Ok(());
    }
    let opts = HistoryOptions {
        depth: config.history.depth,
        max_files_per_commit: config.history.max_files_per_commit,
    };
    let report = walk_history(root, &opts).context("walk git history")?;
    let storage = RedbStorage::open(&index_path(root)).context("open redb index for history")?;
    storage
        .replace_history(&report.churn, &report.pairs)
        .context("persist git history")?;
    eprintln!(
        "[index] history: {} files, {} co-change pairs",
        report.churn.len(),
        report.pairs.len(),
    );
    Ok(())
}
