mod helpers;

use ariadne_graph::diagnostic::{DiagnosticCollector, Warning, WarningCode};
use ariadne_graph::model::CanonicalPath;
use ariadne_graph::pipeline::{BuildPipeline, FsReader, FsWalker, WalkConfig};
use ariadne_graph::parser::ParserRegistry;
use ariadne_graph::serial::json::JsonSerializer;

// ---------------------------------------------------------------------------
// DiagnosticCollector tests
// ---------------------------------------------------------------------------

#[test]
fn diagnostic_collector_empty() {
    let collector = DiagnosticCollector::new();
    let report = collector.drain();
    assert!(report.warnings.is_empty(), "fresh collector should have no warnings");
    assert_eq!(report.counts.files_skipped, 0);
    assert_eq!(report.counts.imports_unresolved, 0);
    assert_eq!(report.counts.partial_parses, 0);
}

#[test]
fn diagnostic_collector_collects_and_sorts() {
    let collector = DiagnosticCollector::new();

    // Add warnings in non-sorted order
    collector.warn(Warning {
        code: WarningCode::W002ReadFailed,
        path: CanonicalPath::new("z/file.ts".to_string()),
        message: "cannot read".to_string(),
        detail: None,
    });
    collector.warn(Warning {
        code: WarningCode::W001ParseFailed,
        path: CanonicalPath::new("a/file.ts".to_string()),
        message: "parse failed".to_string(),
        detail: None,
    });
    collector.warn(Warning {
        code: WarningCode::W006ImportUnresolved,
        path: CanonicalPath::new("m/file.ts".to_string()),
        message: "unresolved".to_string(),
        detail: None,
    });

    let report = collector.drain();

    assert_eq!(report.warnings.len(), 3);
    assert_eq!(report.counts.files_skipped, 2); // W001 + W002
    assert_eq!(report.counts.imports_unresolved, 1); // W006

    // Verify sorted by path (a < m < z)
    let paths: Vec<&str> = report.warnings.iter().map(|w| w.path.as_str()).collect();
    assert_eq!(paths, vec!["a/file.ts", "m/file.ts", "z/file.ts"]);
}

#[test]
fn diagnostic_collector_increment_unresolved() {
    let collector = DiagnosticCollector::new();
    collector.increment_unresolved();
    collector.increment_unresolved();
    collector.increment_unresolved();

    let report = collector.drain();
    assert!(report.warnings.is_empty(), "increment_unresolved should not add warnings");
    assert_eq!(report.counts.imports_unresolved, 3);
}

// ---------------------------------------------------------------------------
// Pipeline error handling tests
// ---------------------------------------------------------------------------

fn make_pipeline() -> BuildPipeline {
    BuildPipeline::new(
        Box::new(FsWalker::new()),
        Box::new(FsReader::new()),
        ParserRegistry::with_tier1(),
        Box::new(JsonSerializer),
    )
}

/// E001: Build on a nonexistent path should return ProjectNotFound.
#[test]
fn pipeline_nonexistent_path_returns_e001() {
    let pipeline = make_pipeline();
    let result = pipeline.run(
        std::path::Path::new("/tmp/ariadne-test-nonexistent-path-that-does-not-exist"),
        WalkConfig::default(),
    );

    assert!(result.is_err(), "should fail on nonexistent path");
    let err = result.unwrap_err();
    let msg = format!("{}", err);
    assert!(
        msg.contains("E001"),
        "error should be E001 (ProjectNotFound), got: {}",
        msg
    );
}

/// E004: Build on an empty directory (no parseable files) should return NoParseableFiles.
#[test]
fn pipeline_empty_dir_returns_e004() {
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let pipeline = make_pipeline();
    let result = pipeline.run(temp_dir.path(), WalkConfig::default());

    assert!(result.is_err(), "should fail on empty directory");
    let err = result.unwrap_err();
    let msg = format!("{}", err);
    assert!(
        msg.contains("E004"),
        "error should be E004 (NoParseableFiles), got: {}",
        msg
    );
}

/// A valid fixture should produce output files on disk.
#[test]
fn pipeline_produces_output_files() {
    let output = helpers::build_fixture("typescript-app");

    assert!(
        output.graph_path.exists(),
        "graph.json should exist at {:?}",
        output.graph_path
    );
    assert!(
        output.clusters_path.exists(),
        "clusters.json should exist at {:?}",
        output.clusters_path
    );
}
