//! Diff-aware blast-radius use-case
//! [src: .claude/plans/post-v1-roadmap/tier-14-diff-aware-blast-radius.md step 1].
//!
//! A two-file changeset seeds the blast: a hunk inside `first` (file `a.rs`)
//! and a hunk inside `fourth` (file `b.rs`) resolve to symbol seeds `sid(1)` and
//! `sid(4)`; a third changed path covers no symbol and must surface as an
//! unresolved-impact entry. The returned must∪may impact set must equal the
//! union over those seeds of v1 `blast_radius` (must∪may), both sides computed
//! from the same `GraphIndex` — the union semantics the report promises (D4). A
//! re-run yields a byte-identical report, pinning determinism.

use std::collections::BTreeSet;
use std::fmt::Write as _;

use ariadne_core::{LineHunk, SymbolId};
use ariadne_graph::{DiffBlastReport, EdgeKind, EdgeKindSet, FileSymbolSpans, GraphIndex};

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

/// Two reverse-call chains feeding two seeds:
/// `sid(3) → sid(2) → sid(1)` and `sid(6) → sid(5) → sid(4)` (edge `x → y`
/// reads "x depends on y", so `blast_radius(y)` reaches its callers). Seeding on
/// `sid(1)` and `sid(4)` makes the impact union `{2, 3, 5, 6}`.
fn two_chain_graph() -> GraphIndex {
    let mut g = GraphIndex::new();
    for n in 1..=6 {
        g.add_symbol(sid(n));
    }
    g.add_edge(sid(2), sid(1), EdgeKind::Calls);
    g.add_edge(sid(3), sid(2), EdgeKind::Calls);
    g.add_edge(sid(5), sid(4), EdgeKind::Calls);
    g.add_edge(sid(6), sid(5), EdgeKind::Calls);
    g
}

/// `a.rs` defines `sid(1)` over bytes `[0, 20)` (lines 1-2); `b.rs` defines
/// `sid(4)` over bytes `[0, 20)`. Ten-byte lines: line `n` starts at `10*(n-1)`.
fn two_file_spans() -> Vec<FileSymbolSpans> {
    vec![
        FileSymbolSpans {
            path: "a.rs".to_owned(),
            line_starts: vec![0, 10, 20, 30],
            symbols: vec![(sid(1), 0, 20)],
        },
        FileSymbolSpans {
            path: "b.rs".to_owned(),
            line_starts: vec![0, 10, 20, 30],
            symbols: vec![(sid(4), 0, 20)],
        },
    ]
}

fn fmt_report(report: &DiffBlastReport) -> String {
    let mut out = String::new();
    for seed in &report.seeds {
        writeln!(
            out,
            "seed {} must={:?} may={:?} depth={}",
            seed.symbol.get(),
            seed.must_touch.iter().map(|s| s.get()).collect::<Vec<_>>(),
            seed.may_touch.iter().map(|s| s.get()).collect::<Vec<_>>(),
            seed.depth_used,
        )
        .expect("writing to a String never fails");
    }
    writeln!(
        out,
        "union must={:?} may={:?}",
        report
            .must_touch
            .iter()
            .map(|s| s.get())
            .collect::<Vec<_>>(),
        report.may_touch.iter().map(|s| s.get()).collect::<Vec<_>>(),
    )
    .expect("writing to a String never fails");
    writeln!(out, "unresolved {:?}", report.unresolved).expect("write");
    out
}

#[test]
fn impact_union_equals_per_seed_blast_radius_union() {
    let g = two_chain_graph();
    let spans = two_file_spans();
    // A hunk inside `sid(1)` (line 1) and one inside `sid(4)` (line 2); the third
    // changed path covers no symbol.
    let hunks = vec![hunk("a.rs", 1, 1), hunk("b.rs", 2, 2)];
    let changed_paths = vec![
        "a.rs".to_owned(),
        "b.rs".to_owned(),
        "newfile.rs".to_owned(),
    ];

    let report = g.diff_blast(&spans, &hunks, &changed_paths, 5, EdgeKindSet::ALL);

    // Left side: the report's deduped must∪may impact set.
    let lhs: BTreeSet<SymbolId> = report
        .must_touch
        .iter()
        .chain(report.may_touch.iter())
        .copied()
        .collect();

    // Right side: the union over the changed seeds of v1 blast_radius (must∪may),
    // computed from the SAME graph.
    let mut rhs: BTreeSet<SymbolId> = BTreeSet::new();
    for seed in [sid(1), sid(4)] {
        let br = g
            .blast_radius(seed, 5, EdgeKindSet::ALL)
            .expect("seed present");
        rhs.extend(br.must_touch);
        rhs.extend(br.may_touch);
    }

    assert_eq!(
        lhs, rhs,
        "diff_blast must∪may equals the per-seed blast_radius union",
    );
    assert_eq!(
        rhs,
        BTreeSet::from([sid(2), sid(3), sid(5), sid(6)]),
        "both chains' callers are reached",
    );

    // must and may are disjoint (must wins on conflict).
    let must: BTreeSet<SymbolId> = report.must_touch.iter().copied().collect();
    let may: BTreeSet<SymbolId> = report.may_touch.iter().copied().collect();
    assert!(must.is_disjoint(&may), "must and may sets are disjoint");

    // The two seeds are listed, sorted by SymbolId.
    let seed_ids: Vec<u64> = report.seeds.iter().map(|s| s.symbol.get()).collect();
    assert_eq!(seed_ids, vec![1, 4], "both seeds listed, sorted");

    // The changed path covering no symbol is an unresolved-impact entry,
    // never silently dropped.
    assert_eq!(
        report.unresolved,
        vec!["newfile.rs".to_owned()],
        "a changed file with no resolved symbol is unresolved",
    );

    // Determinism: the same inputs yield a byte-identical report on a re-run.
    assert_eq!(
        g.diff_blast(&spans, &hunks, &changed_paths, 5, EdgeKindSet::ALL),
        report,
        "diff_blast is deterministic across runs",
    );

    insta::assert_snapshot!("diff_blast_two_chains", fmt_report(&report));
}
