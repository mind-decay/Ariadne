mod helpers;

use std::collections::{BTreeSet, HashSet};

const FIXTURE: &str = "typescript-app";

/// Read the graph.json output for a fixture into a serde_json::Value.
fn read_graph_json(fixture_name: &str) -> serde_json::Value {
    let json_str = helpers::build_and_read_graph_json(fixture_name);
    serde_json::from_str(&json_str).expect("graph.json is not valid JSON")
}

/// INV-1: Every edge's `from` and `to` must reference a node that exists in `nodes`.
#[test]
fn inv1_edge_referential_integrity() {
    let graph = read_graph_json(FIXTURE);

    let nodes = graph["nodes"].as_object().expect("nodes should be an object");
    let edges = graph["edges"].as_array().expect("edges should be an array");

    let node_ids: HashSet<&str> = nodes.keys().map(|k| k.as_str()).collect();

    for (i, edge) in edges.iter().enumerate() {
        let edge_arr = edge.as_array().expect("each edge should be an array");
        let from = edge_arr[0].as_str().expect("edge[0] (from) should be a string");
        let to = edge_arr[1].as_str().expect("edge[1] (to) should be a string");

        assert!(
            node_ids.contains(from),
            "INV-1 violated: edge {} has from='{}' which is not in nodes",
            i,
            from
        );
        assert!(
            node_ids.contains(to),
            "INV-1 violated: edge {} has to='{}' which is not in nodes",
            i,
            to
        );
    }
}

/// INV-2: No edge has from == to (no self-imports).
#[test]
fn inv2_no_self_imports() {
    let graph = read_graph_json(FIXTURE);

    let edges = graph["edges"].as_array().expect("edges should be an array");

    for (i, edge) in edges.iter().enumerate() {
        let edge_arr = edge.as_array().expect("each edge should be an array");
        let from = edge_arr[0].as_str().expect("edge[0] (from) should be a string");
        let to = edge_arr[1].as_str().expect("edge[1] (to) should be a string");

        assert_ne!(
            from, to,
            "INV-2 violated: edge {} is a self-import (from == to == '{}')",
            i, from
        );
    }
}

/// INV-8: node_count matches nodes.len(), edge_count matches edges.len().
#[test]
fn inv8_counts_match() {
    let graph = read_graph_json(FIXTURE);

    let node_count = graph["node_count"]
        .as_u64()
        .expect("node_count should be a number") as usize;
    let edge_count = graph["edge_count"]
        .as_u64()
        .expect("edge_count should be a number") as usize;

    let nodes_len = graph["nodes"]
        .as_object()
        .expect("nodes should be an object")
        .len();
    let edges_len = graph["edges"]
        .as_array()
        .expect("edges should be an array")
        .len();

    assert_eq!(
        node_count, nodes_len,
        "INV-8 violated: node_count ({}) != nodes.len() ({})",
        node_count, nodes_len
    );
    assert_eq!(
        edge_count, edges_len,
        "INV-8 violated: edge_count ({}) != edges.len() ({})",
        edge_count, edges_len
    );
}

/// INV-9: No two edges have the same (from, to, edge_type) triple.
#[test]
fn inv9_no_duplicate_edges() {
    let graph = read_graph_json(FIXTURE);

    let edges = graph["edges"].as_array().expect("edges should be an array");

    let mut seen = BTreeSet::new();
    for (i, edge) in edges.iter().enumerate() {
        let edge_arr = edge.as_array().expect("each edge should be an array");
        let from = edge_arr[0].as_str().expect("edge[0] (from) should be a string");
        let to = edge_arr[1].as_str().expect("edge[1] (to) should be a string");
        let edge_type = edge_arr[2].as_str().expect("edge[2] (edge_type) should be a string");

        let key = (from.to_string(), to.to_string(), edge_type.to_string());
        assert!(
            seen.insert(key.clone()),
            "INV-9 violated: duplicate edge at index {}: ({}, {}, {})",
            i,
            key.0,
            key.1,
            key.2
        );
    }
}

/// INV-11: Building the same fixture twice produces byte-identical graph.json.
#[test]
fn inv11_determinism() {
    let json1 = helpers::build_and_read_graph_json(FIXTURE);
    let json2 = helpers::build_and_read_graph_json(FIXTURE);

    assert_eq!(
        json1, json2,
        "INV-11 violated: two builds of '{}' produced different graph.json output",
        FIXTURE
    );
}
