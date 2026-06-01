//! Symbol-churn attribution use case (tier-11b).
//!
//! Joins per-commit `blob-diff` line hunks (from the symbol-agnostic
//! `ariadne-git` adapter) against symbol defining spans to produce per-symbol
//! churn — how many commits in the attributed window changed lines a symbol
//! covers. The cross-cutting join (history × symbols) lives here in the
//! use-case layer, never in the driven git adapter, which depends only on
//! `ariadne-core` and cannot know symbol ranges (ADR-0019).
//!
//! The span↔line↔overlap primitives live in [`crate::span_lines`], shared with
//! the tier-14 diff-blast seed resolver (D1); this module folds each commit's
//! touched-symbol set into per-symbol commit counts.
//!
//! Pure and deterministic: the same inputs always yield the same counts (no
//! clock, no RNG). Line ranges are interpreted against the file's HEAD layout,
//! so attribution is exact for the latest revision and degrades for commits
//! predating later line shifts — the bounded window (`symbol_churn_depth`)
//! keeps that drift small (R-C3) [src: post-v1-roadmap plan.md RD7 + tier-11b
//! step 3; <https://understandlegacycode.com/blog/key-points-of-software-design-x-rays/>].

use std::collections::BTreeMap;

use ariadne_core::{LineHunk, SymbolChurn, SymbolId};

use crate::span_lines::SymbolLineIndex;

/// Attribute each commit's changed lines to the symbols whose span covers them.
///
/// A commit is counted once for a symbol when any of that commit's changed
/// lines on the symbol's file fall within the symbol's HEAD line range; a
/// symbol touched by two hunks of the same commit is still counted once. The
/// result is sorted by `SymbolId` and lists only symbols with at least one
/// attributed commit (absent ⇒ zero churn).
#[must_use]
pub fn attribute_symbol_churn(
    symbol_lines: &[crate::span_lines::FileSymbolSpans],
    commit_hunks: &[Vec<LineHunk>],
) -> Vec<SymbolChurn> {
    // Resolve each symbol's byte span to a HEAD line range once, then probe the
    // resolved index per commit (its hunks only touch symbols sharing a file).
    let index = SymbolLineIndex::build(symbol_lines);

    let mut counts: BTreeMap<SymbolId, u32> = BTreeMap::new();
    for hunks in commit_hunks {
        // Symbols this single commit touches, deduplicated so multiple hunks
        // landing in one symbol still count the commit once.
        for symbol in index.symbols_touched_by(hunks) {
            *counts.entry(symbol).or_insert(0) += 1;
        }
    }

    counts
        .into_iter()
        .map(|(symbol, commits)| SymbolChurn { symbol, commits })
        .collect()
}
