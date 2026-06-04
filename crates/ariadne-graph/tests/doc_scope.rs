//! Tier-01 — doc-layer source scoping. Golden `classify()` table, the
//! `Source`-only default `DocScope`, `crate_of` / `LayerHint` grouping,
//! and a `for_project` filter check that doubles as the graph-unmutated
//! proof (scope is a reporting filter, never a graph mutation).

mod support;

use std::collections::BTreeSet;

use ariadne_graph::doc_model::{DocKind, DocScope, LayerHint, classify, crate_of};
use ariadne_graph::{EdgeKind, GraphIndex, ModuleSpec, docgen};

#[test]
fn classify_buckets_paths_by_priority() {
    // Fixture wins over the `.js` extension and over a nested `/tests/`.
    assert_eq!(
        classify("crates/ariadne-parser/fixtures/javascript/jquery.js"),
        DocKind::Fixture
    );
    assert_eq!(
        classify("crates/ariadne-graph/tests/support.rs"),
        DocKind::Test
    );
    assert_eq!(
        classify("crates/ariadne-graph/src/docgen.rs"),
        DocKind::Source
    );
    assert_eq!(
        classify("web/node_modules/react/index.js"),
        DocKind::Vendored
    );
    assert_eq!(classify("assets/vendor/jquery.min.js"), DocKind::Vendored);
    assert_eq!(classify("target/debug/build/out.rs"), DocKind::Generated);
    assert_eq!(
        classify("crates/ariadne-scip/src/gen/scip.pb.rs"),
        DocKind::Generated
    );
    // Manifests, lock, and the `.claude/` plan tree are project metadata, not
    // first-party source — they classify as Config and drop out of scope.
    assert_eq!(classify("Cargo.toml"), DocKind::Config);
    assert_eq!(classify("Cargo.lock"), DocKind::Config);
    assert_eq!(
        classify(".claude/plans/post-v1-roadmap/audit-state.json"),
        DocKind::Config
    );
}

#[test]
fn config_paths_drop_out_of_default_scope() {
    let scope = DocScope::default();
    assert!(!scope.include("Cargo.toml"));
    assert!(!scope.include("Cargo.lock"));
    assert!(!scope.include(".claude/plans/post-v1-roadmap/audit-state.json"));
}

#[test]
fn default_scope_includes_only_source() {
    let scope = DocScope::default();
    assert!(scope.include("crates/ariadne-graph/src/docgen.rs"));
    assert!(!scope.include("crates/ariadne-parser/fixtures/javascript/jquery.js"));
    assert!(!scope.include("crates/ariadne-graph/tests/support.rs"));
    assert!(!scope.include("target/debug/build/out.rs"));
    assert!(!scope.include("web/node_modules/react/index.js"));
}

#[test]
fn extra_excludes_layer_atop_source() {
    let scope = DocScope {
        extra_excludes: vec!["generated".to_owned()],
    };
    assert!(scope.include("crates/ariadne-graph/src/docgen.rs"));
    assert!(!scope.include("crates/ariadne-graph/src/generated/wire.rs"));
}

#[test]
fn crate_of_groups_by_prefix() {
    assert_eq!(
        crate_of("crates/ariadne-mcp/src/server.rs"),
        Some("ariadne-mcp")
    );
    assert_eq!(
        crate_of("crates/ariadne-graph/src/lib.rs"),
        Some("ariadne-graph")
    );
    assert_eq!(crate_of("docs/architecture.md"), None);
}

#[test]
fn layer_hint_reads_path_segments() {
    assert_eq!(
        LayerHint::of("crates/ariadne-core/src/domain/types/ids.rs"),
        LayerHint::Domain
    );
    assert_eq!(
        LayerHint::of("crates/ariadne-cli/src/adapters/daemon_client.rs"),
        LayerHint::Adapter
    );
    assert_eq!(
        LayerHint::of("crates/ariadne-graph/src/lib.rs"),
        LayerHint::Interior
    );
}

#[test]
fn for_project_omits_fixtures_but_graph_keeps_them() {
    // Two-module graph: a Source module whose symbol calls into a Fixture
    // module's symbol, so the fixture symbol has a real graph fan-in.
    let src = support::sid(1);
    let jquery = support::sid(100);
    let mut graph = GraphIndex::new();
    graph.add_symbol(src);
    graph.add_symbol(jquery);
    graph.add_edge(src, jquery, EdgeKind::Calls);

    let modules = vec![
        ModuleSpec {
            name: "crates/ariadne-graph/src/docgen.rs".to_owned(),
            members: BTreeSet::from([src]),
            abstract_members: BTreeSet::new(),
        },
        ModuleSpec {
            name: "crates/ariadne-parser/fixtures/javascript/jquery.js".to_owned(),
            members: BTreeSet::from([jquery]),
            abstract_members: BTreeSet::new(),
        },
    ];
    let snap = support::empty_snapshot();
    let md = docgen::for_project(&graph, &snap, &modules, &[], &[], &DocScope::default())
        .expect("for_project");

    // tier-03 reports crates, not per-file paths: the source module's crate
    // (`ariadne-graph`) appears in the Architecture table.
    assert!(
        md.contains("ariadne-graph"),
        "source crate must appear in the doc:\n{md}"
    );
    assert!(
        !md.contains("jquery.js"),
        "fixture module must be filtered from every reported section:\n{md}"
    );
    // Scope is doc-layer only: the fixture symbol still resolves in the
    // graph (its inbound edge survives), proving the graph was not mutated.
    assert_eq!(
        graph.fan_in(jquery),
        1,
        "scoping must not mutate the graph; fixture symbol still has fan-in"
    );
}
