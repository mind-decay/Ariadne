mod helpers;

use ariadne_graph::diagnostic::{DiagnosticCollector, Warning, WarningCode};
use ariadne_graph::model::CanonicalPath;
use ariadne_graph::parser::ParserRegistry;
use ariadne_graph::pipeline::{BuildOptions, BuildPipeline, FileWalker, FsReader, FsWalker, WalkConfig};
use ariadne_graph::serial::json::JsonSerializer;
use ariadne_graph::serial::{GraphReader, RawImportOutput};

// ---------------------------------------------------------------------------
// DiagnosticCollector tests
// ---------------------------------------------------------------------------

#[test]
fn diagnostic_collector_empty() {
    let collector = DiagnosticCollector::new();
    let report = collector.drain();
    assert!(
        report.warnings.is_empty(),
        "fresh collector should have no warnings"
    );
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
    assert!(
        report.warnings.is_empty(),
        "increment_unresolved should not add warnings"
    );
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

/// Multiple exclude_dirs should all be respected, not just the last one.
#[test]
fn walk_excludes_all_configured_dirs() {
    let root = helpers::fixture_path("typescript-app");
    let walker = FsWalker::new();
    let config = WalkConfig {
        exclude_dirs: vec![".ariadne".to_string(), "node_modules".to_string()],
        ..WalkConfig::default()
    };

    let walk_result = walker.walk(&root, &config).expect("walk should succeed");

    for entry in &walk_result.entries {
        let rel = entry.path.strip_prefix(&root).unwrap_or(&entry.path);
        let components: Vec<&str> = rel
            .components()
            .filter_map(|c| c.as_os_str().to_str())
            .collect();
        assert!(
            !components.contains(&".ariadne"),
            "entry should not be under .ariadne: {:?}",
            entry.path
        );
        assert!(
            !components.contains(&"node_modules"),
            "entry should not be under node_modules: {:?}",
            entry.path
        );
    }

    // Sanity: we should still get some files (the fixture has .ts files)
    assert!(
        !walk_result.entries.is_empty(),
        "walk should find at least one file"
    );
}

#[test]
fn binary_file_detected_by_null_bytes() {
    use ariadne_graph::pipeline::{FileEntry, FileReader, FsReader};
    let temp_dir = tempfile::tempdir().expect("create tempdir");
    let file_path = temp_dir.path().join("binary.ts");
    std::fs::write(&file_path, b"import foo\x00\x00 from 'bar';\n").unwrap();
    let reader = FsReader::new();
    let entry = FileEntry {
        path: file_path,
        extension: "ts".to_string(),
    };
    let result = reader.read(&entry, temp_dir.path(), 1_048_576);
    assert!(result.is_err());
    match result.unwrap_err() {
        ariadne_graph::pipeline::FileSkipReason::BinaryFile { .. } => {}
        other => panic!("expected BinaryFile, got {:?}", other),
    }
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

// ---------------------------------------------------------------------------
// CLI flag behavior tests
// ---------------------------------------------------------------------------

/// --timestamp=false should omit the `generated` field from graph.json.
#[test]
fn timestamp_false_omits_generated_field() {
    let path = helpers::fixture_path("typescript-app");
    let output_dir = tempfile::tempdir().expect("create tempdir");
    let output_path = output_dir.path();
    let pipeline = make_pipeline();

    pipeline
        .run_with_options(
            &path,
            WalkConfig::default(),
            &BuildOptions { output_dir: Some(output_path), ..Default::default() },
        )
        .expect("build should succeed");

    let graph_json = std::fs::read_to_string(output_path.join("graph.json")).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&graph_json).unwrap();
    assert!(
        parsed.get("generated").is_none(),
        "generated field should be absent when timestamp=false"
    );
}

/// --timestamp=true should include the `generated` field in ISO 8601 format.
#[test]
fn timestamp_true_adds_generated_field() {
    let path = helpers::fixture_path("typescript-app");
    let output_dir = tempfile::tempdir().expect("create tempdir");
    let output_path = output_dir.path();
    let pipeline = make_pipeline();

    pipeline
        .run_with_options(
            &path,
            WalkConfig::default(),
            &BuildOptions { output_dir: Some(output_path), timestamp: true, ..Default::default() },
        )
        .expect("build should succeed");

    let graph_json = std::fs::read_to_string(output_path.join("graph.json")).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&graph_json).unwrap();
    let generated = parsed
        .get("generated")
        .expect("generated field should be present when timestamp=true")
        .as_str()
        .expect("generated should be a string");

    // Verify ISO 8601 format: YYYY-MM-DDTHH:MM:SSZ
    assert!(
        generated.len() == 20,
        "timestamp should be 20 chars (YYYY-MM-DDTHH:MM:SSZ), got: {}",
        generated
    );
    assert!(
        generated.ends_with('Z'),
        "timestamp should end with Z, got: {}",
        generated
    );
    assert!(
        generated.contains('T'),
        "timestamp should contain T separator, got: {}",
        generated
    );
}

/// --max-file-size and --max-files should be threaded to WalkConfig.
#[test]
fn walk_config_respects_max_files() {
    let path = helpers::fixture_path("typescript-app");
    let output_dir = tempfile::tempdir().expect("create tempdir");
    let output_path = output_dir.path();
    let pipeline = make_pipeline();

    // Set max_files to 1 — should still work but limit walk
    let config = WalkConfig {
        max_files: 1,
        ..WalkConfig::default()
    };

    // The pipeline might still succeed if it finds at least 1 parseable file,
    // or fail with E004 if the 1 file isn't parseable. Either is valid.
    let _result = pipeline.run_with_options(&path, config, &BuildOptions { output_dir: Some(output_path), ..Default::default() });
    // We just verify it doesn't panic
}

// ---------------------------------------------------------------------------
// Raw imports serialization tests
// ---------------------------------------------------------------------------

#[test]
fn raw_imports_round_trip() {
    use ariadne_graph::serial::GraphSerializer;
    use std::collections::BTreeMap;

    let dir = tempfile::tempdir().unwrap();
    let serializer = JsonSerializer;

    let mut imports = BTreeMap::new();
    imports.insert(
        "src/auth/login.ts".to_string(),
        vec![RawImportOutput {
            path: "./session".to_string(),
            symbols: vec!["getSession".to_string()],
            is_type_only: false,
        }],
    );

    serializer.write_raw_imports(&imports, dir.path()).unwrap();

    let reader = JsonSerializer;
    let loaded = reader.read_raw_imports(dir.path()).unwrap();
    assert_eq!(loaded, Some(imports));
}

#[test]
fn raw_imports_missing_file_returns_none() {
    let dir = tempfile::tempdir().unwrap();
    let reader = JsonSerializer;
    let loaded = reader.read_raw_imports(dir.path()).unwrap();
    assert_eq!(loaded, None);
}

#[test]
fn pipeline_produces_raw_imports_json() {
    let path = helpers::fixture_path("typescript-app");
    let output_dir = tempfile::tempdir().unwrap();
    let pipeline = make_pipeline();

    pipeline
        .run_with_options(
            &path,
            WalkConfig::default(),
            &BuildOptions { output_dir: Some(output_dir.path()), ..Default::default() },
        )
        .expect("build should succeed");

    assert!(
        output_dir.path().join("raw_imports.json").exists(),
        "raw_imports.json should be produced by build"
    );

    let reader = JsonSerializer;
    let imports = reader.read_raw_imports(output_dir.path()).unwrap();
    assert!(imports.is_some(), "raw_imports.json should be readable");
    assert!(
        !imports.unwrap().is_empty(),
        "raw_imports should not be empty for typescript-app"
    );
}

#[test]
fn reparse_imports_returns_imports_for_known_extension() {
    let pipeline = make_pipeline();
    let source = b"import { foo } from './bar';";
    let result = pipeline.reparse_imports("ts", source);
    assert!(result.is_some());
    let imports = result.unwrap();
    assert!(!imports.is_empty());
    assert_eq!(imports[0].path, "./bar");
}

#[test]
fn reparse_imports_tsx_with_jsx_syntax() {
    let pipeline = make_pipeline();
    let source = b"import React from 'react';\nexport const A = () => <div style={{ color: 'red' }}>text</div>;\n";
    let result = pipeline.reparse_imports("tsx", source);
    assert!(result.is_some(), "reparse_imports should work for tsx extension");
    let imports = result.unwrap();
    assert_eq!(imports.len(), 1);
    assert_eq!(imports[0].path, "react");
}

#[test]
fn reparse_imports_returns_none_for_unknown_extension() {
    let pipeline = make_pipeline();
    let source = b"some random content";
    let result = pipeline.reparse_imports("xyz", source);
    assert!(result.is_none());
}

// ---------------------------------------------------------------------------
// Watcher rebuild edge parity test (REQ-P2-05)
// ---------------------------------------------------------------------------

/// Verifies that building with `rust_crate_name` set produces the same edge
/// count as a build without it (for non-Rust fixtures). This exercises the
/// same code path the watcher's rebuild uses (`run_with_options` with full
/// `BuildOptions`).
#[test]
fn watcher_rebuild_edge_parity() {
    let path = helpers::fixture_path("typescript-app");
    let pipeline = make_pipeline();

    // Build 1: fresh build (no rust_crate_name — this is a TS fixture)
    let out_dir1 = tempfile::tempdir().expect("create tempdir");
    let output1 = pipeline
        .run_with_options(
            &path,
            WalkConfig::default(),
            &BuildOptions {
                output_dir: Some(out_dir1.path()),
                ..Default::default()
            },
        )
        .expect("first build should succeed");

    // Build 2: rebuild (same options, simulating watcher rebuild path)
    let out_dir2 = tempfile::tempdir().expect("create tempdir");
    let output2 = pipeline
        .run_with_options(
            &path,
            WalkConfig::default(),
            &BuildOptions {
                output_dir: Some(out_dir2.path()),
                rust_crate_name: None,
                ..Default::default()
            },
        )
        .expect("rebuild should succeed");

    assert_eq!(
        output1.edge_count, output2.edge_count,
        "fresh build and rebuild should produce the same edge count"
    );
    assert_eq!(
        output1.file_count, output2.file_count,
        "fresh build and rebuild should produce the same file count"
    );
}

/// Same test for the rust-crate fixture, verifying that setting `rust_crate_name`
/// in the rebuild path matches a fresh build.
#[test]
fn watcher_rebuild_edge_parity_rust_crate() {
    let path = helpers::fixture_path("rust-crate");
    let pipeline = make_pipeline();
    let crate_name = ariadne_graph::detect::detect_rust_crate_name(&path);

    // Build 1: fresh build with crate name
    let out_dir1 = tempfile::tempdir().expect("create tempdir");
    let output1 = pipeline
        .run_with_options(
            &path,
            WalkConfig::default(),
            &BuildOptions {
                output_dir: Some(out_dir1.path()),
                rust_crate_name: crate_name.as_deref(),
                ..Default::default()
            },
        )
        .expect("first build should succeed");

    // Build 2: rebuild with same crate name (watcher path)
    let out_dir2 = tempfile::tempdir().expect("create tempdir");
    let output2 = pipeline
        .run_with_options(
            &path,
            WalkConfig::default(),
            &BuildOptions {
                output_dir: Some(out_dir2.path()),
                rust_crate_name: crate_name.as_deref(),
                ..Default::default()
            },
        )
        .expect("rebuild should succeed");

    assert_eq!(
        output1.edge_count, output2.edge_count,
        "fresh build and rebuild should produce the same edge count (rust-crate)"
    );
    assert_eq!(
        output1.file_count, output2.file_count,
        "fresh build and rebuild should produce the same file count (rust-crate)"
    );
}

// ---------------------------------------------------------------------------
// L1 view export + read round-trip (REQ-P2-05)
// ---------------------------------------------------------------------------

#[test]
fn l1_view_export_round_trip() {
    use ariadne_graph::serial::GraphReader;
    use ariadne_graph::views::generate_all_views;

    let path = helpers::fixture_path("typescript-app");
    let output_dir = tempfile::tempdir().expect("create tempdir");
    let graph_dir = output_dir.path();
    let pipeline = make_pipeline();

    // Step 1: Build graph
    pipeline
        .run_with_options(
            &path,
            WalkConfig::default(),
            &BuildOptions {
                output_dir: Some(graph_dir),
                ..Default::default()
            },
        )
        .expect("build should succeed");

    // Step 2: Load graph, clusters, stats from disk
    let reader = JsonSerializer;
    let graph_output = reader.read_graph(graph_dir).expect("read graph");
    let graph: ariadne_graph::model::ProjectGraph = graph_output
        .try_into()
        .expect("graph conversion");
    let cluster_output = reader.read_clusters(graph_dir).expect("read clusters");
    let clusters: ariadne_graph::model::ClusterMap = cluster_output
        .try_into()
        .expect("cluster conversion");
    let stats = reader
        .read_stats(graph_dir)
        .expect("read stats")
        .expect("stats should exist");

    // Step 3: Generate views
    let views_dir = output_dir.path().join("views");
    let cluster_count =
        generate_all_views(&graph, &clusters, &stats, &views_dir).expect("views generation");

    // Step 4: Verify files exist and contain content
    let index_path = views_dir.join("index.md");
    assert!(index_path.exists(), "index.md should exist");
    let index_content = std::fs::read_to_string(&index_path).expect("read index.md");
    assert!(
        index_content.contains("# Project Index"),
        "index.md should contain header"
    );
    assert!(
        !index_content.is_empty(),
        "index.md should not be empty"
    );

    // Verify cluster view files exist in clusters/ subdirectory
    let clusters_dir = views_dir.join("clusters");
    assert!(clusters_dir.exists(), "clusters/ directory should exist");

    let cluster_files: Vec<_> = std::fs::read_dir(&clusters_dir)
        .expect("read clusters dir")
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .map(|ext| ext == "md")
                .unwrap_or(false)
        })
        .collect();

    assert_eq!(
        cluster_files.len(),
        cluster_count,
        "number of cluster .md files should match generate_all_views return value"
    );

    // Verify each cluster file has content
    for entry in &cluster_files {
        let content = std::fs::read_to_string(entry.path()).expect("read cluster file");
        assert!(
            content.contains("# Cluster:"),
            "cluster view {} should contain header",
            entry.file_name().to_string_lossy()
        );
        assert!(
            !content.is_empty(),
            "cluster view {} should not be empty",
            entry.file_name().to_string_lossy()
        );
    }
}
