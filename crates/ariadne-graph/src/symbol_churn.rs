//! Symbol-churn attribution use case (tier-11b).
//!
//! Joins per-commit `blob-diff` line hunks (from the symbol-agnostic
//! `ariadne-git` adapter) against symbol defining spans to produce per-symbol
//! churn — how many commits in the attributed window changed lines a symbol
//! covers. The cross-cutting join (history × symbols) lives here in the
//! use-case layer, never in the driven git adapter, which depends only on
//! `ariadne-core` and cannot know symbol ranges (ADR-0019).
//!
//! Pure and deterministic: the same inputs always yield the same counts (no
//! clock, no RNG). Line ranges are interpreted against the file's HEAD layout,
//! so attribution is exact for the latest revision and degrades for commits
//! predating later line shifts — the bounded window (`symbol_churn_depth`)
//! keeps that drift small (R-C3) [src: post-v1-roadmap plan.md RD7 + tier-11b
//! step 3; <https://understandlegacycode.com/blog/key-points-of-software-design-x-rays/>].

use std::collections::{BTreeMap, BTreeSet};

use ariadne_core::{LineHunk, SymbolChurn, SymbolId};

/// Per-file attribution input: the path as it appears in Git history, the HEAD
/// line index, and the symbols whose defining occurrence lives in the file.
///
/// `line_starts[i]` is the byte offset of line `i + 1`'s first byte (line 1 at
/// offset 0), strictly ascending — the same layout the symbol byte spans were
/// computed against. Each symbol is `(id, byte_start, byte_end)`, a half-open
/// byte span. The use-case converts every span to a HEAD line range against
/// `line_starts`, so the byte→line conversion + symbol join stay inside
/// `ariadne-graph` (the git adapter emits paths + line ranges only) [src:
/// tier-11b steps 3-4].
#[derive(Debug, Clone)]
pub struct FileSymbolSpans {
    /// Repository-root-relative path of the file the symbols define in.
    pub path: String,
    /// Byte offset of each line's first byte at HEAD; line 1 at offset 0.
    pub line_starts: Vec<u32>,
    /// `(symbol, byte_start, byte_end)` half-open defining byte spans.
    pub symbols: Vec<(SymbolId, u32, u32)>,
}

/// One symbol's defining occurrence resolved to a 1-based inclusive HEAD line
/// range, grouped by path for the per-commit overlap check.
struct SymbolLineRange {
    symbol: SymbolId,
    start_line: u32,
    end_line: u32,
}

/// Attribute each commit's changed lines to the symbols whose span covers them.
///
/// A commit is counted once for a symbol when any of that commit's changed
/// lines on the symbol's file fall within the symbol's HEAD line range; a
/// symbol touched by two hunks of the same commit is still counted once. The
/// result is sorted by `SymbolId` and lists only symbols with at least one
/// attributed commit (absent ⇒ zero churn).
#[must_use]
pub fn attribute_symbol_churn(
    symbol_lines: &[FileSymbolSpans],
    commit_hunks: &[Vec<LineHunk>],
) -> Vec<SymbolChurn> {
    // Resolve each symbol's byte span to a HEAD line range once, grouped by
    // path so a commit's hunks only probe symbols sharing their file.
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

    let mut counts: BTreeMap<SymbolId, u32> = BTreeMap::new();
    for hunks in commit_hunks {
        // Symbols this single commit touches, deduplicated so multiple hunks
        // landing in one symbol still count the commit once.
        let mut touched: BTreeSet<SymbolId> = BTreeSet::new();
        for hunk in hunks {
            let Some(ranges) = by_path.get(hunk.path.as_str()) else {
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
        for symbol in touched {
            *counts.entry(symbol).or_insert(0) += 1;
        }
    }

    counts
        .into_iter()
        .map(|(symbol, commits)| SymbolChurn { symbol, commits })
        .collect()
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
