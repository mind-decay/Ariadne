//! `api_surface_diff` in-process path — the public-surface semver verdict
//! between two refs (block A, A2).
//!
//! Unlike the warm/cold tools, A2 has no daemon leg (D6 / ADR-0027): the whole
//! composition runs in the querying process where `ariadne-git` and
//! `ariadne-parser` are linked. [`handle`] resolves the diff between the refs
//! (bounding the read to changed files, D4), re-extracts each changed source
//! file's public surface at each ref through the SAME tree-sitter path, and
//! classifies the delta with the pure `ariadne_graph::api_surface_diff`. Both
//! the MCP `#[tool]` and the CLI `api-diff` command call this one function, so
//! their output is parity by construction [src:
//! .claude/plans/intelligence-platform/block-a/plan.md D3/D4/D6;
//! docs/adr/0027-mcp-parser-dependency.md].

use std::path::Path;

use ariadne_core::{DiffSpec, Lang, PublicSymbol};
use ariadne_graph::{ApiDiffReport, SemverBump, api_surface_diff};
use ariadne_parser::public_surface;

use crate::errors::McpError;
use crate::types::{ApiChangeRow, ApiSurfaceDiffOutput, ApiSymbolRow, SemverBumpWire};

/// Classify the public-surface delta between `base` and `head` revspecs in the
/// repository at `root`.
///
/// Diffs `base`→`head` to bound the surface read to the changed files (only a
/// changed file can change the public surface, D4), re-parses each changed
/// source file's blob at each ref into its public surface, and runs the pure
/// classifier. Returns the wire output both surfaces serialize, so the MCP and
/// CLI answers are identical.
///
/// # Errors
/// [`McpError`] when the git diff, a base/head blob read, or a public-surface
/// re-parse fails.
pub fn handle(root: &Path, base: &str, head: &str) -> Result<ApiSurfaceDiffOutput, McpError> {
    // The diff's changed-path list bounds the surface read to files that could
    // have changed the public surface (D4); the line hunks are unused — A2
    // re-parses whole files, not line ranges.
    let spec = DiffSpec::RefRange {
        from: base.to_owned(),
        to: head.to_owned(),
    };
    let (_hunks, changed_paths) = ariadne_git::diff(root, &spec)
        .map_err(|e| McpError::Other(format!("git diff failed: {e}")))?;

    let base_surface = surface_at(root, base, &changed_paths)?;
    let head_surface = surface_at(root, head, &changed_paths)?;

    Ok(to_wire(api_surface_diff(&base_surface, &head_surface)))
}

/// Re-extract the concatenated public surface of every changed *source* file at
/// `rev`. A path absent at `rev` (added/removed across the diff) yields no blob
/// and so contributes nothing — its symbols then surface as added/removed
/// against the other ref. A changed non-source file (an extension outside the
/// indexer's table) is skipped.
fn surface_at(
    root: &Path,
    rev: &str,
    changed_paths: &[String],
) -> Result<Vec<PublicSymbol>, McpError> {
    let blobs = ariadne_git::read_blobs_at(root, rev, changed_paths)
        .map_err(|e| McpError::Other(format!("read blobs at {rev} failed: {e}")))?;

    let mut surface = Vec::new();
    for (path, bytes) in blobs {
        let Some(lang) = lang_of(&path) else {
            continue;
        };
        let file_surface = public_surface(lang, &bytes).map_err(|e| {
            McpError::Other(format!("public_surface for {path} at {rev} failed: {e}"))
        })?;
        surface.extend(file_surface);
    }
    Ok(surface)
}

/// Path extension → [`Lang`] via the indexer's canonical table; `None` for an
/// extension outside the indexed source languages (skipped) [src:
/// crates/ariadne-core/src/domain/types/lang.rs `Lang::from_extension`].
fn lang_of(path: &str) -> Option<Lang> {
    let ext = Path::new(path).extension()?.to_str()?;
    Lang::from_extension(ext)
}

/// Project the pure graph report onto the wire output both surfaces serialize.
fn to_wire(report: ApiDiffReport) -> ApiSurfaceDiffOutput {
    ApiSurfaceDiffOutput {
        verdict: match report.verdict {
            SemverBump::None => SemverBumpWire::None,
            SemverBump::Patch => SemverBumpWire::Patch,
            SemverBump::Minor => SemverBumpWire::Minor,
            SemverBump::Major => SemverBumpWire::Major,
        },
        added: report.added.into_iter().map(symbol_row).collect(),
        removed: report.removed.into_iter().map(symbol_row).collect(),
        changed: report
            .changed
            .into_iter()
            .map(|c| ApiChangeRow {
                name: c.name,
                kind: c.kind,
                base_signature: c.base_signature,
                head_signature: c.head_signature,
            })
            .collect(),
    }
}

/// Project one [`PublicSymbol`] onto an [`ApiSymbolRow`] (visibility dropped —
/// every surface symbol is public by construction).
fn symbol_row(s: PublicSymbol) -> ApiSymbolRow {
    ApiSymbolRow {
        name: s.name,
        kind: s.kind,
        signature: s.signature,
    }
}
