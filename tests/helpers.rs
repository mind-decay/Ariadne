use std::path::PathBuf;

use ariadne_graph::parser::ParserRegistry;
use ariadne_graph::pipeline::{BuildOutput, BuildPipeline, FsReader, FsWalker, WalkConfig};
use ariadne_graph::serial::json::JsonSerializer;

/// Returns the absolute path to a named test fixture directory.
pub fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

/// Builds a fixture project through the full pipeline and returns the output.
/// Uses a tempdir for output to avoid cross-test races.
pub fn build_fixture(name: &str) -> BuildOutput {
    let path = fixture_path(name);
    let output_dir = tempfile::tempdir().expect("create tempdir");
    let output_path = output_dir.keep();
    let pipeline = BuildPipeline::new(
        Box::new(FsWalker::new()),
        Box::new(FsReader::new()),
        ParserRegistry::with_tier1(),
        Box::new(JsonSerializer),
    );
    pipeline
        .run_with_output(
            &path,
            WalkConfig::default(),
            Some(&output_path),
            false,
            false,
            false,
        )
        .unwrap_or_else(|e| panic!("build failed for fixture '{}': {}", name, e))
}

/// Builds a fixture and returns the raw graph.json content as a string.
/// (allow dead_code: helpers.rs is compiled per test binary — not all binaries use this)
#[allow(dead_code)]
pub fn build_and_read_graph_json(name: &str) -> String {
    let output = build_fixture(name);
    std::fs::read_to_string(&output.graph_path)
        .unwrap_or_else(|e| panic!("failed to read graph.json for fixture '{}': {}", name, e))
}
