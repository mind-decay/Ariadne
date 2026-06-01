//! tier-15a — analytics catalog projection (cold MCP `Catalog`).
//!
//! The cold catalog loads the Block-C analytics substrate — per-file churn,
//! co-change pairs, and per-symbol churn — from the `Storage` port at build
//! time, sorted by key, and threads per-symbol cyclomatic `complexity` onto
//! `SymbolMeta`, so the 15b/15c tools become pure in-RAM reads. The same
//! fixture is asserted field-for-field on the warm side by the
//! `ariadne-daemon` `catalog` unit test; the two projections are field-equal
//! [src: .claude/plans/post-v1-roadmap/tier-15a-analytics-catalog-projection.md].

mod support;

use ariadne_core::{CoChangePair, SymbolChurn};
use ariadne_mcp::Catalog;
use ariadne_storage::RedbStorage;

use support::{seed_analytics_project, sid};

/// Building the cold catalog from the seeded fixture exposes complexity on
/// every symbol meta and loads churn / co-change / symbol-churn, each sorted
/// by key.
#[test]
fn catalog_loads_analytics_and_complexity() {
    let (root, _guard) = seed_analytics_project();
    let storage =
        RedbStorage::open(&root.join(".ariadne").join("index.redb")).expect("open seeded redb");
    let cat = Catalog::build(&storage, root.display().to_string()).expect("build cold catalog");

    // Complexity is threaded onto every per-symbol meta from SymbolRecord.
    assert_eq!(
        cat.meta_of(sid(1)).expect("alpha meta").complexity,
        7,
        "non-zero complexity rides SymbolMeta",
    );
    assert_eq!(cat.meta_of(sid(2)).expect("beta meta").complexity, 3);

    // File churn loaded and sorted by path (D3): alpha before beta despite the
    // fixture persisting them in the reverse order.
    assert_eq!(
        cat.churn
            .iter()
            .map(|c| (c.path.as_str(), c.commits, c.authors()))
            .collect::<Vec<_>>(),
        vec![("src/alpha.rs", 9, 1), ("src/beta.rs", 4, 2)],
    );

    // Co-change loaded.
    assert_eq!(
        cat.co_change,
        vec![CoChangePair {
            a: "src/alpha.rs".into(),
            b: "src/beta.rs".into(),
            count: 3,
        }],
    );

    // Symbol churn loaded and sorted by SymbolId (D3): sid(1) before sid(2)
    // despite the fixture persisting them in the reverse order.
    assert_eq!(
        cat.symbol_churn,
        vec![
            SymbolChurn {
                symbol: sid(1),
                commits: 5,
            },
            SymbolChurn {
                symbol: sid(2),
                commits: 2,
            },
        ],
    );
}
