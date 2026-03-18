mod helpers;

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

/// All edge-case checks in a single test to avoid concurrent writes to the same fixture.
#[test]
fn edge_cases() {
    let output = helpers::build_fixture("edge-cases");

    // Basic: should build successfully with some nodes
    assert!(output.file_count > 0, "should have at least one file node");

    // Read graph.json for detailed assertions
    let json_str = std::fs::read_to_string(&output.graph_path).expect("read graph.json");
    let graph: serde_json::Value = serde_json::from_str(&json_str).expect("parse JSON");
    let nodes = graph["nodes"].as_object().expect("nodes should be an object");
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
    assert!(has_a, "circular-a.ts should be a node; found: {:?}", node_keys);
    assert!(has_b, "circular-b.ts should be a node; found: {:?}", node_keys);

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
}
