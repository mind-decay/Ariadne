//! Hotspot ranking use-case
//! [src: .claude/plans/post-v1-roadmap/tier-13-hotspot-cochange-metrics.md steps 1-2, 6].
//!
//! A hotspot is code that is *both* frequently changed *and* complex (D2). The
//! score is the product of project-max-normalized churn and complexity, so a
//! unit with zero churn or zero complexity scores exactly `0.0` and the unit
//! that is high on both ranks first. Pure inputs (no git, no storage) exercise
//! the normalization and product ranking in isolation; a re-run yields a
//! byte-identical report, pinning determinism (D4).

use std::collections::BTreeMap;
use std::fmt::Write as _;

use ariadne_core::{FileChurn, SymbolChurn, SymbolId};
use ariadne_graph::{HotspotGrain, HotspotReport, file_hotspots, symbol_hotspots};

fn sid(n: u64) -> SymbolId {
    SymbolId::new(n).expect("nonzero symbol id")
}

fn file_churn(path: &str, commits: u32) -> FileChurn {
    FileChurn {
        path: path.to_owned(),
        commits,
        author_keys: vec![],
        last_changed_ns: 0,
    }
}

/// Four files spanning the score space: `hot` is high on both axes (the known
/// hotspot), `complex_stable` is max complexity but barely churned,
/// `churned_simple` is high churn but zero complexity (must score 0), `cold` is
/// low on both. Max churn = 10 (`hot`), max complexity = 25 (`complex_stable`).
fn file_inputs() -> (Vec<FileChurn>, BTreeMap<String, u32>) {
    let churn = vec![
        file_churn("src/hot.rs", 10),
        file_churn("src/cold.rs", 2),
        file_churn("src/churned_simple.rs", 8),
        file_churn("src/complex_stable.rs", 1),
    ];
    let complexity = BTreeMap::from([
        ("src/hot.rs".to_owned(), 20),
        ("src/cold.rs".to_owned(), 5),
        // churned_simple absent ⇒ complexity 0 ⇒ score 0.
        ("src/complex_stable.rs".to_owned(), 25),
    ]);
    (churn, complexity)
}

fn fmt_report(report: &HotspotReport) -> String {
    let mut out = String::new();
    for e in &report.entries {
        let key = match &e.grain {
            HotspotGrain::File { path } => format!("file {path}"),
            HotspotGrain::Symbol { symbol } => format!("symbol {}", symbol.get()),
        };
        writeln!(
            out,
            "{key} churn={} cx={} score={:.4}",
            e.churn, e.complexity, e.score
        )
        .expect("writing to a String never fails");
    }
    out
}

#[test]
// The zero-factor product is an exact `0.0` (`x * 0.0`), so an exact compare
// is intentional, not an approximate-equality bug.
#[allow(clippy::float_cmp)]
fn hot_file_ranks_first_and_zero_complexity_scores_zero() {
    let (churn, complexity) = file_inputs();

    let report = file_hotspots(&churn, &complexity);

    // The unit high on BOTH axes ranks first (score 1.0*0.8 = 0.8), beating
    // complex_stable (0.1*1.0 = 0.1) which is max complexity but barely churned.
    assert_eq!(
        report.entries[0].grain,
        HotspotGrain::File {
            path: "src/hot.rs".to_owned()
        },
        "the churned-and-complex file ranks first",
    );

    // A file with zero complexity scores exactly 0 despite high churn.
    let simple = report
        .entries
        .iter()
        .find(|e| {
            e.grain
                == HotspotGrain::File {
                    path: "src/churned_simple.rs".to_owned(),
                }
        })
        .expect("churned_simple present");
    assert_eq!(simple.score, 0.0, "zero complexity ⇒ zero hotspot score");

    // Determinism: the same inputs yield a byte-identical report on a re-run.
    assert_eq!(
        file_hotspots(&churn, &complexity),
        report,
        "file hotspots are deterministic across runs",
    );

    insta::assert_snapshot!("file_hotspots", fmt_report(&report));
}

#[test]
// The zero-factor product is an exact `0.0` (`x * 0.0`), so an exact compare
// is intentional, not an approximate-equality bug.
#[allow(clippy::float_cmp)]
fn symbol_hotspot_ranks_first_and_zero_complexity_scores_zero() {
    // sid(1) hot on both (max churn 10, complexity 12); sid(3) high churn but
    // zero complexity (absent from the map) ⇒ score 0.
    let churn = vec![
        SymbolChurn {
            symbol: sid(1),
            commits: 10,
        },
        SymbolChurn {
            symbol: sid(2),
            commits: 3,
        },
        SymbolChurn {
            symbol: sid(3),
            commits: 6,
        },
    ];
    let complexity = BTreeMap::from([(sid(1), 12), (sid(2), 4)]);

    let report = symbol_hotspots(&churn, &complexity);

    assert_eq!(
        report.entries[0].grain,
        HotspotGrain::Symbol { symbol: sid(1) },
        "the churned-and-complex symbol ranks first",
    );
    let simple = report
        .entries
        .iter()
        .find(|e| e.grain == HotspotGrain::Symbol { symbol: sid(3) })
        .expect("sid(3) present");
    assert_eq!(simple.score, 0.0, "zero complexity ⇒ zero hotspot score");

    assert_eq!(
        symbol_hotspots(&churn, &complexity),
        report,
        "symbol hotspots are deterministic across runs",
    );

    insta::assert_snapshot!("symbol_hotspots", fmt_report(&report));
}
