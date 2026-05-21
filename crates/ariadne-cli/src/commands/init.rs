//! `ariadne init` — scaffold `.ariadne/` and write a default config.

use std::path::Path;

use anyhow::{Context, Result};

use crate::config::{Config, INDEXER_BINARIES, resolve_on_path};

/// Detect languages, probe indexers on PATH, write `config.toml`, and add
/// `.ariadne/` to `.gitignore` (idempotent).
///
/// # Errors
/// Propagates repository-walk and filesystem failures.
pub fn run(root: &Path) -> Result<()> {
    let mut config = Config::detect(root);
    for (_lang, binary) in INDEXER_BINARIES {
        if let Some(path) = resolve_on_path(binary) {
            config
                .indexers
                .insert((*binary).to_owned(), path.display().to_string());
        }
    }
    config.write(root)?;
    ensure_gitignored(root)?;

    println!("initialized .ariadne/ in {}", root.display());
    if config.enabled_langs.is_empty() {
        println!("  enabled langs: (none detected)");
    } else {
        println!("  enabled langs: {}", config.enabled_langs.join(", "));
    }
    println!(
        "  indexers on PATH: {} of {}",
        config.indexers.len(),
        INDEXER_BINARIES.len()
    );
    Ok(())
}

/// Append `.ariadne/` to `<root>/.gitignore` unless it is already ignored.
fn ensure_gitignored(root: &Path) -> Result<()> {
    let path = root.join(".gitignore");
    let existing = std::fs::read_to_string(&path).unwrap_or_default();
    let already = existing
        .lines()
        .any(|line| matches!(line.trim(), ".ariadne" | ".ariadne/"));
    if already {
        return Ok(());
    }
    let mut updated = existing;
    if !updated.is_empty() && !updated.ends_with('\n') {
        updated.push('\n');
    }
    updated.push_str(".ariadne/\n");
    std::fs::write(&path, updated).with_context(|| format!("update {}", path.display()))?;
    Ok(())
}
