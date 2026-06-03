//! `read_symbol` — return a symbol's source straight from disk.
//!
//! Resolves `symbol` (disambiguated by `file` when several share the name) to
//! its defining span via the in-RAM [`Catalog`], then delegates the live file
//! read to [`crate::adapters::source::read_span`] — the handler itself does
//! no `std::fs`, keeping IO under `src/adapters/` (tier-08 D9). The catalog
//! `revision` and resolved `name` are attached to the returned slice so the
//! caller can judge freshness.

use std::path::Path;

use crate::adapters::source::{self, SourceMode};
use crate::catalog::Catalog;
use crate::errors::McpError;
use crate::types::{ReadSymbolInput, SourceSlice};

/// Lines of surrounding context for `context` mode when the caller omits
/// `context_lines`.
const DEFAULT_CONTEXT_LINES: u32 = 3;

/// Resolve `input.symbol` to a defining span and read its source.
///
/// # Errors
/// Returns [`McpError::NotFound`] when the symbol (or the requested `file`
/// disambiguation) resolves to nothing, or its file cannot be read;
/// [`McpError::InvalidInput`] when `mode` is not `signature | full | context`.
pub fn handle(cat: &Catalog, input: &ReadSymbolInput) -> Result<SourceSlice, McpError> {
    let ids = cat
        .by_name
        .get(&input.symbol)
        .filter(|v| !v.is_empty())
        .ok_or_else(|| McpError::NotFound(format!("symbol `{}`", input.symbol)))?;

    // Disambiguate by defining-file path when `file` is supplied; otherwise
    // take the first match. With no `file` and several matches, collect the
    // other candidates' defining paths so the caller learns overloads existed
    // and can re-query with `file` to pin one (tier-08 step 4 "+ note").
    let (id, alternatives) = if let Some(want) = input.file.as_deref() {
        let id = ids
            .iter()
            .copied()
            .find(|id| cat.meta_of(*id).and_then(|m| cat.path_of(m.file)) == Some(want))
            .ok_or_else(|| McpError::NotFound(format!("symbol `{}` in `{want}`", input.symbol)))?;
        (id, Vec::new())
    } else {
        let mut others: Vec<String> = ids
            .iter()
            .skip(1)
            .filter_map(|id| cat.meta_of(*id).and_then(|m| cat.path_of(m.file)))
            .map(str::to_owned)
            .collect();
        others.sort();
        others.dedup();
        (ids[0], others)
    };

    let meta = cat
        .meta_of(id)
        .ok_or_else(|| McpError::NotFound(format!("symbol id {}", id.get())))?;
    let path = cat
        .path_of(meta.file)
        .ok_or_else(|| McpError::NotFound(format!("file for symbol `{}`", input.symbol)))?;

    let mode = SourceMode::parse(input.mode.as_deref())?;
    let ctx = input.context_lines.unwrap_or(DEFAULT_CONTEXT_LINES);

    let mut slice = source::read_span(
        Path::new(&cat.root),
        path,
        meta.byte_start,
        meta.byte_end,
        mode,
        ctx,
    )?;
    slice.name.clone_from(&meta.name);
    slice.revision = cat.revision;
    slice.alternatives = alternatives;
    Ok(slice)
}
