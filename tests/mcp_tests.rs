mod helpers;

#[cfg(feature = "serve")]
mod lock_tests {
    use ariadne_graph::mcp::lock::{acquire_lock, check_lock, release_lock};
    use tempfile::tempdir;

    #[test]
    fn test_acquire_and_release_lock() {
        let dir = tempdir().unwrap();
        let lock_path = dir.path().join(".lock");

        acquire_lock(&lock_path).unwrap();
        assert!(lock_path.exists());

        let status = check_lock(&lock_path).unwrap();
        assert!(status.is_held_by_us());

        release_lock(&lock_path).unwrap();
        assert!(!lock_path.exists());
    }

    #[test]
    fn test_check_lock_no_file() {
        let dir = tempdir().unwrap();
        let lock_path = dir.path().join(".lock");

        let status = check_lock(&lock_path).unwrap();
        assert!(status.is_free());
    }

    #[test]
    fn test_stale_lock_detection() {
        let dir = tempdir().unwrap();
        let lock_path = dir.path().join(".lock");

        // Write a lock with a fake PID that doesn't exist
        let content = serde_json::json!({
            "pid": 999999999u32,
            "started_at": "2026-01-01T00:00:00Z"
        });
        std::fs::write(&lock_path, serde_json::to_string(&content).unwrap()).unwrap();

        let status = check_lock(&lock_path).unwrap();
        assert!(status.is_stale());
    }

    #[test]
    fn test_corrupted_lock_treated_as_stale() {
        let dir = tempdir().unwrap();
        let lock_path = dir.path().join(".lock");

        // Write garbage to the lock file
        std::fs::write(&lock_path, "not valid json!!!").unwrap();

        let status = check_lock(&lock_path).unwrap();
        assert!(
            status.is_stale(),
            "Corrupted lock should be treated as stale"
        );
    }

    #[test]
    fn test_acquire_removes_stale_lock() {
        let dir = tempdir().unwrap();
        let lock_path = dir.path().join(".lock");

        // Write a stale lock
        let content = serde_json::json!({
            "pid": 999999999u32,
            "started_at": "2026-01-01T00:00:00Z"
        });
        std::fs::write(&lock_path, serde_json::to_string(&content).unwrap()).unwrap();

        // Acquire should succeed (stale lock gets replaced)
        acquire_lock(&lock_path).unwrap();
        let status = check_lock(&lock_path).unwrap();
        assert!(status.is_held_by_us());

        release_lock(&lock_path).unwrap();
    }

    #[test]
    fn test_double_acquire_is_idempotent() {
        let dir = tempdir().unwrap();
        let lock_path = dir.path().join(".lock");

        acquire_lock(&lock_path).unwrap();
        // Second acquire by same process should be fine
        acquire_lock(&lock_path).unwrap();

        let status = check_lock(&lock_path).unwrap();
        assert!(status.is_held_by_us());

        release_lock(&lock_path).unwrap();
    }

    #[test]
    fn test_release_nonexistent_is_ok() {
        let dir = tempdir().unwrap();
        let lock_path = dir.path().join(".lock");

        // Should not error
        release_lock(&lock_path).unwrap();
    }
}

#[cfg(feature = "serve")]
mod freshness_tests {
    use ariadne_graph::mcp::state::FreshnessState;
    use ariadne_graph::model::CanonicalPath;

    #[test]
    fn fresh_state_has_full_confidence() {
        let state = FreshnessState::new();
        assert_eq!(state.hash_confidence, 1.0);
        assert_eq!(state.structural_confidence, 1.0);
    }

    #[test]
    fn stale_files_reduce_hash_confidence() {
        let mut state = FreshnessState::new();
        state.stale_files.insert(CanonicalPath::new("src/a.ts"));
        state.stale_files.insert(CanonicalPath::new("src/b.ts"));
        state.recompute_confidence(10);
        assert!((state.hash_confidence - 0.8).abs() < 0.001);
    }

    #[test]
    fn body_only_change_keeps_structural_confidence_high() {
        let mut state = FreshnessState::new();
        // 2 files stale but no structural changes
        state.stale_files.insert(CanonicalPath::new("src/a.ts"));
        state.stale_files.insert(CanonicalPath::new("src/b.ts"));
        state.recompute_confidence(10);
        // Hash confidence drops but structural stays at 1.0
        assert!((state.hash_confidence - 0.8).abs() < 0.001);
        assert_eq!(state.structural_confidence, 1.0);
    }

    #[test]
    fn structural_changes_reduce_structural_confidence() {
        let mut state = FreshnessState::new();
        state.stale_files.insert(CanonicalPath::new("src/a.ts"));
        state
            .structurally_changed
            .insert(CanonicalPath::new("src/a.ts"));
        state.recompute_confidence(10);
        assert!((state.hash_confidence - 0.9).abs() < 0.001);
        assert!((state.structural_confidence - 0.9).abs() < 0.001);
    }

    #[test]
    fn new_files_affect_confidence() {
        let mut state = FreshnessState::new();
        state.new_files.push(std::path::PathBuf::from("src/new.ts"));
        state.recompute_confidence(10);
        assert!((state.hash_confidence - 0.9).abs() < 0.001);
        assert!((state.structural_confidence - 0.9).abs() < 0.001);
    }

    #[test]
    fn empty_graph_has_full_confidence() {
        let mut state = FreshnessState::new();
        state.stale_files.insert(CanonicalPath::new("src/a.ts"));
        state.recompute_confidence(0);
        assert_eq!(state.hash_confidence, 1.0);
        assert_eq!(state.structural_confidence, 1.0);
    }
}

#[cfg(feature = "serve")]
mod state_tests {
    use ariadne_graph::mcp::state::GraphState;
    use ariadne_graph::model::*;
    use std::collections::BTreeMap;

    fn make_test_graph() -> (ProjectGraph, StatsOutput, ClusterMap) {
        let mut nodes = BTreeMap::new();
        nodes.insert(
            CanonicalPath::new("src/a.ts"),
            Node {
                file_type: FileType::Source,
                layer: ArchLayer::Service,
                fsd_layer: None,
                arch_depth: 1,
                lines: 100,
                hash: ContentHash::new("abc123".to_string()),
                exports: vec![Symbol::new("foo")],
                cluster: ClusterId::new("src"),
            },
        );
        nodes.insert(
            CanonicalPath::new("src/b.ts"),
            Node {
                file_type: FileType::Source,
                layer: ArchLayer::Service,
                fsd_layer: None,
                arch_depth: 0,
                lines: 50,
                hash: ContentHash::new("def456".to_string()),
                exports: vec![],
                cluster: ClusterId::new("src"),
            },
        );
        let edges = vec![Edge {
            from: CanonicalPath::new("src/a.ts"),
            to: CanonicalPath::new("src/b.ts"),
            edge_type: EdgeType::Imports,
            symbols: vec![Symbol::new("bar")],
        }];
        let graph = ProjectGraph { nodes, edges };

        let stats = StatsOutput {
            version: 1,
            centrality: BTreeMap::new(),
            sccs: vec![],
            layers: BTreeMap::new(),
            summary: StatsSummary {
                max_depth: 1,
                avg_in_degree: 0.5,
                avg_out_degree: 0.5,
                bottleneck_files: vec![],
                orphan_files: vec![],
            },
        };

        let mut clusters_map = BTreeMap::new();
        clusters_map.insert(
            ClusterId::new("src"),
            Cluster {
                files: vec![
                    CanonicalPath::new("src/a.ts"),
                    CanonicalPath::new("src/b.ts"),
                ],
                file_count: 2,
                internal_edges: 1,
                external_edges: 0,
                cohesion: 1.0,
            },
        );
        let clusters = ClusterMap {
            clusters: clusters_map,
        };

        (graph, stats, clusters)
    }

    #[test]
    fn graph_state_builds_reverse_index() {
        let (graph, stats, clusters) = make_test_graph();
        let state = GraphState::from_loaded_data(graph, stats, clusters, BTreeMap::new());

        // b.ts should have one incoming edge from a.ts
        let b_incoming = state
            .reverse_index
            .get(&CanonicalPath::new("src/b.ts"))
            .unwrap();
        assert_eq!(b_incoming.len(), 1);
        assert_eq!(b_incoming[0].from, CanonicalPath::new("src/a.ts"));
    }

    #[test]
    fn graph_state_builds_layer_index() {
        let (graph, stats, clusters) = make_test_graph();
        let state = GraphState::from_loaded_data(graph, stats, clusters, BTreeMap::new());

        let layer0 = state.layer_index.get(&0).unwrap();
        assert_eq!(layer0.len(), 1);
        assert_eq!(layer0[0], CanonicalPath::new("src/b.ts"));

        let layer1 = state.layer_index.get(&1).unwrap();
        assert_eq!(layer1.len(), 1);
        assert_eq!(layer1[0], CanonicalPath::new("src/a.ts"));
    }

    #[test]
    fn graph_state_builds_forward_index() {
        let (graph, stats, clusters) = make_test_graph();
        let state = GraphState::from_loaded_data(graph, stats, clusters, BTreeMap::new());

        // a.ts should have one outgoing edge to b.ts
        let a_outgoing = state
            .forward_index
            .get(&CanonicalPath::new("src/a.ts"))
            .unwrap();
        assert_eq!(a_outgoing.len(), 1);
        assert_eq!(a_outgoing[0].to, CanonicalPath::new("src/b.ts"));

        // b.ts has no outgoing edges
        assert!(!state
            .forward_index
            .contains_key(&CanonicalPath::new("src/b.ts")));
    }

    #[test]
    fn graph_state_extracts_file_hashes() {
        let (graph, stats, clusters) = make_test_graph();
        let state = GraphState::from_loaded_data(graph, stats, clusters, BTreeMap::new());

        assert_eq!(
            state.file_hashes.get(&CanonicalPath::new("src/a.ts")),
            Some(&ContentHash::new("abc123".to_string()))
        );
    }
}

#[cfg(feature = "serve")]
mod integration_tests {
    use std::io::{BufRead, BufReader, Write};
    use std::process::{Command, Stdio};

    /// Build the fixture first, then spawn ariadne serve as subprocess.
    /// Send JSON-RPC initialize, verify response.
    #[test]
    fn test_mcp_server_initialize_and_tool_list() {
        let fixture = crate::helpers::fixture_path("typescript-app");

        // Build the fixture first
        let build_output = crate::helpers::build_fixture("typescript-app");
        assert!(build_output.graph_path.exists());

        let output_dir = build_output.graph_path.parent().unwrap();

        // Find the ariadne binary
        let binary = env!("CARGO_BIN_EXE_ariadne");

        // Spawn ariadne serve
        let mut child = Command::new(binary)
            .args([
                "serve",
                "--project",
                fixture.to_str().unwrap(),
                "--output",
                output_dir.to_str().unwrap(),
                "--no-watch",
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("failed to spawn ariadne serve");

        let stdin = child.stdin.as_mut().unwrap();
        let stdout = child.stdout.take().unwrap();
        let mut reader = BufReader::new(stdout);

        // Send initialize request
        let init_request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {
                    "name": "test-client",
                    "version": "0.1.0"
                }
            }
        });
        let request_str = serde_json::to_string(&init_request).unwrap();
        writeln!(stdin, "{}", request_str).unwrap();
        stdin.flush().unwrap();

        // Read response
        let mut response_line = String::new();
        reader.read_line(&mut response_line).unwrap();

        let response: serde_json::Value = serde_json::from_str(response_line.trim()).unwrap();
        assert_eq!(response["jsonrpc"], "2.0");
        assert_eq!(response["id"], 1);
        assert!(response["result"].is_object(), "Should have result field");
        assert!(
            response["result"]["capabilities"]["tools"].is_object(),
            "Should advertise tools capability"
        );

        // Send initialized notification
        let initialized = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized"
        });
        writeln!(stdin, "{}", serde_json::to_string(&initialized).unwrap()).unwrap();
        stdin.flush().unwrap();

        // Send tools/list request
        let list_tools = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/list"
        });
        writeln!(stdin, "{}", serde_json::to_string(&list_tools).unwrap()).unwrap();
        stdin.flush().unwrap();

        let mut tools_response = String::new();
        reader.read_line(&mut tools_response).unwrap();

        let tools_resp: serde_json::Value = serde_json::from_str(tools_response.trim()).unwrap();
        assert_eq!(tools_resp["id"], 2);
        let tools = tools_resp["result"]["tools"].as_array().unwrap();
        assert!(
            tools.len() >= 11,
            "Should have at least 11 tools, got {}",
            tools.len()
        );

        // Verify tool names
        let tool_names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
        assert!(tool_names.contains(&"ariadne_overview"));
        assert!(tool_names.contains(&"ariadne_file"));
        assert!(tool_names.contains(&"ariadne_blast_radius"));
        assert!(tool_names.contains(&"ariadne_freshness"));

        // Kill the server and wait to avoid zombie process
        child.kill().ok();
        child.wait().ok();
    }
}
