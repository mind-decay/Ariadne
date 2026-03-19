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
        state
            .new_files
            .push(std::path::PathBuf::from("src/new.ts"));
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
    use ariadne_graph::serial::RawImportOutput;
    use std::collections::BTreeMap;

    fn make_test_graph() -> (ProjectGraph, StatsOutput, ClusterMap) {
        let mut nodes = BTreeMap::new();
        nodes.insert(
            CanonicalPath::new("src/a.ts"),
            Node {
                file_type: FileType::Source,
                layer: ArchLayer::Service,
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
    fn graph_state_extracts_file_hashes() {
        let (graph, stats, clusters) = make_test_graph();
        let state = GraphState::from_loaded_data(graph, stats, clusters, BTreeMap::new());

        assert_eq!(
            state.file_hashes.get(&CanonicalPath::new("src/a.ts")),
            Some(&ContentHash::new("abc123".to_string()))
        );
    }
}
