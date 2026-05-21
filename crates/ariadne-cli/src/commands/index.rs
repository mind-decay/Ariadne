//! `ariadne index` — run the cold-index pipeline and commit to redb.

use std::path::Path;

use anyhow::Result;

use crate::config::Config;
use crate::domain::run_index;

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
    println!("{}", serde_json::to_string(&summary)?);
    Ok(())
}
