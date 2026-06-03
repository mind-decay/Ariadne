//! Tier-04 — `docgen::for_module` insight-section redesign and the
//! `module_svg` neighbourhood emitter.
//!
//! Asserts the module doc emits the six insight headers (Role, Neighbourhood,
//! Coupling, Cycles, Dead code, Risk), references a sidecar neighbourhood SVG,
//! reports a crate-aware role, scope-filters the coupling tables so non-source
//! neighbours drop out, degrades the risk line to an explicit "history
//! unavailable" when the Git-history vector is empty, and that `module_svg`
//! is byte-identical across two calls.

mod support;

use ariadne_core::FileChurn;
use ariadne_graph::{DocScope, docgen, module_svg};

/// One commit-rich churn row per fixture source file. `src/core.rs` is the
/// `core` module's defining file, so its risk line is populated.
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
fn module_doc_emits_insight_headers() {
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

    for header in [
        "## Role",
        "## Neighbourhood",
        "## Coupling",
        "## Cycles",
        "## Dead code",
        "## Risk",
    ] {
        assert!(md.contains(header), "missing section {header}, got:\n{md}");
    }

    // Neighbourhood references a sidecar SVG (D4), not an inline `<svg>` block.
    assert!(
        md.contains("![neighbourhood](") && md.contains(".svg)"),
        "missing neighbourhood SVG reference, got:\n{md}"
    );
    assert!(
        !md.contains("<svg"),
        "module doc must reference the SVG, not inline it, got:\n{md}"
    );

    // Crate-aware role names the owning crate + hexagonal layer.
    assert!(
        md.contains("crate `") && md.contains("layer."),
        "role line lacks crate / layer, got:\n{md}"
    );
}

#[test]
fn coupling_excludes_out_of_scope_neighbours() {
    let fx = support::core_fixture();
    let core = support::module_named(&fx.modules, "core");

    // Default scope keeps every (source) neighbour: `api::serve` (src/api.rs)
    // is an inbound caller of `core`.
    let default_md = docgen::for_module(
        &fx.graph,
        &fx.snapshot,
        core,
        &churn(),
        &DocScope::default(),
    )
    .expect("for_module default scope");
    assert!(
        default_md.contains("`api::serve`"),
        "default scope must keep the source caller, got:\n{default_md}"
    );

    // An extra exclude on `api.rs` drops that neighbour from the coupling
    // table while keeping the in-scope `db::connect` (src/db.rs) caller.
    let scope = DocScope {
        extra_excludes: vec!["api.rs".to_owned()],
    };
    let scoped_md = docgen::for_module(&fx.graph, &fx.snapshot, core, &churn(), &scope)
        .expect("for_module scoped");
    assert!(
        !scoped_md.contains("`api::serve`"),
        "excluded neighbour must drop from coupling, got:\n{scoped_md}"
    );
    assert!(
        scoped_md.contains("`db::connect`"),
        "in-scope neighbour must remain in coupling, got:\n{scoped_md}"
    );
}

#[test]
fn empty_history_degrades_risk_to_explicit_line() {
    let fx = support::core_fixture();
    let core = support::module_named(&fx.modules, "core");
    let md = docgen::for_module(&fx.graph, &fx.snapshot, core, &[], &DocScope::default())
        .expect("for_module");
    assert!(
        md.contains("history unavailable"),
        "empty churn must emit an explicit history-unavailable risk line, got:\n{md}"
    );
}

#[test]
fn module_svg_is_deterministic_and_well_formed() {
    let fx = support::core_fixture();
    let core = support::module_named(&fx.modules, "core");
    let first =
        module_svg(&fx.graph, &fx.snapshot, core, &DocScope::default()).expect("module_svg");
    let second =
        module_svg(&fx.graph, &fx.snapshot, core, &DocScope::default()).expect("module_svg");
    assert_eq!(first, second, "module_svg must be byte-identical");
    assert!(
        first.starts_with("<svg viewBox="),
        "not an SVG root:\n{first}"
    );
    assert!(
        first.trim_end().ends_with("</svg>"),
        "unclosed SVG:\n{first}"
    );
    // The module node and at least one neighbour edge are drawn.
    assert!(
        first.contains(">core<"),
        "missing module node, got:\n{first}"
    );
    assert!(
        first.contains("<line "),
        "missing neighbour edge, got:\n{first}"
    );
}

#[test]
fn module_svg_excludes_out_of_scope_neighbours() {
    let fx = support::core_fixture();
    let core = support::module_named(&fx.modules, "core");
    // `api::serve` (sid 4, src/api.rs) is an inbound caller of `core`, drawn as
    // the neighbour node `#4` under the default (source-only) scope.
    let default_svg =
        module_svg(&fx.graph, &fx.snapshot, core, &DocScope::default()).expect("module_svg");
    assert!(
        default_svg.contains(">#4<"),
        "default scope must draw the api caller node, got:\n{default_svg}"
    );
    // Excluding `api.rs` drops that neighbour from the SVG too, matching the
    // scope-filtered coupling table (INFO-1 / D3).
    let scope = DocScope {
        extra_excludes: vec!["api.rs".to_owned()],
    };
    let scoped_svg = module_svg(&fx.graph, &fx.snapshot, core, &scope).expect("module_svg");
    assert!(
        !scoped_svg.contains(">#4<"),
        "excluded neighbour must drop from the SVG, got:\n{scoped_svg}"
    );
}

#[test]
fn for_module_is_deterministic() {
    let fx = support::core_fixture();
    let core = support::module_named(&fx.modules, "core");
    let render = || {
        docgen::for_module(
            &fx.graph,
            &fx.snapshot,
            core,
            &churn(),
            &DocScope::default(),
        )
        .expect("for_module")
    };
    assert_eq!(render(), render(), "for_module must be byte-identical");
}
