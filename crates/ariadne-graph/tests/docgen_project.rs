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

use std::collections::BTreeSet;

use ariadne_core::{CoChangePair, FileChurn};
use ariadne_graph::{DocScope, EdgeKind, GraphIndex, ModuleSpec, architecture_svg, docgen};

/// One commit-rich churn row per fixture source file plus an out-of-scope
/// fixture path that must be filtered from the risk ranking. Paths match the
/// `crates/` fixture so the structural-edge / scope checks resolve.
fn churn() -> Vec<FileChurn> {
    [
        "crates/ariadne-core/src/core.rs",
        "crates/ariadne-core/src/db.rs",
        "crates/ariadne-salsa/src/types.rs",
        "crates/ariadne-storage/src/adapters/redb/util.rs",
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
            a: "crates/ariadne-core/src/core.rs".to_owned(),
            b: "crates/ariadne-core/src/db.rs".to_owned(),
            count: 5,
        },
        CoChangePair {
            a: "crates/ariadne-salsa/src/types.rs".to_owned(),
            b: "crates/ariadne-storage/src/adapters/redb/util.rs".to_owned(),
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
        md.contains("crates/ariadne-core/src/core.rs"),
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
        md.contains(
            "`crates/ariadne-salsa/src/types.rs` ⇄ \
`crates/ariadne-storage/src/adapters/redb/util.rs`"
        ),
        "hidden change-coupling pair missing, got:\n{md}"
    );
    assert!(
        !md.contains("`crates/ariadne-core/src/core.rs` ⇄ `crates/ariadne-core/src/db.rs`"),
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
        first.contains(">ariadne-core<"),
        "missing crate node, got:\n{first}"
    );
}

/// Tier-01 fixture: modules named by real crate paths spanning the
/// domain-interior crates (core/graph/salsa), an adapter crate, an interior
/// crate, a `tools/` dir, an out-of-scope test + fixture module, plus a
/// cross-crate 2-node cycle (`core_a` ⇄ `graph_b`) and an intra-crate 2-node
/// cycle (`storage_d` ⇄ `storage_d2`). Exercises the scope / layer / withhold
/// rules in `for_project` end-to-end.
fn crate_paths_fixture() -> (GraphIndex, support::MemSnapshot, Vec<ModuleSpec>) {
    const FILES: [(u32, &str); 9] = [
        (1, "crates/ariadne-core/src/lib.rs"),
        (2, "crates/ariadne-graph/src/docgen.rs"),
        (3, "crates/ariadne-salsa/src/db.rs"),
        (4, "crates/ariadne-storage/src/adapters/redb/apply.rs"),
        (5, "crates/ariadne-cli/src/main.rs"),
        (6, "tools/xtask/src/main.rs"),
        (7, "crates/ariadne-mcp/tests/handshake.rs"),
        (8, "crates/ariadne-parser/fixtures/x.js"),
        (9, "crates/ariadne-graph/src/build.rs"),
    ];
    const SYMS: [(u64, &str, &str, u32); 10] = [
        (1, "core_a", "function", 1),
        (2, "graph_b", "function", 2),
        (3, "salsa_c", "function", 3),
        (4, "storage_d", "function", 4),
        (5, "cli_e", "function", 5),
        (6, "tools_f", "function", 6),
        (7, "test_g", "function", 7),
        (8, "fixture_h", "function", 8),
        (9, "storage_d2", "function", 4),
        (10, "graph_z", "function", 9),
    ];

    let mut graph = GraphIndex::new();
    for &(id, ..) in &SYMS {
        graph.add_symbol(support::sid(id));
    }
    // Cross-crate cycle {core_a, graph_b}; intra-crate cycle {storage_d,
    // storage_d2}.
    for &(a, b) in &[(1u64, 2u64), (2, 1), (4, 9), (9, 4)] {
        graph.add_edge(support::sid(a), support::sid(b), EdgeKind::Calls);
    }

    let module = |name: &str, ids: &[u64]| ModuleSpec {
        name: name.to_owned(),
        members: ids.iter().map(|&i| support::sid(i)).collect(),
        abstract_members: BTreeSet::new(),
    };
    let modules = vec![
        module("crates/ariadne-core/src/lib.rs", &[1]),
        module("crates/ariadne-graph/src/docgen.rs", &[2]),
        module("crates/ariadne-salsa/src/db.rs", &[3]),
        module("crates/ariadne-storage/src/adapters/redb/apply.rs", &[4, 9]),
        module("crates/ariadne-cli/src/main.rs", &[5]),
        module("tools/xtask/src/main.rs", &[6]),
        module("crates/ariadne-mcp/tests/handshake.rs", &[7]),
        module("crates/ariadne-parser/fixtures/x.js", &[8]),
        module("crates/ariadne-graph/src/build.rs", &[10]),
    ];

    (graph, support::snapshot_from(&FILES, &SYMS), modules)
}

/// Churn for the crate-path fixture: two source files plus an out-of-scope
/// test and snapshot path, all commit-rich enough to clear the co-change
/// filters so scope filtering — not the threshold — is what drops them.
fn crate_paths_churn() -> Vec<FileChurn> {
    [
        "crates/ariadne-graph/src/build.rs",
        "crates/ariadne-graph/src/docgen.rs",
        "crates/ariadne-mcp/tests/handshake.rs",
        "crates/ariadne-mcp/tests/snapshots/handshake__tools_list.snap",
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

/// Co-change pairs for the crate-path fixture: one source⇄source hidden pair
/// (kept) and two pairs touching a `/tests/` and a `.snap` path (must be
/// dropped by source scope, never rendered).
fn crate_paths_co_change() -> Vec<CoChangePair> {
    let pair = |a: &str, b: &str| CoChangePair {
        a: a.to_owned(),
        b: b.to_owned(),
        count: 5,
    };
    vec![
        pair(
            "crates/ariadne-graph/src/build.rs",
            "crates/ariadne-graph/src/docgen.rs",
        ),
        pair(
            "crates/ariadne-graph/src/docgen.rs",
            "crates/ariadne-mcp/tests/handshake.rs",
        ),
        pair(
            "crates/ariadne-graph/src/build.rs",
            "crates/ariadne-mcp/tests/snapshots/handshake__tools_list.snap",
        ),
    ]
}

/// Render the crate-path fixture overview with populated history.
fn render_crate_paths() -> String {
    let (graph, snap, modules) = crate_paths_fixture();
    docgen::for_project(
        &graph,
        &snap,
        &modules,
        &crate_paths_churn(),
        &crate_paths_co_change(),
        &DocScope::default(),
    )
    .expect("for_project")
}

#[test]
fn overview_leaks_no_test_or_fixture_paths() {
    let md = render_crate_paths();
    for forbidden in ["/tests/", ".snap", "/fixtures/"] {
        assert!(
            !md.contains(forbidden),
            "non-source path substring {forbidden:?} leaked into the overview:\n{md}"
        );
    }
}

#[test]
fn architecture_rows_pin_each_crate_layer() {
    let md = render_crate_paths();
    // Domain-interior crates (flat `src/`) get the Domain override; the adapter
    // and interior crates fall through to the path heuristic.
    for row in [
        "| `ariadne-core` | Domain |",
        "| `ariadne-graph` | Domain |",
        "| `ariadne-salsa` | Domain |",
        "| `ariadne-storage` | Adapter |",
        "| `ariadne-cli` | Interior |",
    ] {
        assert!(
            md.contains(row),
            "missing architecture row {row:?}, got:\n{md}"
        );
    }
}

#[test]
fn synopsis_crate_count_excludes_tools_dir() {
    let md = render_crate_paths();
    // Five `crates/` crates are scoped (core, graph, salsa, storage, cli); the
    // in-scope `tools/xtask` module is not under `crates/` so it is excluded.
    assert!(
        md.contains("5 crate(s)"),
        "synopsis must count only `crates/` crates, got:\n{md}"
    );
}

#[test]
fn role_and_boundary_sections_are_withheld() {
    let md = render_crate_paths();
    assert!(
        md.contains("| _withheld (R1)_ |"),
        "Architecture Role cell must be withheld, got:\n{md}"
    );
    assert!(
        md.contains("Role withheld — depends on cross-crate edge accuracy (R1)."),
        "Architecture table must carry the Role-withheld note, got:\n{md}"
    );
    assert!(
        md.contains(
            "_Withheld — symbol-edge boundary checks depend on cross-crate edge accuracy (R1)"
        ),
        "Boundary violations must be withheld, got:\n{md}"
    );
}

#[test]
fn cross_crate_cycle_is_withheld_intra_crate_listed() {
    let md = render_crate_paths();
    // Intra-crate storage cluster is still listed with its members.
    assert!(
        md.contains("`storage_d`") && md.contains("`storage_d2`"),
        "intra-crate cycle must still be listed, got:\n{md}"
    );
    // Cross-crate {core_a, graph_b} cluster is withheld, never listed.
    assert!(
        md.contains("cross-crate dependency cluster(s) depend on cross-crate edge accuracy (R1)"),
        "cross-crate cycle cluster must be withheld, got:\n{md}"
    );
    assert!(
        !md.contains("graph_b"),
        "cross-crate cluster member must not be listed, got:\n{md}"
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
