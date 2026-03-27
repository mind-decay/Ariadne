//! MCP resource handlers for Ariadne.
//!
//! Pure functions that take GraphState and return rmcp resource types.
//! Resources expose read-only views of the dependency graph to MCP clients.

use std::collections::BTreeMap;

use rmcp::model::*;

use crate::analysis::smells::detect_smells;
use crate::mcp::state::GraphState;

/// List all available Ariadne resources.
///
/// Returns static resources (overview, smells, hotspots, freshness) plus
/// dynamic resources for each file and cluster in the current graph.
pub fn list_resources_impl(state: &GraphState) -> ListResourcesResult {
    let mut resources: Vec<Resource> = vec![
        make_resource(
            "ariadne://overview",
            "Project Overview",
            Some("Project summary: file/edge counts, languages, layers, cycles"),
        ),
        make_resource(
            "ariadne://smells",
            "Architectural Smells",
            Some("Detected architectural issues and anti-patterns"),
        ),
        make_resource(
            "ariadne://hotspots",
            "Hotspots",
            Some("Top files ranked by combined importance (centrality + PageRank)"),
        ),
        make_resource(
            "ariadne://freshness",
            "Graph Freshness",
            Some("Graph staleness: confidence scores, stale/new/removed files"),
        ),
    ];

    // Dynamic: one resource per file
    for path in state.graph.nodes.keys() {
        resources.push(make_resource(
            &format!("ariadne://file/{}", path.as_str()),
            path.as_str(),
            Some("File metadata and dependency info"),
        ));
    }

    // Dynamic: one resource per cluster
    for cluster_id in state.clusters.clusters.keys() {
        resources.push(make_resource(
            &format!("ariadne://cluster/{}", cluster_id.as_str()),
            cluster_id.as_str(),
            Some("Cluster detail: files, metrics, internal edges"),
        ));
    }

    ListResourcesResult::with_all_items(resources)
}

/// Read a specific Ariadne resource by URI.
///
/// Dispatches on the URI scheme to produce JSON content.
pub fn read_resource_impl(uri: &str, state: &GraphState) -> ReadResourceResult {
    let json = match uri {
        "ariadne://overview" => read_overview(state),
        "ariadne://smells" => read_smells(state),
        "ariadne://hotspots" => read_hotspots(state),
        "ariadne://freshness" => read_freshness(state),
        _ if uri.starts_with("ariadne://file/") => {
            let path = &uri["ariadne://file/".len()..];
            read_file(path, state)
        }
        _ if uri.starts_with("ariadne://cluster/") => {
            let name = &uri["ariadne://cluster/".len()..];
            read_cluster(name, state)
        }
        _ => to_json_string(&serde_json::json!({
            "error": "unknown_resource",
            "uri": uri,
        })),
    };

    ReadResourceResult::new(vec![ResourceContents::TextResourceContents {
        uri: uri.to_string(),
        mime_type: Some("application/json".to_string()),
        text: json,
        meta: None,
    }])
}

// --- Internal helpers ---

fn make_resource(uri: &str, name: &str, description: Option<&str>) -> Resource {
    let mut raw = RawResource::new(uri, name);
    raw.description = description.map(|s| s.to_string());
    raw.mime_type = Some("application/json".to_string());
    Annotated::new(raw, None)
}

fn read_overview(state: &GraphState) -> String {
    let graph = &state.graph;
    let stats = &state.stats;

    let mut lang_counts: BTreeMap<String, usize> = BTreeMap::new();
    for path in graph.nodes.keys() {
        let ext = path
            .as_str()
            .rsplit('.')
            .next()
            .unwrap_or("unknown")
            .to_string();
        *lang_counts.entry(ext).or_default() += 1;
    }

    let mut layer_counts: BTreeMap<u32, usize> = BTreeMap::new();
    for node in graph.nodes.values() {
        *layer_counts.entry(node.arch_depth).or_default() += 1;
    }

    to_json_string(&serde_json::json!({
        "node_count": graph.nodes.len(),
        "edge_count": graph.edges.len(),
        "cluster_count": state.clusters.clusters.len(),
        "language_breakdown": lang_counts,
        "layer_distribution": layer_counts,
        "max_depth": stats.summary.max_depth,
        "bottleneck_files": stats.summary.bottleneck_files,
        "cycle_count": stats.sccs.len(),
        "orphan_files": stats.summary.orphan_files.len(),
        "freshness": {
            "hash_confidence": state.freshness.hash_confidence,
            "structural_confidence": state.freshness.structural_confidence,
        },
    }))
}

fn read_file(path: &str, state: &GraphState) -> String {
    let cp = crate::model::CanonicalPath::new(path);

    let node = match state.graph.nodes.get(&cp) {
        Some(n) => n,
        None => {
            return to_json_string(&serde_json::json!({
                "error": "not_found",
                "path": path,
                "suggestion": format!(
                    "File not in graph. Freshness: {:.0}%",
                    state.freshness.hash_confidence * 100.0
                ),
            }));
        }
    };

    let incoming_count = state
        .reverse_index
        .get(&cp)
        .map(|e| e.len())
        .unwrap_or(0);
    let outgoing_count = state
        .forward_index
        .get(&cp)
        .map(|e| e.len())
        .unwrap_or(0);
    let centrality = state.stats.centrality.get(path).copied();
    let importance = state.combined_importance.get(&cp).copied();

    to_json_string(&serde_json::json!({
        "path": path,
        "type": node.file_type.as_str(),
        "layer": node.layer.as_str(),
        "arch_depth": node.arch_depth,
        "lines": node.lines,
        "hash": node.hash.as_str(),
        "exports": node.exports.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
        "cluster": node.cluster.as_str(),
        "centrality": centrality,
        "combined_importance": importance,
        "incoming_edge_count": incoming_count,
        "outgoing_edge_count": outgoing_count,
    }))
}

fn read_cluster(name: &str, state: &GraphState) -> String {
    let cluster_id = crate::model::ClusterId::new(name);

    let cluster = match state.clusters.clusters.get(&cluster_id) {
        Some(c) => c,
        None => {
            return to_json_string(&serde_json::json!({
                "error": "not_found",
                "cluster": name,
            }));
        }
    };

    let metrics = state.cluster_metrics.get(&cluster_id);

    to_json_string(&serde_json::json!({
        "name": name,
        "file_count": cluster.files.len(),
        "files": cluster.files.iter().map(|f| f.as_str()).collect::<Vec<_>>(),
        "metrics": metrics.map(|m| serde_json::json!({
            "afferent_coupling": m.afferent_coupling,
            "efferent_coupling": m.efferent_coupling,
            "instability": m.instability,
            "abstractness": m.abstractness,
            "distance": m.distance,
        })),
    }))
}

fn read_smells(state: &GraphState) -> String {
    let semantic = state.semantic.as_ref().map(crate::serial::boundary_output_to_semantic_state);
    let smells = detect_smells(
        &state.graph,
        &state.stats,
        &state.clusters,
        &state.cluster_metrics,
        state.temporal.as_ref(),
        semantic.as_ref(),
    );

    let items: Vec<serde_json::Value> = smells
        .iter()
        .map(|s| {
            serde_json::json!({
                "smell_type": format!("{:?}", s.smell_type),
                "severity": format!("{:?}", s.severity),
                "explanation": s.explanation,
                "files": s.files.iter().map(|f| f.as_str()).collect::<Vec<_>>(),
                "metrics": {
                    "primary_value": s.metrics.primary_value,
                    "threshold": s.metrics.threshold,
                },
            })
        })
        .collect();

    to_json_string(&serde_json::json!({
        "smell_count": items.len(),
        "smells": items,
    }))
}

fn read_hotspots(state: &GraphState) -> String {
    let mut entries: Vec<(&crate::model::CanonicalPath, &f64)> =
        state.combined_importance.iter().collect();
    entries.sort_by(|a, b| b.1.partial_cmp(a.1).unwrap_or(std::cmp::Ordering::Equal));
    entries.truncate(10);

    let items: Vec<serde_json::Value> = entries
        .iter()
        .map(|(path, score)| {
            let centrality = state.stats.centrality.get(path.as_str()).copied();
            let pagerank = state.pagerank.get(*path).copied();
            serde_json::json!({
                "path": path.as_str(),
                "combined_importance": score,
                "centrality": centrality,
                "pagerank": pagerank,
            })
        })
        .collect();

    to_json_string(&serde_json::json!({
        "top_count": items.len(),
        "hotspots": items,
    }))
}

fn read_freshness(state: &GraphState) -> String {
    to_json_string(&serde_json::json!({
        "hash_confidence": state.freshness.hash_confidence,
        "structural_confidence": state.freshness.structural_confidence,
        "stale_files": state.freshness.stale_files.iter().map(|p| p.as_str()).collect::<Vec<_>>(),
        "structurally_changed": state.freshness.structurally_changed.iter().map(|p| p.as_str()).collect::<Vec<_>>(),
        "new_files": state.freshness.new_files.iter().map(|p| p.display().to_string()).collect::<Vec<_>>(),
        "removed_files": state.freshness.removed_files.iter().map(|p| p.as_str()).collect::<Vec<_>>(),
    }))
}

/// Serialize to pretty JSON, returning error string on failure.
fn to_json_string<T: serde::Serialize>(value: &T) -> String {
    serde_json::to_string_pretty(value).unwrap_or_else(|e| {
        format!(
            "{{\"error\":\"serialization_failed\",\"reason\":\"{}\"}}",
            e
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn make_resource_sets_fields() {
        let r = make_resource("ariadne://overview", "Overview", Some("desc"));
        assert_eq!(r.uri, "ariadne://overview");
        assert_eq!(r.name, "Overview");
        assert_eq!(r.description.as_deref(), Some("desc"));
        assert_eq!(r.mime_type.as_deref(), Some("application/json"));
        assert!(r.annotations.is_none());
    }

    #[test]
    fn unknown_uri_returns_error_json() {
        let json = to_json_string(&serde_json::json!({
            "error": "unknown_resource",
            "uri": "ariadne://nonexistent",
        }));
        assert!(json.contains("unknown_resource"));
    }

    /// Build a minimal GraphState with two files and one cluster for resource tests.
    fn make_test_state() -> GraphState {
        use crate::model::*;

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
                symbols: Vec::new(),
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
                symbols: Vec::new(),
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

        GraphState::from_loaded_data(graph, stats, clusters, BTreeMap::new(), None, None)
    }

    #[test]
    fn list_resources_returns_all_static_plus_dynamic() {
        let state = make_test_state();
        let result = list_resources_impl(&state);
        // 4 static + 2 files + 1 cluster = 7
        assert_eq!(
            result.resources.len(),
            7,
            "Expected 4 static + 2 file + 1 cluster resources"
        );

        let uris: Vec<&str> = result
            .resources
            .iter()
            .map(|r| r.uri.as_str())
            .collect();
        assert!(uris.contains(&"ariadne://overview"));
        assert!(uris.contains(&"ariadne://smells"));
        assert!(uris.contains(&"ariadne://hotspots"));
        assert!(uris.contains(&"ariadne://freshness"));
        assert!(uris.contains(&"ariadne://file/src/a.ts"));
        assert!(uris.contains(&"ariadne://file/src/b.ts"));
        assert!(uris.contains(&"ariadne://cluster/src"));
    }

    #[test]
    fn read_resource_overview_returns_valid_json() {
        let state = make_test_state();
        let result = read_resource_impl("ariadne://overview", &state);
        assert_eq!(result.contents.len(), 1);
        if let ResourceContents::TextResourceContents { ref text, .. } = result.contents[0] {
            let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
            assert_eq!(parsed["node_count"], 2);
            assert_eq!(parsed["edge_count"], 1);
            assert_eq!(parsed["cluster_count"], 1);
        } else {
            panic!("Expected TextResourceContents");
        }
    }

    #[test]
    fn read_resource_file_found() {
        let state = make_test_state();
        let result = read_resource_impl("ariadne://file/src/a.ts", &state);
        assert_eq!(result.contents.len(), 1);
        if let ResourceContents::TextResourceContents { ref text, .. } = result.contents[0] {
            let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
            assert_eq!(parsed["path"], "src/a.ts");
            assert_eq!(parsed["lines"], 100);
            assert_eq!(parsed["arch_depth"], 1);
            assert!(parsed["hash"].as_str().is_some());
        } else {
            panic!("Expected TextResourceContents");
        }
    }

    #[test]
    fn read_resource_file_not_found() {
        let state = make_test_state();
        let result = read_resource_impl("ariadne://file/nonexistent.ts", &state);
        assert_eq!(result.contents.len(), 1);
        if let ResourceContents::TextResourceContents { ref text, .. } = result.contents[0] {
            let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
            assert_eq!(parsed["error"], "not_found");
            assert_eq!(parsed["path"], "nonexistent.ts");
        } else {
            panic!("Expected TextResourceContents");
        }
    }

    #[test]
    fn read_resource_cluster_found() {
        let state = make_test_state();
        let result = read_resource_impl("ariadne://cluster/src", &state);
        assert_eq!(result.contents.len(), 1);
        if let ResourceContents::TextResourceContents { ref text, .. } = result.contents[0] {
            let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
            assert_eq!(parsed["name"], "src");
            assert_eq!(parsed["file_count"], 2);
            assert!(parsed["files"].as_array().is_some());
            assert!(parsed["metrics"].is_object());
        } else {
            panic!("Expected TextResourceContents");
        }
    }

    #[test]
    fn read_resource_cluster_not_found() {
        let state = make_test_state();
        let result = read_resource_impl("ariadne://cluster/nonexistent", &state);
        assert_eq!(result.contents.len(), 1);
        if let ResourceContents::TextResourceContents { ref text, .. } = result.contents[0] {
            let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
            assert_eq!(parsed["error"], "not_found");
            assert_eq!(parsed["cluster"], "nonexistent");
        } else {
            panic!("Expected TextResourceContents");
        }
    }

    #[test]
    fn read_resource_hotspots_returns_sorted() {
        let state = make_test_state();
        let result = read_resource_impl("ariadne://hotspots", &state);
        assert_eq!(result.contents.len(), 1);
        if let ResourceContents::TextResourceContents { ref text, .. } = result.contents[0] {
            let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
            assert!(parsed["hotspots"].as_array().is_some());
            // Verify sorted descending by combined_importance
            let hotspots = parsed["hotspots"].as_array().unwrap();
            for window in hotspots.windows(2) {
                let a = window[0]["combined_importance"].as_f64().unwrap();
                let b = window[1]["combined_importance"].as_f64().unwrap();
                assert!(a >= b, "Hotspots should be sorted descending");
            }
        } else {
            panic!("Expected TextResourceContents");
        }
    }

    #[test]
    fn read_resource_freshness_returns_confidence() {
        let state = make_test_state();
        let result = read_resource_impl("ariadne://freshness", &state);
        assert_eq!(result.contents.len(), 1);
        if let ResourceContents::TextResourceContents { ref text, .. } = result.contents[0] {
            let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
            assert!(parsed["hash_confidence"].as_f64().is_some());
            assert!(parsed["structural_confidence"].as_f64().is_some());
            // Fresh state should have 1.0 confidence
            assert_eq!(parsed["hash_confidence"].as_f64().unwrap(), 1.0);
            assert_eq!(parsed["structural_confidence"].as_f64().unwrap(), 1.0);
        } else {
            panic!("Expected TextResourceContents");
        }
    }

    #[test]
    fn read_resource_unknown_uri_returns_error() {
        let state = make_test_state();
        let result = read_resource_impl("ariadne://totally-unknown", &state);
        assert_eq!(result.contents.len(), 1);
        if let ResourceContents::TextResourceContents { ref text, .. } = result.contents[0] {
            let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
            assert_eq!(parsed["error"], "unknown_resource");
            assert_eq!(parsed["uri"], "ariadne://totally-unknown");
        } else {
            panic!("Expected TextResourceContents");
        }
    }
}
