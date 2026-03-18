mod helpers;

use std::collections::{BTreeSet, HashSet};

const FIXTURES: &[&str] = &[
    "typescript-app",
    "go-service",
    "python-package",
    "mixed-project",
    "edge-cases",
    "workspace-project",
];

/// Build a fixture and return parsed (graph, clusters) JSON values.
fn build_and_parse(fixture: &str) -> (serde_json::Value, serde_json::Value) {
    let output = helpers::build_fixture(fixture);
    let graph_json = std::fs::read_to_string(&output.graph_path)
        .unwrap_or_else(|e| panic!("failed to read graph.json for '{}': {}", fixture, e));
    let clusters_json = std::fs::read_to_string(&output.clusters_path)
        .unwrap_or_else(|e| panic!("failed to read clusters.json for '{}': {}", fixture, e));
    let graph: serde_json::Value =
        serde_json::from_str(&graph_json).expect("graph.json is not valid JSON");
    let clusters: serde_json::Value =
        serde_json::from_str(&clusters_json).expect("clusters.json is not valid JSON");
    (graph, clusters)
}

macro_rules! invariant_test {
    ($name:ident, $check:expr) => {
        #[test]
        fn $name() {
            for fixture in FIXTURES {
                let (graph, clusters) = build_and_parse(fixture);
                $check(&graph, &clusters, fixture);
            }
        }
    };
}

// INV-1: Every edge's `from` and `to` must reference a node that exists in `nodes`.
invariant_test!(inv1_edge_referential_integrity, |graph: &serde_json::Value,
                                                  _clusters: &serde_json::Value,
                                                  fixture: &str| {
    let nodes = graph["nodes"].as_object().expect("nodes should be an object");
    let edges = graph["edges"].as_array().expect("edges should be an array");
    let node_ids: HashSet<&str> = nodes.keys().map(|k| k.as_str()).collect();

    for (i, edge) in edges.iter().enumerate() {
        let edge_arr = edge.as_array().expect("each edge should be an array");
        let from = edge_arr[0].as_str().expect("edge[0] (from) should be a string");
        let to = edge_arr[1].as_str().expect("edge[1] (to) should be a string");

        assert!(
            node_ids.contains(from),
            "INV-1 violated in '{}': edge {} has from='{}' which is not in nodes",
            fixture, i, from
        );
        assert!(
            node_ids.contains(to),
            "INV-1 violated in '{}': edge {} has to='{}' which is not in nodes",
            fixture, i, to
        );
    }
});

// INV-2: No edge has from == to (no self-imports).
invariant_test!(inv2_no_self_imports, |graph: &serde_json::Value,
                                       _clusters: &serde_json::Value,
                                       fixture: &str| {
    let edges = graph["edges"].as_array().expect("edges should be an array");

    for (i, edge) in edges.iter().enumerate() {
        let edge_arr = edge.as_array().expect("each edge should be an array");
        let from = edge_arr[0].as_str().expect("edge[0] (from) should be a string");
        let to = edge_arr[1].as_str().expect("edge[1] (to) should be a string");

        assert_ne!(
            from, to,
            "INV-2 violated in '{}': edge {} is a self-import (from == to == '{}')",
            fixture, i, from
        );
    }
});

// INV-3: Test edges connect test files to source/type_def files.
invariant_test!(inv3_test_edges_connect_test_to_source, |graph: &serde_json::Value,
                                                          _clusters: &serde_json::Value,
                                                          fixture: &str| {
    let nodes = graph["nodes"].as_object().expect("nodes should be an object");
    let edges = graph["edges"].as_array().expect("edges should be an array");

    for (i, edge) in edges.iter().enumerate() {
        let edge_arr = edge.as_array().expect("each edge should be an array");
        let from = edge_arr[0].as_str().unwrap();
        let to = edge_arr[1].as_str().unwrap();
        let edge_type = edge_arr[2].as_str().unwrap();

        if edge_type == "tests" {
            let from_type = nodes[from]["type"].as_str().unwrap_or("");
            let to_type = nodes[to]["type"].as_str().unwrap_or("");

            assert!(
                from_type == "test" && (to_type == "source" || to_type == "type_def"),
                "INV-3 violated in '{}': test edge {} (from='{}' type={}, to='{}' type={}) \
                 — expected from to be 'test' AND to to be 'source'/'type_def'",
                fixture, i, from, from_type, to, to_type
            );
        }
    }
});

// INV-4: Every node belongs to a cluster (cluster field is non-empty).
invariant_test!(inv4_every_node_has_cluster, |graph: &serde_json::Value,
                                              _clusters: &serde_json::Value,
                                              fixture: &str| {
    let nodes = graph["nodes"].as_object().expect("nodes should be an object");

    for (path, node) in nodes {
        let cluster = node["cluster"].as_str().unwrap_or("");
        assert!(
            !cluster.is_empty(),
            "INV-4 violated in '{}': node '{}' has empty cluster",
            fixture, path
        );
    }
});

// INV-5: Cluster file lists are complete.
// - Every file in a cluster exists as a node in graph.json
// - Every node's cluster field points to an existing cluster
invariant_test!(inv5_cluster_file_lists_complete, |graph: &serde_json::Value,
                                                    clusters: &serde_json::Value,
                                                    fixture: &str| {
    let nodes = graph["nodes"].as_object().expect("nodes should be an object");
    let cluster_map = clusters["clusters"]
        .as_object()
        .expect("clusters should be an object");

    let cluster_ids: HashSet<&str> = cluster_map.keys().map(|k| k.as_str()).collect();
    let node_ids: HashSet<&str> = nodes.keys().map(|k| k.as_str()).collect();

    // Every file listed in a cluster must exist as a node
    for (cluster_id, cluster_entry) in cluster_map {
        let files = cluster_entry["files"]
            .as_array()
            .expect("cluster files should be an array");
        for file in files {
            let file_path = file.as_str().unwrap();
            assert!(
                node_ids.contains(file_path),
                "INV-5 violated in '{}': cluster '{}' lists file '{}' which is not a node",
                fixture, cluster_id, file_path
            );
        }
    }

    // Every node's cluster must reference an existing cluster
    for (path, node) in nodes {
        let cluster = node["cluster"].as_str().unwrap_or("");
        assert!(
            cluster_ids.contains(cluster),
            "INV-5 violated in '{}': node '{}' has cluster '{}' which doesn't exist in clusters.json",
            fixture, path, cluster
        );
    }
});

// INV-6: Cluster edge counts are correct.
// internal_edges = edges where both from and to are in the cluster
// external_edges = edges where exactly one endpoint is in the cluster
invariant_test!(inv6_cluster_edge_counts, |graph: &serde_json::Value,
                                           clusters: &serde_json::Value,
                                           fixture: &str| {
    let edges = graph["edges"].as_array().expect("edges should be an array");
    let cluster_map = clusters["clusters"]
        .as_object()
        .expect("clusters should be an object");

    for (cluster_id, cluster_entry) in cluster_map {
        let files = cluster_entry["files"]
            .as_array()
            .expect("cluster files should be an array");
        let file_set: HashSet<&str> = files.iter().map(|f| f.as_str().unwrap()).collect();

        let expected_internal = cluster_entry["internal_edges"].as_u64().unwrap() as usize;
        let expected_external = cluster_entry["external_edges"].as_u64().unwrap() as usize;

        let mut actual_internal = 0usize;
        let mut actual_external = 0usize;

        for edge in edges {
            let edge_arr = edge.as_array().unwrap();
            let from = edge_arr[0].as_str().unwrap();
            let to = edge_arr[1].as_str().unwrap();

            // Look up which cluster from/to belong to
            let from_in = file_set.contains(from);
            let to_in = file_set.contains(to);

            if from_in && to_in {
                actual_internal += 1;
            } else if from_in || to_in {
                actual_external += 1;
            }
        }

        assert_eq!(
            actual_internal, expected_internal,
            "INV-6 violated in '{}': cluster '{}' internal_edges: expected {}, got {}",
            fixture, cluster_id, expected_internal, actual_internal
        );
        assert_eq!(
            actual_external, expected_external,
            "INV-6 violated in '{}': cluster '{}' external_edges: expected {}, got {}",
            fixture, cluster_id, expected_external, actual_external
        );
    }
});

// INV-7: Cohesion is correctly computed.
// Formula: if internal == 0 && external == 0 → 1.0; else internal / (internal + external)
// Rounded to 4 decimal places.
invariant_test!(inv7_cohesion_correct, |_graph: &serde_json::Value,
                                        clusters: &serde_json::Value,
                                        fixture: &str| {
    let cluster_map = clusters["clusters"]
        .as_object()
        .expect("clusters should be an object");

    for (cluster_id, cluster_entry) in cluster_map {
        let internal_edges = cluster_entry["internal_edges"].as_u64().unwrap() as f64;
        let external_edges = cluster_entry["external_edges"].as_u64().unwrap() as f64;
        let cohesion = cluster_entry["cohesion"].as_f64().unwrap();

        let expected = if internal_edges == 0.0 && external_edges == 0.0 {
            1.0
        } else {
            internal_edges / (internal_edges + external_edges)
        };
        let expected_rounded = (expected * 10000.0).round() / 10000.0;
        let cohesion_rounded = (cohesion * 10000.0).round() / 10000.0;

        assert_eq!(
            cohesion_rounded, expected_rounded,
            "INV-7 violated in '{}': cluster '{}' cohesion: expected {}, got {} \
             (internal_edges={}, external_edges={})",
            fixture, cluster_id, expected_rounded, cohesion_rounded, internal_edges, external_edges
        );
    }
});

// INV-8: node_count matches nodes.len(), edge_count matches edges.len().
invariant_test!(inv8_counts_match, |graph: &serde_json::Value,
                                    _clusters: &serde_json::Value,
                                    fixture: &str| {
    let node_count = graph["node_count"].as_u64().expect("node_count should be a number") as usize;
    let edge_count = graph["edge_count"].as_u64().expect("edge_count should be a number") as usize;

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
        "INV-8 violated in '{}': node_count ({}) != nodes.len() ({})",
        fixture, node_count, nodes_len
    );
    assert_eq!(
        edge_count, edges_len,
        "INV-8 violated in '{}': edge_count ({}) != edges.len() ({})",
        fixture, edge_count, edges_len
    );
});

// INV-9: No two edges have the same (from, to, edge_type) triple.
invariant_test!(inv9_no_duplicate_edges, |graph: &serde_json::Value,
                                          _clusters: &serde_json::Value,
                                          fixture: &str| {
    let edges = graph["edges"].as_array().expect("edges should be an array");

    let mut seen = BTreeSet::new();
    for (i, edge) in edges.iter().enumerate() {
        let edge_arr = edge.as_array().expect("each edge should be an array");
        let from = edge_arr[0].as_str().unwrap();
        let to = edge_arr[1].as_str().unwrap();
        let edge_type = edge_arr[2].as_str().unwrap();

        let key = (from.to_string(), to.to_string(), edge_type.to_string());
        assert!(
            seen.insert(key.clone()),
            "INV-9 violated in '{}': duplicate edge at index {}: ({}, {}, {})",
            fixture, i, key.0, key.1, key.2
        );
    }
});

/// INV-10: Content hashes are deterministic across builds.
#[test]
fn inv10_content_hashes_deterministic() {
    for fixture in FIXTURES {
        let (graph1, _) = build_and_parse(fixture);
        let (graph2, _) = build_and_parse(fixture);

        let nodes1 = graph1["nodes"].as_object().expect("nodes should be an object");
        let nodes2 = graph2["nodes"].as_object().expect("nodes should be an object");

        assert_eq!(
            nodes1.len(),
            nodes2.len(),
            "INV-10 violated in '{}': different node counts across builds",
            fixture
        );

        for (path, node1) in nodes1 {
            let hash1 = node1["hash"].as_str().unwrap();
            let node2 = &nodes2[path];
            let hash2 = node2["hash"].as_str().unwrap();

            assert_eq!(
                hash1, hash2,
                "INV-10 violated in '{}': hash for '{}' differs across builds: '{}' vs '{}'",
                fixture, path, hash1, hash2
            );
        }
    }
}

/// INV-11: Building the same fixture twice produces byte-identical graph.json.
#[test]
fn inv11_determinism() {
    for fixture in FIXTURES {
        let output1 = helpers::build_fixture(fixture);
        let output2 = helpers::build_fixture(fixture);

        let json1 = std::fs::read_to_string(&output1.graph_path).unwrap();
        let json2 = std::fs::read_to_string(&output2.graph_path).unwrap();

        assert_eq!(
            json1, json2,
            "INV-11 violated: two builds of '{}' produced different graph.json output",
            fixture
        );
    }
}

// INV-12: Type-only imports produce type_imports edges.
// If any edge has type "type_imports", it structurally exists (basic check).
invariant_test!(inv12_type_imports_edges, |graph: &serde_json::Value,
                                           _clusters: &serde_json::Value,
                                           _fixture: &str| {
    let nodes = graph["nodes"].as_object().expect("nodes should be an object");
    let edges = graph["edges"].as_array().expect("edges should be an array");
    let node_ids: HashSet<&str> = nodes.keys().map(|k| k.as_str()).collect();

    for edge in edges {
        let edge_arr = edge.as_array().unwrap();
        let from = edge_arr[0].as_str().unwrap();
        let to = edge_arr[1].as_str().unwrap();
        let edge_type = edge_arr[2].as_str().unwrap();

        if edge_type == "type_imports" {
            // Both endpoints must exist (already covered by INV-1, but verify specifically)
            assert!(
                node_ids.contains(from) && node_ids.contains(to),
                "INV-12: type_imports edge has invalid endpoints: from='{}', to='{}'",
                from, to
            );
        }
    }
});

// INV-13: Re-export edges have valid structure.
// If any edge has type "re_exports", verify it exists with valid endpoints.
invariant_test!(inv13_re_export_edges_valid, |graph: &serde_json::Value,
                                              _clusters: &serde_json::Value,
                                              _fixture: &str| {
    let nodes = graph["nodes"].as_object().expect("nodes should be an object");
    let edges = graph["edges"].as_array().expect("edges should be an array");
    let node_ids: HashSet<&str> = nodes.keys().map(|k| k.as_str()).collect();

    for edge in edges {
        let edge_arr = edge.as_array().unwrap();
        let from = edge_arr[0].as_str().unwrap();
        let to = edge_arr[1].as_str().unwrap();
        let edge_type = edge_arr[2].as_str().unwrap();

        if edge_type == "re_exports" {
            assert!(
                node_ids.contains(from) && node_ids.contains(to),
                "INV-13: re_exports edge has invalid endpoints: from='{}', to='{}'",
                from, to
            );
            // from and to must be different (already covered by INV-2, but verify specifically)
            assert_ne!(
                from, to,
                "INV-13: re_exports edge is a self-reference: '{}'",
                from
            );
        }
    }
});
