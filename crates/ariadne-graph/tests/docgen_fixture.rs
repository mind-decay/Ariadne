//! Tier-09 steps 1 + 8 — golden Markdown for `docgen::for_module` /
//! `docgen::for_project` on the fixture repo, the empty-project negative
//! case, and the insertion-order determinism proptest.
//!
//! Insta review: `cargo insta review -p ariadne-graph`.

mod support;

use ariadne_core::FileChurn;
use ariadne_graph::{DocScope, GraphIndex, docgen};
use proptest::prelude::*;

/// Populated per-file churn so the module-doc golden exercises the risk line.
fn churn() -> Vec<FileChurn> {
    ["src/core.rs", "src/db.rs", "src/api.rs"]
        .iter()
        .map(|p| FileChurn {
            path: (*p).to_owned(),
            commits: 10,
            author_keys: Vec::new(),
            last_changed_ns: 0,
        })
        .collect()
}

#[test]
fn golden_module_doc_core() {
    let fx = support::core_fixture();
    let core = support::module_named(&fx.modules, "core");
    let md = docgen::for_module(
        &fx.graph,
        &fx.snapshot,
        core,
        &churn(),
        &DocScope::default(),
    )
    .expect("for_module");
    insta::assert_snapshot!("module_core", md);
}

#[test]
fn golden_project_doc() {
    let fx = support::core_fixture();
    let md = docgen::for_project(
        &fx.graph,
        &fx.snapshot,
        &fx.modules,
        &[],
        &[],
        &DocScope::default(),
    )
    .expect("for_project");
    insta::assert_snapshot!("project", md);
}

#[test]
fn empty_project_has_no_modules_placeholder() {
    let graph = GraphIndex::new();
    let snap = support::empty_snapshot();
    let md = docgen::for_project(&graph, &snap, &[], &[], &[], &DocScope::default())
        .expect("for_project on empty project");
    assert!(
        md.contains("_No modules indexed._"),
        "empty project must render a placeholder, got:\n{md}"
    );
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    /// Tier-09 step 8: `for_module` renders byte-identical output
    /// regardless of the graph's symbol/edge insertion order.
    #[test]
    fn module_doc_insertion_order_independent(seed in any::<u64>()) {
        let reference = {
            let fx = support::core_fixture();
            docgen::for_module(
                &fx.graph,
                &fx.snapshot,
                support::module_named(&fx.modules, "core"),
                &[],
                &DocScope::default(),
            )
            .expect("reference render")
        };
        let graph = support::shuffled_graph(seed);
        let snap = support::snapshot();
        let modules = support::modules();
        let shuffled = docgen::for_module(
            &graph,
            &snap,
            support::module_named(&modules, "core"),
            &[],
            &DocScope::default(),
        )
        .expect("shuffled render");
        prop_assert_eq!(reference, shuffled);
    }

    /// Tier-09 step 8 (audit F2): `for_project` — the extra
    /// `render_layers` SCC-condensation path — renders byte-identical
    /// output regardless of the graph's symbol/edge insertion order.
    #[test]
    fn project_doc_insertion_order_independent(seed in any::<u64>()) {
        let reference = {
            let fx = support::core_fixture();
            docgen::for_project(&fx.graph, &fx.snapshot, &fx.modules, &[], &[], &DocScope::default())
                .expect("reference render")
        };
        let graph = support::shuffled_graph(seed);
        let snap = support::snapshot();
        let modules = support::modules();
        let shuffled = docgen::for_project(&graph, &snap, &modules, &[], &[], &DocScope::default())
            .expect("shuffled render");
        prop_assert_eq!(reference, shuffled);
    }
}
