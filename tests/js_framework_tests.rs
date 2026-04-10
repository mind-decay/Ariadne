//! Integration tests for Phase 13a: JS/TS framework support.
//!
//! Tests bundler alias resolution, Next.js route discovery, and React
//! context boundary extraction using fixture projects.

mod helpers;

use std::path::PathBuf;

use ariadne_graph::parser::ParserRegistry;
use ariadne_graph::pipeline::{BuildOptions, BuildPipeline, FsReader, FsWalker, WalkConfig};
use ariadne_graph::serial::json::JsonSerializer;
use ariadne_graph::serial::{BoundaryOutput, GraphReader};

fn fixture_dir(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

fn make_pipeline() -> BuildPipeline {
    BuildPipeline::new(
        Box::new(FsWalker::new()),
        Box::new(FsReader::new()),
        ParserRegistry::with_tier1(),
        Box::new(JsonSerializer),
    )
}

/// Build a fixture project to a tempdir and return the graph edges.
fn build_and_read_edges(fixture: &str) -> Vec<(String, String)> {
    let output_dir = tempfile::tempdir().expect("create tempdir");
    let pipeline = make_pipeline();

    pipeline
        .run_with_options(
            &fixture_dir(fixture),
            WalkConfig::default(),
            &BuildOptions {
                output_dir: Some(output_dir.path()),
                ..Default::default()
            },
        )
        .unwrap_or_else(|e| panic!("build failed for fixture '{}': {}", fixture, e));

    let reader = JsonSerializer;
    let graph = reader.read_graph(output_dir.path()).expect("read graph");
    graph
        .edges
        .iter()
        .map(|e| (e.0.clone(), e.1.clone()))
        .collect()
}

/// Build a fixture project and return the boundary output.
fn build_and_read_boundaries(fixture: &str) -> BoundaryOutput {
    let output_dir = tempfile::tempdir().expect("create tempdir");
    let pipeline = make_pipeline();

    pipeline
        .run_with_options(
            &fixture_dir(fixture),
            WalkConfig::default(),
            &BuildOptions {
                output_dir: Some(output_dir.path()),
                ..Default::default()
            },
        )
        .unwrap_or_else(|e| panic!("build failed for fixture '{}': {}", fixture, e));

    let reader = JsonSerializer;
    reader
        .read_boundaries(output_dir.path())
        .expect("read boundaries")
        .expect("boundaries file should exist")
}

// ---------------------------------------------------------------------------
// Vite alias resolution (SC-4, SC-5, SC-6)
// ---------------------------------------------------------------------------

#[test]
fn vite_alias_resolves_at_sign() {
    let edges = build_and_read_edges("react-vite");

    assert!(
        edges.iter().any(|(from, to)| from == "src/App.tsx" && to == "src/utils/format.ts"),
        "App.tsx → src/utils/format.ts edge should exist via @ alias; edges from App.tsx: {:?}",
        edges.iter().filter(|(f, _)| f == "src/App.tsx").collect::<Vec<_>>()
    );
}

#[test]
fn vite_alias_resolves_named() {
    let edges = build_and_read_edges("react-vite");

    assert!(
        edges.iter().any(|(from, to)| from == "src/App.tsx" && to == "src/components/Button.tsx"),
        "App.tsx → src/components/Button.tsx edge should exist via @components alias; edges from App.tsx: {:?}",
        edges.iter().filter(|(f, _)| f == "src/App.tsx").collect::<Vec<_>>()
    );
}

// ---------------------------------------------------------------------------
// React context boundaries (SC-25)
// ---------------------------------------------------------------------------

#[test]
fn react_context_boundary_in_vite_fixture() {
    let bo = build_and_read_boundaries("react-vite");

    // Find Context:ThemeContext in any file's boundaries
    let has_theme_context = bo.boundaries.values().any(|entries| {
        entries
            .iter()
            .any(|e| e.name == "Context:ThemeContext" && e.role == "Producer")
    });

    assert!(
        has_theme_context,
        "should detect Context:ThemeContext producer boundary; boundaries: {:?}",
        bo.boundaries.keys().collect::<Vec<_>>()
    );
}

// ---------------------------------------------------------------------------
// Next.js boundaries (SC-26, SC-27)
// ---------------------------------------------------------------------------

#[test]
fn nextjs_page_boundary() {
    let bo = build_and_read_boundaries("nextjs-app");

    let has_dashboard = bo.boundaries.values().any(|entries| {
        entries
            .iter()
            .any(|e| e.kind == "HttpRoute" && e.name == "/dashboard")
    });

    assert!(
        has_dashboard,
        "should extract /dashboard HttpRoute boundary; files: {:?}",
        bo.boundaries.keys().collect::<Vec<_>>()
    );
}

#[test]
fn nextjs_api_route_boundary() {
    let bo = build_and_read_boundaries("nextjs-app");

    let has_api = bo.boundaries.values().any(|entries| {
        entries
            .iter()
            .any(|e| e.kind == "HttpRoute" && e.name == "API:/api/users")
    });

    assert!(
        has_api,
        "should extract API:/api/users HttpRoute boundary; files: {:?}",
        bo.boundaries.keys().collect::<Vec<_>>()
    );
}

#[test]
fn nextjs_client_boundary() {
    let bo = build_and_read_boundaries("nextjs-app");

    let has_client = bo
        .boundaries
        .get("app/dashboard/page.tsx")
        .map(|entries| entries.iter().any(|e| e.name == "ClientBoundary"))
        .unwrap_or(false);

    assert!(
        has_client,
        "dashboard/page.tsx should have ClientBoundary; its boundaries: {:?}",
        bo.boundaries.get("app/dashboard/page.tsx")
    );
}

// ---------------------------------------------------------------------------
// No regression (SC-12)
// ---------------------------------------------------------------------------

#[test]
fn no_bundler_config_no_regression() {
    let edges = build_and_read_edges("mixed-project");
    assert!(
        !edges.is_empty(),
        "mixed-project should still produce edges without bundler config"
    );
}
