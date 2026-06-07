//! Source-retrieval driven adapter — the only filesystem IO in the MCP
//! crate. `read_symbol` resolves a symbol to its defining span and delegates
//! here to read the live file under the catalog root, returning just the
//! requested slice. Isolating `std::fs` behind this module keeps the
//! IO-under-`src/adapters/` convention (the tool handler stays pure)
//! [src: CLAUDE.md conventions; tier-08 D9].
//!
//! Spans can be stale after an edit: the recorded `end` may now run past the
//! current file length. This module never fails or fabricates bytes for that
//! — it clamps the span to the live length, flags `stale`, and serves only
//! the bytes that still exist (R7).

use std::path::Path;

use crate::errors::McpError;
use crate::types::SourceSlice;

/// How much of a symbol's source to return.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceMode {
    /// The declaration line(s) only — up to the body-opening `{` / `:`.
    Signature,
    /// The whole defining span `[start, end]` (the default).
    Full,
    /// The span widened by `ctx` lines on each side.
    Context,
}

impl SourceMode {
    /// Parse the wire `mode` string. `None` defaults to [`Self::Full`]; an
    /// unrecognised value is a typed caller-input error, never a panic.
    ///
    /// # Errors
    /// Returns [`McpError::InvalidInput`] for a mode outside
    /// `signature | full | context`.
    pub fn parse(mode: Option<&str>) -> Result<Self, McpError> {
        match mode.map(str::to_ascii_lowercase).as_deref() {
            None | Some("full") => Ok(Self::Full),
            Some("signature") => Ok(Self::Signature),
            Some("context") => Ok(Self::Context),
            Some(other) => Err(McpError::InvalidInput(format!(
                "mode `{other}`: expected signature | full | context"
            ))),
        }
    }
}

/// Read the slice of `root/rel_path` spanning the symbol's recorded
/// `[start, end]` byte range, shaped by `mode`. The returned [`SourceSlice`]
/// leaves `name` empty, `revision` zero, and `alternatives` empty — the
/// handler attaches those from the catalog (tier-08 step 4).
///
/// # Errors
/// Returns [`McpError::NotFound`] when the file cannot be read (missing or
/// unreadable). Out-of-range spans are clamped, not errored (R7).
// The six-parameter shape (root, rel_path, start, end, mode, ctx) is fixed by
// the tier-08 plan; grouping them into a struct would only add indirection.
#[allow(clippy::too_many_arguments)]
pub fn read_span(
    root: &Path,
    rel_path: &str,
    start: u32,
    end: u32,
    mode: SourceMode,
    ctx: u32,
) -> Result<SourceSlice, McpError> {
    let path = root.join(rel_path);
    let bytes = std::fs::read(&path)
        .map_err(|e| McpError::NotFound(format!("read {}: {e}", path.display())))?;
    let file_len = bytes.len();

    let req_start = start as usize;
    let req_end = end as usize;
    // Stale when the recorded span runs past the current file length: serve a
    // clamped slice and flag it rather than failing or fabricating bytes (R7).
    let stale = req_end > file_len;
    let clamped_start = req_start.min(file_len);
    let clamped_end = req_end.min(file_len).max(clamped_start);

    let (out_start, out_end) = match mode {
        SourceMode::Full => (clamped_start, clamped_end),
        SourceMode::Signature => (clamped_start, signature_end(&bytes, clamped_start)),
        SourceMode::Context => context_bounds(&bytes, clamped_start, clamped_end, ctx as usize),
    };

    let source = String::from_utf8_lossy(&bytes[out_start..out_end]).into_owned();
    Ok(SourceSlice {
        name: String::new(),
        file: rel_path.to_owned(),
        line_start: line_at(&bytes, out_start),
        line_end: if out_end > out_start {
            line_at(&bytes, out_end - 1)
        } else {
            line_at(&bytes, out_start)
        },
        byte_start: u32::try_from(out_start).unwrap_or(u32::MAX),
        byte_end: u32::try_from(out_end).unwrap_or(u32::MAX),
        revision: 0,
        stale,
        source,
        alternatives: Vec::new(),
    })
}

/// Read the whole file at `root/rel_path`, returning its bytes. Unlike
/// [`read_span`] this returns the entire file: the outline assembler folds it
/// down to signatures, and the handler computes `stale` by comparing the
/// recorded symbol spans against the returned length (against EOF). Isolating
/// `std::fs` here keeps the IO-under-`src/adapters/` convention
/// [src: context-efficient-read tier-02; CLAUDE.md conventions].
///
/// # Errors
/// Returns [`McpError::NotFound`] when the file cannot be read (missing or
/// unreadable).
pub fn read_file(root: &Path, rel_path: &str) -> Result<Vec<u8>, McpError> {
    let path = root.join(rel_path);
    std::fs::read(&path).map_err(|e| McpError::NotFound(format!("read {}: {e}", path.display())))
}

/// End offset of the signature: the first line of the declaration, stopped at
/// the body-opening `{` if it sits on that line, or with a trailing Python-
/// style `:` dropped. Keeps the declaration without its body.
fn signature_end(bytes: &[u8], start: usize) -> usize {
    let line_end = line_end(bytes, start);
    if let Some(rel) = bytes[start..line_end].iter().position(|&b| b == b'{') {
        return start + rel;
    }
    // No brace on this line: drop a trailing block-opening `:` (e.g. Python).
    if line_end > start && bytes[line_end - 1] == b':' {
        return line_end - 1;
    }
    line_end
}

/// Byte bounds of the span widened by `ctx` whole lines on each side.
fn context_bounds(bytes: &[u8], start: usize, end: usize, ctx: usize) -> (usize, usize) {
    let mut s = line_start(bytes, start);
    for _ in 0..ctx {
        if s == 0 {
            break;
        }
        s = line_start(bytes, s - 1);
    }
    let mut e = line_end(bytes, end);
    for _ in 0..ctx {
        if e >= bytes.len() {
            break;
        }
        e = line_end(bytes, e + 1);
    }
    (s, e)
}

/// Start offset of the line containing `pos` (byte after the previous `\n`,
/// or `0`).
fn line_start(bytes: &[u8], pos: usize) -> usize {
    let pos = pos.min(bytes.len());
    match bytes[..pos].iter().rposition(|&b| b == b'\n') {
        Some(i) => i + 1,
        None => 0,
    }
}

/// End offset of the line containing `pos` — the index of the next `\n`
/// (exclusive of it), or the file length when none follows.
fn line_end(bytes: &[u8], pos: usize) -> usize {
    if pos >= bytes.len() {
        return bytes.len();
    }
    match bytes[pos..].iter().position(|&b| b == b'\n') {
        Some(p) => pos + p,
        None => bytes.len(),
    }
}

/// 1-based line number of byte `offset` (one more than the `\n` count before
/// it).
// Counting newlines inline keeps the crate dependency-free; pulling in
// `bytecount` for a one-shot span read is not warranted (no new deps, D-rule).
#[allow(clippy::naive_bytecount)]
fn line_at(bytes: &[u8], offset: usize) -> u32 {
    let offset = offset.min(bytes.len());
    let count = bytes[..offset].iter().filter(|&&b| b == b'\n').count();
    u32::try_from(count + 1).unwrap_or(u32::MAX)
}
