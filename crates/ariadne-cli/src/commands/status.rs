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

/// Print one row per SCIP indexer: language, binary, resolved path or
/// `MISSING`.
fn print_indexer_matrix() {
    println!("indexers:");
    for (lang, binary) in INDEXER_BINARIES {
        let location = resolve_on_path(binary)
            .map_or_else(|| "MISSING".to_owned(), |p| p.display().to_string());
        println!("  {lang:<10} {binary:<16} {location}");
    }
}
