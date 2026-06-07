//! `ariadne outline <path>` — print a token-cheap folded code skeleton of a
//! whole file.
//!
//! Builds the cold in-process [`Catalog`] the MCP server uses (the `query`
//! plumbing), enumerates the target file's symbols (the `file_summary` pattern —
//! iterate `cat.symbols`, keep `meta.file == file_id`, sort by `byte_start`),
//! reads the live bytes via `std::fs`, and hands them to the pure
//! [`ariadne_graph::assemble`] use case shared with the `read_outline` MCP tool
//! — so the two surfaces render the identical skeleton (parity) [src:
//! .claude/plans/context-efficient-read/tier-03-outline-cli.md `<steps>`;
//! plan.md D3/D7]. A file with no indexed symbols never dumps its source: it
//! prints a line-count note advising a native `Read` (D2).

use std::path::Path;

use anyhow::{Context, Result, bail};
use ariadne_core::{FileId, Lang};
use ariadne_graph::{Outline, OutlineOptions, OutlineRequest, OutlineSymbol, assemble};
use ariadne_mcp::Catalog;
use ariadne_storage::RedbStorage;
use serde::Serialize;

use crate::domain::index_path;

/// Cap on rendered top-level symbols, matching the `read_outline` MCP tool so
/// the CLI and MCP skeletons stay byte-identical for the same file and options
/// (parity); the assembler notes the cap in the skeleton tail rather than
/// truncating silently [src: crates/ariadne-mcp/src/tools/read_outline.rs:28].
const MAX_OUTLINE_SYMBOLS: usize = 800;

/// Render the folded skeleton of `path` to stdout (or JSON with `json`). The
/// index is resolved against the current directory.
///
/// # Errors
/// Fails when no index exists, the catalog cannot be built, the file is not
/// indexed, or its bytes cannot be read.
pub fn run(path: &Path, include_private: bool, json: bool) -> Result<()> {
    let root = Path::new(".");
    let db_path = index_path(root);
    if !db_path.exists() {
        bail!(
            "no index at {} — run `ariadne index` first",
            db_path.display()
        );
    }
    let storage = RedbStorage::open(&db_path).context("open redb index")?;
    let catalog = Catalog::build(&storage, root.display().to_string()).context("build catalog")?;

    let (key, file_id) = resolve_file(&catalog, root, path)
        .with_context(|| format!("`{}` is not an indexed file", path.display()))?;

    // Enumerate this file's symbols (file_summary pattern), capturing the
    // defining file's language from any of them (all share it).
    let mut symbols: Vec<OutlineSymbol> = Vec::new();
    let mut lang: Option<Lang> = None;
    for meta in catalog.symbols.values() {
        if meta.file != file_id {
            continue;
        }
        lang.get_or_insert(meta.lang);
        symbols.push(OutlineSymbol {
            name: meta.name.clone(),
            kind: meta.kind.clone(),
            byte_start: meta.byte_start,
            byte_end: meta.byte_end,
            visibility: meta.visibility,
        });
    }
    symbols.sort_by_key(|s| (s.byte_start, s.byte_end));

    let bytes = std::fs::read(root.join(&key)).with_context(|| format!("read `{key}`"))?;

    // Zero indexed symbols: never dump the file — print a line-count note
    // pointing the caller at a native `Read` (parity with read_outline D2).
    if symbols.is_empty() {
        let lines = String::from_utf8_lossy(&bytes).lines().count();
        println!(
            "`{key}` has {lines} lines and no indexed symbols; read it with the native `Read` tool."
        );
        return Ok(());
    }

    let req = OutlineRequest {
        source: bytes,
        symbols,
        lang: lang.unwrap_or(Lang::Other("unknown")),
        options: OutlineOptions {
            include_private,
            max_symbols: MAX_OUTLINE_SYMBOLS,
        },
    };
    let outline = assemble(&req);

    if json {
        println!("{}", to_json(&outline)?);
    } else {
        println!("{}", outline.skeleton);
    }
    Ok(())
}

/// Resolve `path` to its catalog key + [`FileId`]. The catalog keys files by
/// their project-root-relative path, so this tries the path as given, stripped
/// of the `root` prefix, then canonicalized-relative-to-`root` — returning the
/// first form indexed in the catalog.
fn resolve_file(cat: &Catalog, root: &Path, path: &Path) -> Option<(String, FileId)> {
    let normalize = |p: &Path| p.to_string_lossy().replace('\\', "/");
    let mut candidates = vec![normalize(path)];
    if let Ok(rel) = path.strip_prefix(root) {
        candidates.push(normalize(rel));
    }
    if let (Ok(abs), Ok(abs_root)) = (path.canonicalize(), root.canonicalize()) {
        if let Ok(rel) = abs.strip_prefix(&abs_root) {
            candidates.push(normalize(rel));
        }
    }
    candidates
        .into_iter()
        .find_map(|k| cat.path_to_id.get(&k).map(|id| (k, *id)))
}

/// Serialize the graph [`Outline`] (skeleton + symbol index + line counts) as
/// pretty JSON. The domain type carries no serde derive (tier-01 keeps it
/// dependency-free), so the CLI projects it into a local serializable shape
/// rather than coupling the domain to serde.
fn to_json(outline: &Outline) -> Result<String> {
    let value = JsonOutline {
        skeleton: &outline.skeleton,
        symbols: outline
            .symbols
            .iter()
            .map(|e| JsonEntry {
                name: &e.name,
                kind: &e.kind,
                line_start: e.line_start,
                line_end: e.line_end,
                body_lines: e.body_lines,
                has_body: e.has_body,
            })
            .collect(),
        kept_lines: outline.kept_lines,
        elided_lines: outline.elided_lines,
    };
    serde_json::to_string_pretty(&value).context("serialize outline JSON")
}

/// Serializable mirror of [`ariadne_graph::Outline`] for `--json` output.
#[derive(Serialize)]
struct JsonOutline<'a> {
    skeleton: &'a str,
    symbols: Vec<JsonEntry<'a>>,
    kept_lines: u32,
    elided_lines: u32,
}

/// Serializable mirror of [`ariadne_graph::OutlineEntry`].
#[derive(Serialize)]
struct JsonEntry<'a> {
    name: &'a str,
    kind: &'a str,
    line_start: u32,
    line_end: u32,
    body_lines: u32,
    has_body: bool,
}
