//! `ariadne doc` — write the project architecture overview as Markdown plus a
//! sidecar SVG.
//!
//! The read-only MCP `doc_for_project` tool returns Markdown text and does no
//! IO; this driving adapter is the one layer that touches the filesystem. It
//! builds the same cold [`Catalog`] the `query` command uses, renders the
//! overview Markdown via `docgen::for_project` and the crate-level diagram via
//! `docgen::architecture_svg`, then writes both files with `std::fs::write`
//! [src: plan.md D4; crates/ariadne-cli/src/commands/query.rs cold path;
//! crates/ariadne-mcp/src/tools/doc_project.rs].

use std::path::Path;

use anyhow::{Context, Result, bail};
use ariadne_core::Storage;
use ariadne_graph::DocScope;
use ariadne_mcp::Catalog;
use ariadne_mcp::tools::coupling_report::build_modules;
use ariadne_storage::RedbStorage;

use crate::domain::index_path;

/// The sidecar image link the project Markdown emits by default. Rewritten to
/// the chosen `--svg` basename so the committed `.md`+`.svg` pair renders
/// in-place regardless of the output path [src:
/// crates/ariadne-graph/src/docgen_insights.rs `architecture_section`].
const DEFAULT_SVG_LINK: &str = "![architecture](codebase-overview.svg)";

/// Render the project overview from the cold index at `root` and write the
/// Markdown to `out` and the architecture SVG to `svg`. Relative `out`/`svg`
/// resolve against `root`; absolute paths are used as given. `excludes` are
/// extra substring excludes layered atop the default `Source`-only scope.
///
/// # Errors
/// Fails when the index is missing, the catalog or snapshot cannot be built,
/// the overview cannot be rendered, or either file cannot be written.
pub fn run(root: &Path, out: &Path, svg: &Path, excludes: &[String]) -> Result<()> {
    let db_path = index_path(root);
    if !db_path.exists() {
        bail!(
            "no index at {} — run `ariadne index` first",
            db_path.display()
        );
    }
    let storage = RedbStorage::open(&db_path).context("open redb index")?;
    let catalog = Catalog::build(&storage, root.display().to_string()).context("build catalog")?;
    let snap = storage.snapshot().context("snapshot index")?;

    let scope = DocScope {
        extra_excludes: excludes.to_vec(),
    };
    let modules = build_modules(&catalog, None);

    let svg_bytes = ariadne_graph::docgen::architecture_svg(&catalog.graph, &modules, &scope);
    let markdown = ariadne_graph::docgen::for_project(
        &catalog.graph,
        &snap,
        &modules,
        &catalog.churn,
        &catalog.co_change,
        &scope,
    )
    .context("render project overview")?;

    let out_path = root.join(out);
    let svg_path = root.join(svg);

    // Rewrite the hard-coded sidecar link to the chosen SVG's basename so the
    // Markdown points at the file actually written beside it.
    let svg_name = svg_path
        .file_name()
        .and_then(|n| n.to_str())
        .context("svg output path has no file name")?;
    let markdown = markdown.replace(DEFAULT_SVG_LINK, &format!("![architecture]({svg_name})"));

    // Write the SVG first, then the Markdown that references it, so the
    // committed pair is never half-written into an inconsistent state.
    write_file(&svg_path, svg_bytes.as_bytes())?;
    write_file(&out_path, markdown.as_bytes())?;

    println!("wrote {} and {}", out_path.display(), svg_path.display());
    Ok(())
}

/// Create the parent directory (if any) and write `bytes` to `path`.
fn write_file(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("create {}", parent.display()))?;
        }
    }
    std::fs::write(path, bytes).with_context(|| format!("write {}", path.display()))
}
