//! `ariadne status` — print index counts + indexer availability matrix.

use std::path::Path;

use anyhow::{Context, Result};
use ariadne_mcp::Catalog;
use ariadne_storage::RedbStorage;

use crate::config::{Config, INDEXER_BINARIES, resolve_on_path};
use crate::domain::index_path;

/// Report revision, file/symbol/edge counts, configured languages, and the
/// per-language indexer availability matrix [src: tier-10 step 8].
///
/// # Errors
/// Propagates storage-open and catalog-build failures.
pub fn run(root: &Path) -> Result<()> {
    let db_path = index_path(root);
    if !db_path.exists() {
        println!("no index at {} — run `ariadne index`", db_path.display());
        print_indexer_matrix();
        return Ok(());
    }

    let storage = RedbStorage::open(&db_path).context("open redb index")?;
    let catalog =
        Catalog::build(&storage, root.display().to_string()).context("build catalog from index")?;

    println!("index:    {}", db_path.display());
    println!("revision: {}", catalog.revision);
    println!("files:    {}", catalog.paths.len());
    println!("symbols:  {}", catalog.symbols.len());
    println!("edges:    {}", catalog.graph.edge_count());

    match Config::load(root) {
        Ok(config) => println!("langs:    {}", config.enabled_langs.join(", ")),
        Err(_) => println!("langs:    (config unreadable)"),
    }
    print_indexer_matrix();
    Ok(())
}

/// Print the SCIP posture: default-on + out-of-band, then one row per indexer
/// (language, binary, resolved path or `MISSING`), and a degraded-mode summary.
/// SCIP runs by default after the fast index commits; any missing indexer
/// degrades its languages to the precise tree-sitter resolver — a warning, never
/// a failure (plan D6, R1) [src: docs/adr/0026-default-on-out-of-band-scip.md].
fn print_indexer_matrix() {
    println!("scip:     default-on, out-of-band (pass `--no-scip` to disable)");
    println!("indexers:");
    let mut missing = Vec::new();
    for (lang, binary) in INDEXER_BINARIES {
        let location = if let Some(p) = resolve_on_path(binary) {
            p.display().to_string()
        } else {
            missing.push(*lang);
            "MISSING".to_owned()
        };
        println!("  {lang:<10} {binary:<16} {location}");
    }
    if missing.is_empty() {
        println!("  all indexers present — precise SCIP edges available for every language");
    } else {
        println!(
            "  degraded: {} missing — those languages index on the precise resolver (never a failure)",
            missing.join(", "),
        );
    }
}
