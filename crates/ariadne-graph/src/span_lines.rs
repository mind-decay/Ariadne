//! Shared span↔line↔overlap primitives for the history × symbols join.
//!
//! Both the per-commit symbol-churn attribution (tier-11b,
//! [`crate::symbol_churn`]) and the diff-aware blast-radius seed resolution
//! (tier-14, [`crate::diff_blast`]) reduce a set of changed line hunks to the
//! symbols whose HEAD defining span covers them. The byte→line conversion and
//! the line-overlap math live here once (DRY); a [`SymbolLineIndex`] resolves a
//! symbol set's defining spans to HEAD line ranges a single time, then answers
//! per-changeset overlap queries.
//!
//! Pure and deterministic: every result is a function of its inputs, with no
//! clock and no RNG. Line ranges are interpreted against the file's HEAD layout,
//! so resolution is exact for the latest revision and degrades for historical
//! line shifts — the same bounded approximation tier-11b accepts (ADR-0019)
//! [src: .claude/plans/post-v1-roadmap/tier-14-diff-aware-blast-radius.md D1].

use std::collections::{BTreeMap, BTreeSet};

use ariadne_core::{LineHunk, SymbolId};

/// Per-file attribution input: the path as it appears in Git history, the HEAD
/// line index, and the symbols whose defining occurrence lives in the file.
///
/// `line_starts[i]` is the byte offset of line `i + 1`'s first byte (line 1 at
/// offset 0), strictly ascending — the same layout the symbol byte spans were
/// computed against. Each symbol is `(id, byte_start, byte_end)`, a half-open
/// byte span. The byte→line conversion + symbol join stay inside `ariadne-graph`
/// (the git adapter emits paths + line ranges only) [src: ADR-0019].
#[derive(Debug, Clone)]
pub struct FileSymbolSpans {
    /// Repository-root-relative path of the file the symbols define in.
    pub path: String,
    /// Byte offset of each line's first byte at HEAD; line 1 at offset 0.
    pub line_starts: Vec<u32>,
    /// `(symbol, byte_start, byte_end)` half-open defining byte spans.
    pub symbols: Vec<(SymbolId, u32, u32)>,
}

/// One file's pre-guard join input (tier-15c D3): the path, the indexed content
/// hash (the byte-offset validity guard), the symbols' defining byte spans, and
/// the on-disk bytes the caller read. [`spans_from`] turns a batch of these into
/// validated [`FileSymbolSpans`], shared by the CLI symbol-churn pass, the daemon
/// `diff_blast` handler, and the cold MCP `diff_blast` tool so all three agree on
/// the line index + staleness rule.
#[derive(Debug, Clone)]
pub struct FileSpanSource {
    /// Repository-root-relative path of the file.
    pub path: String,
    /// Indexed content hash; on-disk bytes must hash to it or the file is dropped.
    pub blake3: [u8; 32],
    /// `(symbol, byte_start, byte_end)` half-open defining byte spans.
    pub symbols: Vec<(SymbolId, u32, u32)>,
    /// The file's current on-disk bytes, read by the caller (the composition root
    /// owns the IO; this crate stays free of a filesystem dependency).
    pub content: Vec<u8>,
}

/// Build the per-file [`FileSymbolSpans`] join input from raw `sources`, dropping
/// any file whose on-disk bytes no longer hash to its indexed `blake3` (stale
/// byte offsets). A dropped file contributes no spans, so its changed path
/// surfaces downstream as unresolved rather than seeding a wrong symbol (D3). The
/// result is sorted by path so downstream output is deterministic.
#[must_use]
pub fn spans_from(sources: Vec<FileSpanSource>) -> Vec<FileSymbolSpans> {
    let mut out: Vec<FileSymbolSpans> = sources
        .into_iter()
        .filter(|s| blake3::hash(&s.content).as_bytes() == &s.blake3)
        .map(|s| FileSymbolSpans {
            line_starts: line_starts(&s.content),
            path: s.path,
            symbols: s.symbols,
        })
        .collect();
    out.sort_by(|a, b| a.path.cmp(&b.path));
    out
}

/// Byte offset of each line's first byte (line 1 at offset 0): the HEAD line
/// index symbol byte spans are converted against. A trailing newline yields a
/// final start at `content.len()`, which is harmless — no symbol byte offset maps
/// there.
#[must_use]
pub fn line_starts(content: &[u8]) -> Vec<u32> {
    let mut starts = vec![0u32];
    for (idx, &byte) in content.iter().enumerate() {
        if byte == b'\n' {
            if let Ok(next) = u32::try_from(idx + 1) {
                starts.push(next);
            }
        }
    }
    starts
}

/// One symbol's defining occurrence resolved to a 1-based inclusive HEAD line
/// range, grouped by path for the per-changeset overlap check.
struct SymbolLineRange {
    symbol: SymbolId,
    start_line: u32,
    end_line: u32,
}

/// Symbol defining spans resolved to HEAD line ranges once, grouped by path so a
/// changeset's hunks only probe symbols sharing their file. Build it once, then
/// query it per changeset with [`SymbolLineIndex::symbols_touched_by`].
pub(crate) struct SymbolLineIndex<'a> {
    by_path: BTreeMap<&'a str, Vec<SymbolLineRange>>,
}

impl<'a> SymbolLineIndex<'a> {
    /// Resolve every symbol's byte span to a HEAD line range, grouped by path.
    pub(crate) fn build(symbol_lines: &'a [FileSymbolSpans]) -> Self {
        let mut by_path: BTreeMap<&str, Vec<SymbolLineRange>> = BTreeMap::new();
        for file in symbol_lines {
            let ranges = by_path.entry(file.path.as_str()).or_default();
            for &(symbol, byte_start, byte_end) in &file.symbols {
                let (start_line, end_line) =
                    byte_span_to_lines(&file.line_starts, byte_start, byte_end);
                ranges.push(SymbolLineRange {
                    symbol,
                    start_line,
                    end_line,
                });
            }
        }
        Self { by_path }
    }

    /// The set of symbols whose HEAD line range overlaps any of `hunks`. A
    /// symbol touched by two hunks is still listed once (`BTreeSet` dedup); the
    /// set iterates in `SymbolId` order so callers stay deterministic.
    pub(crate) fn symbols_touched_by(&self, hunks: &[LineHunk]) -> BTreeSet<SymbolId> {
        let mut touched: BTreeSet<SymbolId> = BTreeSet::new();
        for hunk in hunks {
            let Some(ranges) = self.by_path.get(hunk.path.as_str()) else {
                continue;
            };
            for range in ranges {
                if overlaps(
                    hunk.start_line,
                    hunk.end_line,
                    range.start_line,
                    range.end_line,
                ) {
                    touched.insert(range.symbol);
                }
            }
        }
        touched
    }
}

/// Resolve a single changeset's line `hunks` to the seed set of symbols whose
/// HEAD defining span covers any changed line. Convenience over
/// [`SymbolLineIndex`] for one-shot callers (the tier-14 diff-blast seed set);
/// repeated callers (per-commit churn) build the index once and reuse it.
pub(crate) fn changed_symbols(
    symbol_lines: &[FileSymbolSpans],
    hunks: &[LineHunk],
) -> BTreeSet<SymbolId> {
    SymbolLineIndex::build(symbol_lines).symbols_touched_by(hunks)
}

/// Convert a half-open byte span `[byte_start, byte_end)` to a 1-based inclusive
/// line range against `line_starts`. The line of a byte is the count of line
/// starts at or before it; the span's last line uses `byte_end - 1` (its last
/// covered byte), clamped to `byte_start` for an empty span. An empty
/// `line_starts` (empty file) collapses to line 1.
fn byte_span_to_lines(line_starts: &[u32], byte_start: u32, byte_end: u32) -> (u32, u32) {
    let last_byte = byte_end.saturating_sub(1).max(byte_start);
    (
        line_of(line_starts, byte_start),
        line_of(line_starts, last_byte),
    )
}

/// 1-based line number of `byte`: the number of line starts at or before it
/// (at least 1, since line 1 starts at offset 0).
fn line_of(line_starts: &[u32], byte: u32) -> u32 {
    let line = line_starts.partition_point(|&start| start <= byte);
    u32::try_from(line).unwrap_or(u32::MAX).max(1)
}

/// Whether two 1-based inclusive line ranges intersect.
fn overlaps(a_start: u32, a_end: u32, b_start: u32, b_end: u32) -> bool {
    a_start <= b_end && b_start <= a_end
}
