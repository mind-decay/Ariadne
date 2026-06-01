//! Symbol-churn attribution use-case
//! [src: .claude/plans/post-v1-roadmap/tier-11b-symbol-churn-attribution.md step 1].
//!
//! A fixture file with two functions and a commit editing only the first:
//! the use-case credits the first symbol and not the second. Pure inputs (no
//! git), so the test exercises the byte-span → line-range conversion and the
//! line-overlap join in isolation. A re-run yields identical counts, pinning
//! determinism (exit-criteria: same index, same per-symbol counts).

use ariadne_core::{LineHunk, SymbolId};
use ariadne_graph::{FileSymbolSpans, attribute_symbol_churn};

fn sid(n: u64) -> SymbolId {
    SymbolId::new(n).expect("nonzero symbol id")
}

fn hunk(path: &str, start_line: u32, end_line: u32) -> LineHunk {
    LineHunk {
        path: path.to_owned(),
        start_line,
        end_line,
    }
}

/// `lib.rs`: `fn first` defines bytes `[0, 30)` (lines 1-3) and `fn second`
/// bytes `[40, 70)` (lines 5-7). Ten-byte lines keep the HEAD line index
/// trivial: line `n` starts at byte `10*(n-1)`.
fn two_function_file() -> Vec<FileSymbolSpans> {
    vec![FileSymbolSpans {
        path: "lib.rs".to_owned(),
        line_starts: vec![0, 10, 20, 30, 40, 50, 60, 70],
        symbols: vec![(sid(1), 0, 30), (sid(2), 40, 70)],
    }]
}

#[test]
fn credits_only_the_edited_symbol() {
    let symbol_lines = two_function_file();
    // One commit edits line 2 — inside `first` (lines 1-3), clear of `second`.
    let commit_hunks = vec![vec![hunk("lib.rs", 2, 2)]];

    let churn = attribute_symbol_churn(&symbol_lines, &commit_hunks);

    assert_eq!(churn.len(), 1, "only the edited symbol is credited");
    assert_eq!(churn[0].symbol, sid(1));
    assert_eq!(churn[0].commits, 1);

    // Determinism: the same inputs yield byte-identical counts on a re-run.
    assert_eq!(
        attribute_symbol_churn(&symbol_lines, &commit_hunks),
        churn,
        "attribution is deterministic across runs",
    );
}

#[test]
fn editing_the_second_function_credits_only_it() {
    let symbol_lines = two_function_file();
    // A hunk on line 6 lands inside `second` (lines 5-7) only.
    let commit_hunks = vec![vec![hunk("lib.rs", 6, 6)]];

    let churn = attribute_symbol_churn(&symbol_lines, &commit_hunks);

    assert_eq!(churn.len(), 1);
    assert_eq!(churn[0].symbol, sid(2));
    assert_eq!(churn[0].commits, 1);
}

#[test]
fn counts_distinct_commits_per_symbol_without_double_counting() {
    let symbol_lines = two_function_file();
    // Commit A edits both functions; commit B edits only the first; a third
    // commit edits an unrelated path. `first` is touched twice, `second` once.
    let commit_hunks = vec![
        vec![hunk("lib.rs", 1, 1), hunk("lib.rs", 7, 7)],
        vec![hunk("lib.rs", 3, 3)],
        vec![hunk("other.rs", 1, 9)],
    ];

    let churn = attribute_symbol_churn(&symbol_lines, &commit_hunks);

    assert_eq!(churn.len(), 2);
    assert_eq!(churn[0].symbol, sid(1));
    assert_eq!(
        churn[0].commits, 2,
        "first touched by commit A and commit B"
    );
    assert_eq!(churn[1].symbol, sid(2));
    assert_eq!(churn[1].commits, 1, "second touched by commit A only");
}

#[test]
fn a_hunk_between_two_symbols_credits_neither() {
    let symbol_lines = two_function_file();
    // Line 4 is the blank gap between `first` (1-3) and `second` (5-7).
    let commit_hunks = vec![vec![hunk("lib.rs", 4, 4)]];

    assert!(
        attribute_symbol_churn(&symbol_lines, &commit_hunks).is_empty(),
        "a change outside every symbol span credits no symbol",
    );
}
