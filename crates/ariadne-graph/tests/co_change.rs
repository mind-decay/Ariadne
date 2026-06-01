//! Change-coupling (logical-coupling) use-case
//! [src: .claude/plans/post-v1-roadmap/tier-13-hotspot-cochange-metrics.md steps 3-4, 6].
//!
//! Emits a coupling edge per file pair that clears code-maat's three filters:
//! each endpoint's individual revisions ≥ `min_revs`, the shared-commit count ≥
//! `min_shared_commits`, and the degree `shared / mean(revs_a, revs_b)` ≥
//! `min_degree` (D3). Pure inputs (no git, no storage) exercise the degree
//! formula and each filter in isolation; a re-run yields a byte-identical
//! report, pinning determinism (D4).

use std::fmt::Write as _;

use ariadne_core::{CoChangePair, FileChurn};
use ariadne_graph::{CoChangeConfig, CoChangeReport, co_change_report};

fn file_churn(path: &str, commits: u32) -> FileChurn {
    FileChurn {
        path: path.to_owned(),
        commits,
        author_keys: vec![],
        last_changed_ns: 0,
    }
}

fn pair(a: &str, b: &str, count: u32) -> CoChangePair {
    CoChangePair {
        a: a.to_owned(),
        b: b.to_owned(),
        count,
    }
}

/// Per-file revisions and candidate pairs spanning every filter outcome.
/// Defaults: `min_revs=5`, `min_shared_commits=5`, `min_degree=0.30`.
fn inputs() -> (Vec<FileChurn>, Vec<CoChangePair>) {
    let churn = vec![
        file_churn("src/a.rs", 10),
        file_churn("src/b.rs", 8),
        file_churn("src/c.rs", 12),
        file_churn("src/d.rs", 20),
        file_churn("src/rare.rs", 3), // below min_revs
        file_churn("src/big1.rs", 30),
        file_churn("src/big2.rs", 40),
    ];
    let pairs = vec![
        pair("src/a.rs", "src/b.rs", 6),       // KEEP: degree 6/9 = 0.6667
        pair("src/b.rs", "src/d.rs", 7),       // KEEP: degree 7/14 = 0.5000
        pair("src/c.rs", "src/d.rs", 5),       // KEEP: degree 5/16 = 0.3125
        pair("src/a.rs", "src/c.rs", 3),       // DROP: count 3 < min_shared_commits
        pair("src/a.rs", "src/rare.rs", 6),    // DROP: rare.rs revs 3 < min_revs
        pair("src/big1.rs", "src/big2.rs", 5), // DROP: degree 5/35 = 0.1429 < min_degree
        pair("src/a.rs", "src/missing.rs", 9), // DROP: missing.rs absent from churn
    ];
    (churn, pairs)
}

fn fmt_report(report: &CoChangeReport) -> String {
    let mut out = String::new();
    for e in &report.edges {
        writeln!(
            out,
            "{} <-> {} shared={} degree={:.4}",
            e.a, e.b, e.shared_commits, e.degree
        )
        .expect("writing to a String never fails");
    }
    out
}

#[test]
fn keeps_coupled_pairs_with_expected_degree_and_drops_filtered() {
    let (churn, pairs) = inputs();
    let cfg = CoChangeConfig::default();

    let report = co_change_report(&churn, &pairs, &cfg);

    // Exactly the three pairs that clear all three filters survive.
    assert_eq!(report.edges.len(), 3, "only above-threshold pairs survive");

    // Sorted by degree descending; the strongest coupling is first with the
    // code-maat degree shared / mean(revs).
    let top = &report.edges[0];
    assert_eq!(
        (&top.a, &top.b),
        (&"src/a.rs".to_owned(), &"src/b.rs".to_owned())
    );
    assert_eq!(top.shared_commits, 6);
    assert!(
        (top.degree - 6.0 / 9.0).abs() < 1e-6,
        "degree = shared / mean(revs_a, revs_b) = 6/((10+8)/2) = 0.6667, got {}",
        top.degree,
    );

    // None of the filtered pairs leak through.
    let present: Vec<(&str, &str)> = report
        .edges
        .iter()
        .map(|e| (e.a.as_str(), e.b.as_str()))
        .collect();
    assert!(
        !present.contains(&("src/a.rs", "src/c.rs")),
        "a pair below min_shared_commits is excluded",
    );
    assert!(
        !present.contains(&("src/a.rs", "src/rare.rs")),
        "a pair with an endpoint below min_revs is excluded",
    );
    assert!(
        !present.contains(&("src/big1.rs", "src/big2.rs")),
        "a pair below min_degree is excluded",
    );
    assert!(
        !present.contains(&("src/a.rs", "src/missing.rs")),
        "a pair with an endpoint absent from churn is excluded",
    );

    // Determinism: the same inputs yield a byte-identical report on a re-run.
    assert_eq!(
        co_change_report(&churn, &pairs, &cfg),
        report,
        "co-change report is deterministic across runs",
    );

    insta::assert_snapshot!("co_change", fmt_report(&report));
}

#[test]
fn default_config_matches_code_maat_thresholds() {
    let cfg = CoChangeConfig::default();
    assert_eq!(cfg.min_revs, 5);
    assert_eq!(cfg.min_shared_commits, 5);
    assert!((cfg.min_degree - 0.30).abs() < 1e-6);
}
