//! `read_outline` — project a whole file into a token-cheap folded code
//! skeleton.
//!
//! Resolves the target file via the in-RAM [`Catalog`], enumerates its symbols
//! (the `file_summary` pattern — iterate `cat.symbols`, keep
//! `meta.file == file_id`, sort by `byte_start`), reads the live bytes through
//! [`crate::adapters::source::read_file`], and hands them to the pure
//! [`ariadne_graph::assemble`] use case, which keeps signatures + leading doc
//! comments and folds bodies to a marker. The handler stays IO-light: the only
//! `std::fs` is the adapter read; the projection is pure (tier-01). A file with
//! no indexed symbols never dumps its source — it returns a line-count note
//! advising a native `Read` [src: context-efficient-read tier-02 D1/D2].

use std::path::Path;

use ariadne_core::Lang;
use ariadne_graph::{OutlineOptions, OutlineRequest, OutlineSymbol, assemble};

use crate::adapters::source;
use crate::catalog::Catalog;
use crate::errors::McpError;
use crate::types::{OutlineEntry, ReadOutlineInput, SourceOutline};

/// Safety cap on rendered top-level symbols, keeping the tool output under the
/// MCP 25k-token ceiling even for very large files; the assembler notes the cap
/// in the skeleton tail rather than truncating silently (R4) [src:
/// context-efficient-read plan.md `<constraints>` MCP limits; risks R4].
const MAX_OUTLINE_SYMBOLS: usize = 800;

/// Build the [`SourceOutline`] for the file at `input.path`.
///
/// # Errors
/// Returns [`McpError::NotFound`] when `input.path` is not indexed or its bytes
/// cannot be read. Out-of-range spans are clamped and flagged `stale`, never
/// errored (R5).
pub fn handle(cat: &Catalog, input: &ReadOutlineInput) -> Result<SourceOutline, McpError> {
    let file_id = *cat
        .path_to_id
        .get(&input.path)
        .ok_or_else(|| McpError::NotFound(format!("file {}", input.path)))?;

    // Enumerate this file's symbols (file_summary pattern), capturing the
    // defining file's language from any of them (all share it).
    let mut symbols: Vec<OutlineSymbol> = Vec::new();
    let mut lang: Option<Lang> = None;
    for meta in cat.symbols.values() {
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

    let bytes = source::read_file(Path::new(&cat.root), &input.path)?;

    // Zero indexed symbols: never dump the file — return a line-count note
    // pointing the caller at a native `Read` (D2; the advisory escalation).
    if symbols.is_empty() {
        let lines = String::from_utf8_lossy(&bytes).lines().count();
        return Ok(SourceOutline {
            path: input.path.clone(),
            revision: cat.revision,
            stale: false,
            skeleton: String::new(),
            symbols: Vec::new(),
            kept_lines: 0,
            elided_lines: 0,
            note: Some(format!(
                "`{}` has {lines} lines and no indexed symbols; read it with the native `Read` tool.",
                input.path
            )),
        });
    }

    // Stale when any recorded span runs past the live file length: the
    // assembler clamps line resolution to EOF, we only flag it (R5).
    let stale = symbols.iter().any(|s| s.byte_end as usize > bytes.len());

    let req = OutlineRequest {
        source: bytes,
        symbols,
        lang: lang.unwrap_or(Lang::Other("unknown")),
        options: OutlineOptions {
            include_private: input.include_private.unwrap_or(true),
            max_symbols: MAX_OUTLINE_SYMBOLS,
        },
    };
    let out = assemble(&req);

    Ok(SourceOutline {
        path: input.path.clone(),
        revision: cat.revision,
        stale,
        skeleton: out.skeleton,
        symbols: out
            .symbols
            .into_iter()
            .map(|e| OutlineEntry {
                name: e.name,
                kind: e.kind,
                line_start: e.line_start,
                line_end: e.line_end,
                body_lines: e.body_lines,
                has_body: e.has_body,
            })
            .collect(),
        kept_lines: out.kept_lines,
        elided_lines: out.elided_lines,
        note: None,
    })
}
