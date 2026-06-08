//! tier-07 warm-graph parity: navigation + impact queries over a real
//! `interprocess` socket equal the v1 cold-path result for the same redb
//! fixture [src: .claude/plans/post-v1-roadmap/tier-07-daemon-warm-graph.md
//! steps 1, 7].

mod support;

use ariadne_core::{
    BlastRadiusReport, Changeset, ComponentRow, DaemonQuery, DaemonRequest, DaemonResponse,
    DiffBlastReport, DiffSeed, EdgeKey, EdgeKind, EdgeRecord, FileRecord, FileSummaryReport, Lang,
    LineHunk, ReferenceSite, ReferencesReport, Span, SymbolRecord, Verbosity, Visibility,
};
use ariadne_graph::EdgeKindSet;

use support::{apply, cold, component_changeset, fid, seed, shutdown, sid, spawn};

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

    // find_references (detailed): the two callers of crate::util::helper, now
    // ordered by the stable (file, byte_start, caller_name) key — src/helper.rs
    // sorts before src/lib.rs — and wrapped in a single-page report.
    let refs = query(
        &root,
        revision,
        DaemonQuery::FindReferences {
            symbol: "crate::util::helper".to_owned(),
            limit: None,
            cursor: None,
            verbosity: Verbosity::Detailed,
        },
    );
    assert_eq!(
        refs,
        DaemonResponse::References(ReferencesReport {
            references: vec![
                ReferenceSite {
                    caller: Some(5),
                    caller_name: "crate::helper::extra".to_owned(),
                    file: "src/helper.rs".to_owned(),
                    byte_start: Some(64),
                    byte_end: Some(96),
                },
                ReferenceSite {
                    caller: Some(2),
                    caller_name: "crate::run".to_owned(),
                    file: "src/lib.rs".to_owned(),
                    byte_start: Some(64),
                    byte_end: Some(96),
                },
            ],
            next_cursor: None,
            note: None,
        }),
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

/// Worktree content of the diff-blast fixture: line 2 (inside `callee`) holds
/// the uncommitted edit the `WorkingTree` hunk scopes.
const DIFF_LIB: &str = "fn callee() {\n    let v = 2;\n}\n\nfn caller() {\n    callee();\n}\n";

/// Seed the diff-blast fixture: write `src/lib.rs` on disk and an index whose
/// `blake3` + worktree-derived spans match it (the live-index invariant the
/// watcher upholds, tier-08), with `caller` referencing `callee`. Returns the
/// committed revision. Mirrors the cold golden's `seed_diff_fixture`.
fn seed_diff_fixture(root: &std::path::Path) -> u64 {
    std::fs::create_dir_all(root.join("src")).expect("mkdir src");
    std::fs::write(root.join("src/lib.rs"), DIFF_LIB).expect("write worktree lib");

    let seed_end = u32::try_from(DIFF_LIB.find('}').expect("callee brace") + 1).expect("fits u32");
    let dep_start =
        u32::try_from(DIFF_LIB.find("fn caller").expect("caller decl")).expect("fits u32");
    let dep_end = u32::try_from(DIFF_LIB.rfind('}').expect("caller brace") + 1).expect("fits u32");
    let blake = *blake3::hash(DIFF_LIB.as_bytes()).as_bytes();
    let function = |name: &str, byte_start: u32, byte_end: u32| SymbolRecord {
        canonical_name: name.into(),
        kind: "function".into(),
        defining_file: fid(1),
        defining_span: Span {
            file: fid(1),
            byte_start,
            byte_end,
        },
        visibility: Visibility::Unknown,
        attributes: Vec::new(),
        complexity: 0,
    };

    let mut cs = Changeset::new();
    cs = cs.upsert_file(
        fid(1),
        FileRecord {
            path: "src/lib.rs".into(),
            lang: Lang::Rust,
            size: 128,
            blake3: blake,
            mtime_ns: 1,
        },
    );
    cs = cs.upsert_symbol(sid(1), function("crate::callee", 0, seed_end));
    cs = cs.upsert_symbol(sid(2), function("crate::caller", dep_start, dep_end));
    // `caller` references `callee`, so `blast_radius(callee)` reaches `caller`.
    cs = cs.add_edge(
        EdgeKey {
            src: sid(2),
            kind: EdgeKind::References,
            dst: sid(1),
        },
        EdgeRecord {
            source_span: Span {
                file: fid(1),
                byte_start: dep_start,
                byte_end: dep_end,
            },
            evidence_lang: Lang::Rust,
            weight: 1,
        },
    );
    apply(root, &cs)
}

/// `diff_blast` over the warm socket resolves an uncommitted line edit to its
/// changed-symbol seed and folds in the seed's blast radius — exercising the
/// warm `impact::diff_blast` + `collect_span_sources` directly (the cold golden
/// + protocol parity test alone never run the warm handler; tier-15c audit F1).
///
/// The daemon's hash-guarded span build reads the worktree bytes and the line-2
/// hunk resolves to `callee` (lines 1-3), whose radius reaches its caller. The
/// git diff is the MCP root's job (RD7); the daemon receives the hunks over the
/// wire.
#[test]
fn diff_blast_resolves_changed_seed_over_warm_socket() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().to_path_buf();
    let revision = seed_diff_fixture(&root);
    let reference = cold(&root);

    let handle = spawn(&root);
    let response = query(
        &root,
        revision,
        DaemonQuery::DiffBlast {
            hunks: vec![LineHunk {
                path: "src/lib.rs".to_owned(),
                start_line: 2,
                end_line: 2,
            }],
            changed_paths: vec!["src/lib.rs".to_owned()],
            depth: None,
            kinds: None,
        },
    );
    shutdown(&root, handle);

    let seed_sym = reference.summ("crate::callee");
    let dependent = reference.summ("crate::caller");
    assert_eq!(
        response,
        DaemonResponse::DiffBlast(DiffBlastReport {
            seeds: vec![DiffSeed {
                symbol: seed_sym,
                must_touch: vec![dependent.clone()],
                may_touch: vec![],
                depth_used: 1,
            }],
            must_touch: vec![dependent],
            may_touch: vec![],
            unresolved: vec![],
        }),
    );
}
