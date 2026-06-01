//! tier-07 warm-graph parity: architecture-health + documentation queries
//! and the staleness-refresh handshake. Each daemon response equals the v1
//! cold-path result (computed via `ariadne-graph`) for the same fixture
//! [src: .claude/plans/post-v1-roadmap/tier-07-daemon-warm-graph.md
//! steps 6, 7].

mod support;

use ariadne_core::{
    Changeset, CouplingReport, CouplingRow, CycleRow, DaemonQuery, DaemonRequest, DaemonResponse,
    DocForReport, DocReport, FileId, FileRecord, Lang, PlanAssistReport, PlanFileRow,
    ProjectStatusReport, Span, SymbolId, SymbolRecord, SymbolSummary, Visibility,
};

use support::{
    apply, canonical_changeset, cold, cold_doc_module, cold_doc_project, cold_refactor, seed,
    shutdown, spawn,
};

fn query(root: &std::path::Path, revision: u64, query: DaemonQuery) -> DaemonResponse {
    ariadne_daemon::query(root, &DaemonRequest { revision, query }).expect("query")
}

/// `plan_assist` over the warm socket equals the cold `ariadne-graph` ranked
/// file list, with `FileId` rows resolved back to paths.
#[test]
fn plan_assist_matches_cold() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().to_path_buf();
    let revision = seed(&root);
    let reference = cold(&root);

    let target = reference.id("crate::util::helper");
    let file_of = |s: SymbolId| reference.sym_file.get(&s).copied();
    let plan = reference.graph.plan_assist(target, 16, &file_of);
    let expect: Vec<PlanFileRow> = plan
        .files
        .iter()
        .map(|row| {
            let mut why: Vec<String> = row
                .why
                .iter()
                .filter_map(|s| reference.summary.get(s).map(|m| m.name.clone()))
                .collect();
            why.sort();
            PlanFileRow {
                file: reference.paths[&row.file].clone(),
                why,
                certainty: row.certainty,
            }
        })
        .collect();
    assert_eq!(expect.len(), 3, "three files implicated for this fixture");

    let handle = spawn(&root);
    let response = query(
        &root,
        revision,
        DaemonQuery::PlanAssist {
            symbol: "crate::util::helper".to_owned(),
            max_files: None,
        },
    );
    shutdown(&root, handle);

    assert_eq!(
        response,
        DaemonResponse::PlanAssist(PlanAssistReport { files: expect })
    );
}

/// `coupling_report` equals the cold per-file Martin metrics.
#[test]
fn coupling_report_matches_cold() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().to_path_buf();
    let revision = seed(&root);
    let reference = cold(&root);

    let modules = reference.modules(None);
    let expect: Vec<CouplingRow> = reference
        .graph
        .coupling_report(&modules)
        .rows
        .iter()
        .map(|m| CouplingRow {
            module: m.name.clone(),
            afferent: m.afferent,
            efferent: m.efferent,
            instability: m.instability,
            abstractness: m.abstractness,
            distance: m.distance,
        })
        .collect();
    assert_eq!(expect.len(), 4, "one module per source file");

    let handle = spawn(&root);
    let response = query(
        &root,
        revision,
        DaemonQuery::CouplingReport { prefix: None },
    );
    shutdown(&root, handle);

    assert_eq!(
        response,
        DaemonResponse::Coupling(CouplingReport { rows: expect })
    );
}

/// `weak_spots` surfaces the single cycle and the lone dead symbol, with the
/// `main` root exempt from the dead-code pass (tier-05 RD4).
#[test]
fn weak_spots_matches_cold() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().to_path_buf();
    let revision = seed(&root);
    let reference = cold(&root);

    let handle = spawn(&root);
    let response = query(&root, revision, DaemonQuery::WeakSpots { prefix: None });
    shutdown(&root, handle);

    let DaemonResponse::WeakSpots(report) = response else {
        panic!("expected WeakSpots, got {response:?}");
    };
    assert_eq!(
        report.cycles,
        vec![CycleRow {
            members: vec![
                "crate::helper::extra".to_owned(),
                "crate::util::helper".to_owned(),
            ],
        }],
    );
    assert!(report.god_modules.is_empty(), "fixture has no god modules");
    assert_eq!(
        report.dead_symbols,
        vec![reference.summ("crate::unused_helper")],
        "crate::main is an exempt root; only the orphan helper is dead",
    );
}

/// `doc_for` returns the structured single-symbol summary with first-hop
/// callers as public refs.
#[test]
fn doc_for_matches_cold() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().to_path_buf();
    let revision = seed(&root);
    let reference = cold(&root);

    let handle = spawn(&root);
    let response = query(
        &root,
        revision,
        DaemonQuery::DocFor {
            symbol: "crate::run".to_owned(),
        },
    );
    shutdown(&root, handle);

    assert_eq!(
        response,
        DaemonResponse::DocFor(DocForReport {
            signature: "function crate::run".to_owned(),
            kind: "function".to_owned(),
            file: "src/lib.rs".to_owned(),
            brief: "crate::run".to_owned(),
            public_refs: vec![reference.summ("crate::main")],
        }),
    );
}

/// `doc_for_module` / `doc_for_project` markdown is byte-identical to the cold
/// `ariadne-graph::docgen` render over the warm snapshot mirror.
#[test]
fn doc_markdown_matches_cold() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().to_path_buf();
    let revision = seed(&root);
    let module_md = cold_doc_module(&root, "src/lib.rs").expect("module render");
    let project_md = cold_doc_project(&root, None);

    let handle = spawn(&root);
    let module = query(
        &root,
        revision,
        DaemonQuery::DocForModule {
            path: "src/lib.rs".to_owned(),
        },
    );
    let project = query(&root, revision, DaemonQuery::DocForProject { prefix: None });
    shutdown(&root, handle);

    assert_eq!(
        module,
        DaemonResponse::Doc(DocReport {
            markdown: module_md
        })
    );
    assert_eq!(
        project,
        DaemonResponse::Doc(DocReport {
            markdown: project_md
        })
    );
}

/// A request carrying a redb revision newer than the warm graph was built
/// from triggers a refresh before the reply; a request at the built revision
/// does not (exit criterion / risk R-B2).
#[test]
fn stale_revision_triggers_refresh() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().to_path_buf();
    let built = seed(&root);

    let handle = spawn(&root);

    // At the built revision the daemon does not refresh: the not-yet-written
    // symbol is absent.
    let before = query(
        &root,
        built,
        DaemonQuery::FindDefinition {
            symbol: "crate::added_later".to_owned(),
        },
    );
    assert!(
        matches!(before, DaemonResponse::Error(_)),
        "symbol absent before the index advances",
    );

    // Advance the on-disk index with a new symbol (daemon is idle, so the
    // single-open redb file is free).
    let advanced = apply(&root, &add_symbol_changeset());
    assert!(advanced > built, "applying a changeset bumps the revision");

    // A request carrying the newer revision triggers a refresh; the symbol
    // is now resolvable.
    let after = query(
        &root,
        advanced,
        DaemonQuery::FindDefinition {
            symbol: "crate::added_later".to_owned(),
        },
    );
    shutdown(&root, handle);

    assert_eq!(
        after,
        DaemonResponse::Definition(SymbolSummary {
            id: 99,
            name: "crate::added_later".to_owned(),
            kind: "function".to_owned(),
            file: "src/lib.rs".to_owned(),
            byte_start: 0,
            byte_end: 32,
        }),
    );
}

/// `project_status` reports the warm graph's revision and coarse counts, plus
/// the project root the daemon was launched against.
#[test]
fn project_status_matches_cold() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().to_path_buf();
    let revision = seed(&root);
    let reference = cold(&root);

    let handle = spawn(&root);
    let response = query(&root, revision, DaemonQuery::ProjectStatus);
    shutdown(&root, handle);

    assert_eq!(
        response,
        DaemonResponse::ProjectStatus(ProjectStatusReport {
            revision,
            file_count: u32::try_from(reference.paths.len()).expect("fits u32"),
            symbol_count: u32::try_from(reference.summary.len()).expect("fits u32"),
            edge_count: u32::try_from(reference.graph.edge_count()).expect("fits u32"),
            root: root.display().to_string(),
        }),
    );
}

/// `refactor_suggestions` equals the cold `ariadne-graph::refactor` output:
/// the fixture's one cycle yields break proposals; no god modules clear the
/// 8.0 efferent threshold on this tiny graph.
#[test]
fn refactor_suggestions_matches_cold() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().to_path_buf();
    let revision = seed(&root);
    let expect = cold_refactor(&root, None);
    assert!(
        !expect.cycle_breaks.is_empty(),
        "fixture's helper/extra cycle yields break proposals",
    );

    let handle = spawn(&root);
    let response = query(
        &root,
        revision,
        DaemonQuery::RefactorSuggestions { prefix: None },
    );
    shutdown(&root, handle);

    assert_eq!(response, DaemonResponse::Refactor(expect));
}

/// A refresh failure (the on-disk index gone unreadable) surfaces as a typed
/// `DaemonResponse::Error`, not a dropped connection, and the daemon keeps
/// serving its last-good warm graph (tier-07 audit F3).
#[test]
fn refresh_failure_returns_typed_error() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().to_path_buf();
    let built = seed(&root);
    let expect_run = cold(&root).summ("crate::run");

    let handle = spawn(&root);

    // Corrupt the on-disk index so the next refresh cannot open it. The daemon
    // is idle here, so the single-open redb file is free to overwrite.
    let index = root.join(".ariadne").join("index.redb");
    std::fs::write(&index, [0xffu8; 4096]).expect("corrupt index");

    // A request carrying a newer revision triggers a refresh, which fails. The
    // transport call still succeeds — the failure is a typed response frame,
    // not a dropped connection.
    let failed = ariadne_daemon::query(
        &root,
        &DaemonRequest {
            revision: built + 1,
            query: DaemonQuery::FindDefinition {
                symbol: "crate::run".to_owned(),
            },
        },
    )
    .expect("transport succeeds: a refresh failure is a typed response, not a drop");
    assert!(
        matches!(failed, DaemonResponse::Error(_)),
        "refresh failure is a typed query-level error, got {failed:?}",
    );

    // The daemon stayed alive on its last-good warm graph: a non-stale request
    // is still answered from RAM, with no reopen of the corrupt index.
    let alive = query(
        &root,
        built,
        DaemonQuery::FindDefinition {
            symbol: "crate::run".to_owned(),
        },
    );

    shutdown(&root, handle);

    assert_eq!(alive, DaemonResponse::Definition(expect_run));
}

/// Canonical fixture plus one new symbol in `src/lib.rs`, applied as a second
/// commit to advance the revision past the daemon's build point.
fn add_symbol_changeset() -> Changeset {
    let mut cs = canonical_changeset();
    cs = cs.upsert_file(
        FileId::new(2).expect("nonzero"),
        FileRecord {
            path: "src/lib.rs".into(),
            lang: Lang::Rust,
            size: 128,
            blake3: [2u8; 32],
            mtime_ns: 2,
        },
    );
    cs.upsert_symbol(
        SymbolId::new(99).expect("nonzero"),
        SymbolRecord {
            canonical_name: "crate::added_later".into(),
            kind: "function".into(),
            defining_file: FileId::new(2).expect("nonzero"),
            defining_span: Span {
                file: FileId::new(2).expect("nonzero"),
                byte_start: 0,
                byte_end: 32,
            },
            visibility: Visibility::Unknown,
            attributes: Vec::new(),
            complexity: 0,
        },
    )
}
