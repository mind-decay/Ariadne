mod helpers;

use ariadne_graph::diagnostic::WarningCode;
use std::collections::HashSet;

#[test]
fn typescript_app() {
    let output = helpers::build_fixture("typescript-app");
    assert!(output.file_count > 0, "should have at least one file node");
    assert!(output.edge_count > 0, "should have at least one edge");
    assert!(output.cluster_count > 0, "should have at least one cluster");
}

#[test]
fn go_service() {
    let output = helpers::build_fixture("go-service");
    assert!(output.file_count > 0, "should have at least one file node");
    // Go import resolution may not produce edges in simple fixtures
    assert!(output.cluster_count > 0, "should have at least one cluster");
}

#[test]
fn python_package() {
    let output = helpers::build_fixture("python-package");
    assert!(output.file_count > 0, "should have at least one file node");
    assert!(output.cluster_count > 0, "should have at least one cluster");
}

#[test]
fn mixed_project() {
    let output = helpers::build_fixture("mixed-project");
    assert!(output.file_count > 0, "should have at least one file node");
    assert!(output.cluster_count > 0, "should have at least one cluster");
}

#[test]
fn workspace_project() {
    let output = helpers::build_fixture("workspace-project");

    // Should have files from all 3 packages
    assert!(
        output.file_count >= 6,
        "expected at least 6 files, got {}",
        output.file_count
    );

    // Should have cross-package import edges
    assert!(
        output.edge_count >= 1,
        "expected at least 1 cross-package edge"
    );

    // Read and verify graph.json content
    let graph_json = std::fs::read_to_string(&output.graph_path).unwrap();
    let graph: serde_json::Value = serde_json::from_str(&graph_json).unwrap();

    // Verify the cross-package edge exists (router.ts -> auth/index.ts via @myapp/auth)
    let edges = graph["edges"].as_array().unwrap();
    let has_cross_package_edge = edges.iter().any(|e| {
        let arr = e.as_array().unwrap();
        let from = arr[0].as_str().unwrap_or("");
        let to = arr[1].as_str().unwrap_or("");
        from.contains("router") && to.contains("auth")
    });
    assert!(
        has_cross_package_edge,
        "expected cross-package edge from router.ts to auth/index.ts; edges: {:?}",
        edges
    );
}

/// TSX components fixture — verifies that JSX syntax parses without W001/W007 warnings
/// and that imports/exports are correctly extracted through the full pipeline.
///
/// Covers:
///   Bug #1: implicit JSX return in arrow functions (=> <JSX> and => (<JSX>) on one line)
///   Bug #2: {{ }} in JSX prop + text/expression content on same line
///   Generic arrow functions: <T,>(props) => <JSX>
///   JSX fragments: <>...</> and inline fragments
///   JSX spread attributes: <Comp {...props} />
///   Conditional rendering: && and ternary in JSX
///   .map with arrow returning JSX inside JSX expression
///   Anonymous default export: export default () => <JSX>
///   Multiline and inline callback props
///   .jsx extension (not just .tsx)
#[test]
fn tsx_components() {
    let output = helpers::build_fixture("tsx-components");

    // 11 files: App.tsx + 7 .tsx components + 1 .jsx component
    // (Header, Card, StyledBox, GenericBox, Fragment, SpreadProps, Conditional, DefaultAnon, Callback, LegacyButton)
    assert_eq!(
        output.file_count, 11,
        "expected 11 file nodes, got {}",
        output.file_count
    );

    // No W001 (parse failed) or W007 (partial parse) warnings
    let parse_warnings: Vec<_> = output
        .warnings
        .iter()
        .filter(|w| {
            w.code == WarningCode::W001ParseFailed || w.code == WarningCode::W007PartialParse
        })
        .collect();
    assert!(
        parse_warnings.is_empty(),
        "TSX/JSX files should parse without W001/W007 warnings, got: {:?}",
        parse_warnings
    );

    // App.tsx imports from 10 component files
    assert!(
        output.edge_count >= 10,
        "expected at least 10 edges (App -> each component), got {}",
        output.edge_count
    );

    // Read graph.json for detailed checks
    let json_str = std::fs::read_to_string(&output.graph_path).expect("read graph.json");
    let graph: serde_json::Value = serde_json::from_str(&json_str).expect("parse JSON");
    let nodes = graph["nodes"]
        .as_object()
        .expect("nodes should be an object");
    let node_keys: HashSet<&str> = nodes.keys().map(|k| k.as_str()).collect();

    // All fixture files should appear as nodes
    let expected_files = [
        "App.tsx",
        "Header.tsx",
        "Card.tsx",
        "StyledBox.tsx",
        "GenericBox.tsx",
        "Fragment.tsx",
        "SpreadProps.tsx",
        "Conditional.tsx",
        "DefaultAnon.tsx",
        "Callback.tsx",
        "LegacyButton.jsx",
    ];
    for file in &expected_files {
        assert!(
            node_keys.iter().any(|k| k.contains(file)),
            "{} should be a node; found: {:?}",
            file,
            node_keys
        );
    }

    // Verify edges: App.tsx -> each component file
    let edges = graph["edges"].as_array().expect("edges should be an array");
    let edge_targets = [
        "Header", "Card", "StyledBox", "GenericBox", "Fragment",
        "SpreadProps", "Conditional", "DefaultAnon", "Callback", "LegacyButton",
    ];
    for target in &edge_targets {
        let has_edge = edges.iter().any(|e| {
            let arr = e.as_array().unwrap();
            let from = arr[0].as_str().unwrap_or("");
            let to = arr[1].as_str().unwrap_or("");
            from.contains("App.tsx") && to.contains(target)
        });
        assert!(
            has_edge,
            "expected edge from App.tsx to {}; edges: {:?}",
            target, edges
        );
    }

    // Verify exports were correctly extracted from each component
    let expected_exports: &[(&str, &[&str])] = &[
        // Bug #1a: implicit JSX return inline
        ("Header.tsx", &["Header"]),
        // Bug #1b: parens JSX return inline
        ("Card.tsx", &["Card"]),
        // Bug #2: {{ }} with text/expression
        ("StyledBox.tsx", &["StyledBox", "StyledExpr"]),
        // Generic arrow: <T,>(...) => <JSX>
        ("GenericBox.tsx", &["GenericBox", "Pair"]),
        // JSX fragments
        ("Fragment.tsx", &["Fragment", "InlineFragment"]),
        // JSX spread attributes
        ("SpreadProps.tsx", &["SpreadProps", "MixedSpread"]),
        // Conditional, ternary, .map
        ("Conditional.tsx", &["Conditional", "Ternary", "List", "IndexedList"]),
        // Anonymous default export
        ("DefaultAnon.tsx", &["default"]),
        // Callback props
        ("Callback.tsx", &["Callback", "InlineCallback"]),
        // .jsx extension
        ("LegacyButton.jsx", &["LegacyButton", "LegacyIcon"]),
    ];

    for (file, exports) in expected_exports {
        let file_node = nodes
            .iter()
            .find(|(k, _)| k.contains(file));
        assert!(
            file_node.is_some(),
            "{} should be in graph nodes",
            file
        );
        let (_, node_val) = file_node.unwrap();
        let actual_exports = node_val["exports"]
            .as_array()
            .unwrap_or_else(|| panic!("{} should have exports array", file));
        for exp in *exports {
            assert!(
                actual_exports.iter().any(|e| e.as_str() == Some(exp)),
                "{} should export '{}'; actual exports: {:?}",
                file,
                exp,
                actual_exports
            );
        }
    }
}

#[test]
fn markdown_docs() {
    let output = helpers::build_fixture("markdown-docs");
    assert!(output.file_count > 0, "should have markdown file nodes");
    assert!(output.edge_count > 0, "should have link edges");
    assert!(output.cluster_count > 0, "should have at least one cluster");
}

/// All edge-case checks in a single test to avoid concurrent writes to the same fixture.
#[test]
fn edge_cases() {
    let output = helpers::build_fixture("edge-cases");

    // Basic: should build successfully with some nodes
    assert!(output.file_count > 0, "should have at least one file node");

    // Read graph.json for detailed assertions
    let json_str = std::fs::read_to_string(&output.graph_path).expect("read graph.json");
    let graph: serde_json::Value = serde_json::from_str(&json_str).expect("parse JSON");
    let nodes = graph["nodes"]
        .as_object()
        .expect("nodes should be an object");
    let node_keys: HashSet<&str> = nodes.keys().map(|k| k.as_str()).collect();

    // empty.ts should appear as a node
    let has_empty = node_keys.iter().any(|k| k.contains("empty.ts"));
    assert!(
        has_empty,
        "empty.ts should appear as a node; found: {:?}",
        node_keys
    );

    // Both circular files should be nodes
    let has_a = node_keys.iter().any(|k| k.contains("circular-a.ts"));
    let has_b = node_keys.iter().any(|k| k.contains("circular-b.ts"));
    assert!(
        has_a,
        "circular-a.ts should be a node; found: {:?}",
        node_keys
    );
    assert!(
        has_b,
        "circular-b.ts should be a node; found: {:?}",
        node_keys
    );

    // There should be edges between circular files
    let edges = graph["edges"].as_array().expect("edges should be an array");
    let circular_edges: Vec<_> = edges
        .iter()
        .filter(|e| {
            let arr = e.as_array().unwrap();
            let from = arr[0].as_str().unwrap();
            let to = arr[1].as_str().unwrap();
            (from.contains("circular-a") && to.contains("circular-b"))
                || (from.contains("circular-b") && to.contains("circular-a"))
        })
        .collect();
    assert!(
        !circular_edges.is_empty(),
        "there should be edges between circular-a.ts and circular-b.ts"
    );

    // Binary and bad-encoding files should be skipped with warnings
    // binary-file.ts should NOT appear as a node
    let has_binary = node_keys.iter().any(|k| k.contains("binary-file.ts"));
    assert!(
        !has_binary,
        "binary-file.ts should NOT appear as a node (should be skipped); found: {:?}",
        node_keys
    );

    // bad-encoding.ts should NOT appear as a node
    let has_bad_encoding = node_keys.iter().any(|k| k.contains("bad-encoding.ts"));
    assert!(
        !has_bad_encoding,
        "bad-encoding.ts should NOT appear as a node (should be skipped); found: {:?}",
        node_keys
    );

    // Verify W004 (BinaryFile) warning was emitted
    let has_w004 = output
        .warnings
        .iter()
        .any(|w| w.code == WarningCode::W004BinaryFile);
    assert!(has_w004, "expected W004 warning for binary-file.ts");

    // Verify W009 (EncodingError) warning was emitted
    let has_w009 = output
        .warnings
        .iter()
        .any(|w| w.code == WarningCode::W009EncodingError);
    assert!(has_w009, "expected W009 warning for bad-encoding.ts");
}
