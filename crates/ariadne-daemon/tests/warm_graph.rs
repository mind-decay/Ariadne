//! tier-07 warm-graph parity: navigation + impact queries over a real
//! `interprocess` socket equal the v1 cold-path result for the same redb
//! fixture [src: .claude/plans/post-v1-roadmap/tier-07-daemon-warm-graph.md
//! steps 1, 7].

mod support;

use ariadne_core::{
    BlastRadiusReport, ComponentRow, DaemonQuery, DaemonRequest, DaemonResponse, FileSummaryReport,
    ReferenceSite,
};
use ariadne_graph::EdgeKindSet;

use support::{apply, cold, component_changeset, seed, shutdown, spawn};

fn query(root: &std::path::Path, revision: u64, query: DaemonQuery) -> DaemonResponse {
    ariadne_daemon::query(root, &DaemonRequest { revision, query }).expect("query")
}

/// `blast_radius` over the warm socket equals the cold `ariadne-graph`
/// reverse-reachability result (tier-07 step 1).
#[test]
fn blast_radius_matches_cold_golden() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().to_path_buf();
    let revision = seed(&root);
    let reference = cold(&root);

    let target = reference.id("crate::util::helper");
    let radius = reference
        .graph
        .blast_radius(target, 3, EdgeKindSet::ALL)
        .expect("target present");
    let expect = BlastRadiusReport {
        symbol: reference.summary[&target].clone(),
        must_touch: radius
            .must_touch
            .iter()
            .map(|s| reference.summary[s].clone())
            .collect(),
        may_touch: radius
            .may_touch
            .iter()
            .map(|s| reference.summary[s].clone())
            .collect(),
        depth_used: radius.depth_used,
    };

    let handle = spawn(&root);
    let response = query(
        &root,
        revision,
        DaemonQuery::BlastRadius {
            symbol: "crate::util::helper".to_owned(),
            depth: None,
            kinds: None,
        },
    );
    shutdown(&root, handle);

    assert_eq!(response, DaemonResponse::BlastRadius(expect));
}

/// `list_symbols` + `find_definition` + `find_references` parity.
#[test]
fn navigation_queries_match_cold() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().to_path_buf();
    let revision = seed(&root);
    let reference = cold(&root);

    let handle = spawn(&root);

    // list_symbols substring filter, ascending SymbolId order.
    let symbols = query(
        &root,
        revision,
        DaemonQuery::ListSymbols {
            query: "util".to_owned(),
            kind: None,
            limit: None,
        },
    );
    assert_eq!(
        symbols,
        DaemonResponse::Symbols(vec![
            reference.summ("crate::util::helper"),
            reference.summ("crate::util::leaf"),
        ]),
    );

    // find_definition exact name.
    let def = query(
        &root,
        revision,
        DaemonQuery::FindDefinition {
            symbol: "crate::run".to_owned(),
        },
    );
    assert_eq!(
        def,
        DaemonResponse::Definition(reference.summ("crate::run"))
    );

    // find_references: the two callers of crate::util::helper, by caller id.
    let refs = query(
        &root,
        revision,
        DaemonQuery::FindReferences {
            symbol: "crate::util::helper".to_owned(),
        },
    );
    assert_eq!(
        refs,
        DaemonResponse::References(vec![
            ReferenceSite {
                caller: 2,
                caller_name: "crate::run".to_owned(),
                file: "src/lib.rs".to_owned(),
                byte_start: 64,
                byte_end: 96,
            },
            ReferenceSite {
                caller: 5,
                caller_name: "crate::helper::extra".to_owned(),
                file: "src/helper.rs".to_owned(),
                byte_start: 64,
                byte_end: 96,
            },
        ]),
    );

    // Unknown symbol is a query-level error, not a panic.
    let missing = query(
        &root,
        revision,
        DaemonQuery::FindDefinition {
            symbol: "crate::nope".to_owned(),
        },
    );
    assert!(matches!(missing, DaemonResponse::Error(_)));

    shutdown(&root, handle);
}

/// `file_summary` on a plain Rust file: symbols + fan totals, no cross-file
/// deps or components (the fixture writes every edge in its caller's file).
#[test]
fn file_summary_matches_cold() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().to_path_buf();
    let revision = seed(&root);
    let reference = cold(&root);

    let handle = spawn(&root);
    let response = query(
        &root,
        revision,
        DaemonQuery::FileSummary {
            path: "src/util.rs".to_owned(),
        },
    );
    shutdown(&root, handle);

    assert_eq!(
        response,
        DaemonResponse::FileSummary(FileSummaryReport {
            path: "src/util.rs".to_owned(),
            symbols: vec![
                reference.summ("crate::util::helper"),
                reference.summ("crate::util::leaf"),
                reference.summ("crate::unused_helper"),
            ],
            fan_in: 3,
            fan_out: 1,
            top_dependencies: vec![],
            components: vec![],
        }),
    );
}

/// `file_summary` on a component file surfaces the render/hook neighbourhood
/// (ADR-0012), exercising the warm snapshot's outgoing-edge mirror.
#[test]
fn file_summary_component_graph_matches_cold() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().to_path_buf();
    let revision = apply(&root, &component_changeset());
    let reference = cold(&root);

    let handle = spawn(&root);
    let response = query(
        &root,
        revision,
        DaemonQuery::FileSummary {
            path: "src/Card.vue".to_owned(),
        },
    );
    shutdown(&root, handle);

    assert_eq!(
        response,
        DaemonResponse::FileSummary(FileSummaryReport {
            path: "src/Card.vue".to_owned(),
            symbols: vec![reference.summ("Card")],
            fan_in: 1,
            fan_out: 2,
            top_dependencies: vec![],
            components: vec![ComponentRow {
                component: "Card".to_owned(),
                renders: vec!["Button".to_owned()],
                hooks: vec!["useToggle".to_owned()],
            }],
        }),
    );
}
