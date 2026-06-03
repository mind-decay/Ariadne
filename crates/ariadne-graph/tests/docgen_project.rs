//! Tier-03 — `docgen::for_project` insight-section redesign and the
//! `architecture_svg` sidecar emitter.
//!
//! Asserts the project overview emits the six insight sections (Synopsis,
//! Architecture, Boundary violations, Cycle clusters, Risk hot-spots, Refactor
//! & change-coupling), references the sidecar SVG, ranks risk over
//! source-scoped files only, surfaces the largest SCC with a deterministic cut
//! edge, finds hidden change-coupling, and degrades to an explicit "history
//! unavailable" line when the Git-history vectors are empty. Determinism is
//! checked by rendering each surface twice and comparing bytes.

mod support;

use ariadne_core::{CoChangePair, FileChurn};
use ariadne_graph::{DocScope, architecture_svg, docgen};

/// One commit-rich churn row per fixture source file plus an out-of-scope
/// fixture path that must be filtered from the risk ranking.
fn churn() -> Vec<FileChurn> {
    [
        "src/core.rs",
        "src/db.rs",
        "src/types.rs",
        "src/util.rs",
        "x/fixtures/bar.rs",
    ]
    .iter()
    .map(|p| FileChurn {
        path: (*p).to_owned(),
        commits: 10,
        author_keys: Vec::new(),
        last_changed_ns: 0,
    })
    .collect()
}

/// `core.rs`/`db.rs` co-change but share a structural edge (`core::run` ->
/// `db::query`), so they are *not* hidden coupling; `types.rs`/`util.rs`
/// co-change with no structural edge, so they are.
fn co_change() -> Vec<CoChangePair> {
    vec![
        CoChangePair {
            a: "src/core.rs".to_owned(),
            b: "src/db.rs".to_owned(),
            count: 5,
        },
        CoChangePair {
            a: "src/types.rs".to_owned(),
            b: "src/util.rs".to_owned(),
            count: 5,
        },
    ]
}

#[test]
fn project_doc_emits_insight_sections() {
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

    for header in [
        "## Synopsis",
        "## Architecture",
        "## Boundary violations",
        "## Cycle clusters",
        "## Risk hot-spots",
        "## Refactor & change-coupling",
    ] {
        assert!(md.contains(header), "missing section {header}, got:\n{md}");
    }

    // Sidecar SVG reference (D4), not an inline Mermaid block.
    assert!(
        md.contains("![architecture](codebase-overview.svg)"),
        "missing sidecar SVG reference, got:\n{md}"
    );
    assert!(
        !md.contains("flowchart TD"),
        "Mermaid block must be gone, got:\n{md}"
    );
    assert!(
        !md.contains("| Module | Ca | Ce | I | A | Distance |"),
        "per-file Martin dump must be gone, got:\n{md}"
    );
}

#[test]
fn cycle_clusters_names_largest_scc_with_cut_edge() {
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

    // The fixture's only SCC is {core::run, db::query, db::connect}; the
    // lowest (src id, dst id) member edge is core::run -> db::query.
    assert!(md.contains("3 members"), "missing SCC size, got:\n{md}");
    assert!(
        md.contains("`core::run`") && md.contains("`db::query`"),
        "missing SCC members / cut edge, got:\n{md}"
    );
}

#[test]
fn empty_history_degrades_to_explicit_line() {
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
    assert!(
        md.contains("history unavailable"),
        "empty churn must emit an explicit history-unavailable line, got:\n{md}"
    );
}

#[test]
fn populated_history_ranks_scoped_risk_and_finds_hidden_coupling() {
    let fx = support::core_fixture();
    let md = docgen::for_project(
        &fx.graph,
        &fx.snapshot,
        &fx.modules,
        &churn(),
        &co_change(),
        &DocScope::default(),
    )
    .expect("for_project");

    // Risk hot-spots rank source-scoped files; the out-of-scope fixture path
    // is dropped even though it has the same churn.
    assert!(
        md.contains("src/core.rs"),
        "scoped source file missing from risk ranking, got:\n{md}"
    );
    assert!(
        !md.contains("x/fixtures/bar.rs"),
        "out-of-scope fixture file must not rank, got:\n{md}"
    );

    // Hidden coupling: types.rs <-> util.rs co-change with no structural edge.
    // The exact pair line proves detection, not a risk-table coincidence; the
    // edge-linked core.rs/db.rs pair must NOT appear as hidden coupling.
    assert!(
        md.contains("`src/types.rs` ⇄ `src/util.rs`"),
        "hidden change-coupling pair missing, got:\n{md}"
    );
    assert!(
        !md.contains("`src/core.rs` ⇄ `src/db.rs`"),
        "structurally-linked pair must not be flagged as hidden coupling, got:\n{md}"
    );
    // No more "history unavailable" once the vectors are populated.
    assert!(
        !md.contains("history unavailable"),
        "populated history must not emit the unavailable line, got:\n{md}"
    );
}

#[test]
fn architecture_svg_is_deterministic_and_well_formed() {
    let fx = support::core_fixture();
    let first = architecture_svg(&fx.graph, &fx.modules, &DocScope::default());
    let second = architecture_svg(&fx.graph, &fx.modules, &DocScope::default());
    assert_eq!(first, second, "architecture_svg must be byte-identical");
    assert!(
        first.starts_with("<svg viewBox="),
        "not an SVG root:\n{first}"
    );
    assert!(
        first.trim_end().ends_with("</svg>"),
        "unclosed SVG:\n{first}"
    );
    // Crate-aggregated nodes: the fixture's crate keys appear as labels.
    assert!(
        first.contains(">core<"),
        "missing crate node, got:\n{first}"
    );
}

#[test]
fn for_project_is_deterministic() {
    let fx = support::core_fixture();
    let render = || {
        docgen::for_project(
            &fx.graph,
            &fx.snapshot,
            &fx.modules,
            &churn(),
            &co_change(),
            &DocScope::default(),
        )
        .expect("for_project")
    };
    assert_eq!(render(), render(), "for_project must be byte-identical");
}
