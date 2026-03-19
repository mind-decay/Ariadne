use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use arc_swap::ArcSwap;
use rmcp::handler::server::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::*;
use rmcp::{tool, tool_handler, tool_router, ServerHandler};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::algo;
use crate::analysis::smells::detect_smells;
use crate::mcp::state::GraphState;
use crate::model::CanonicalPath;

/// MCP tool handler struct. Each tool is a thin wrapper around existing algo functions.
#[derive(Debug, Clone)]
pub struct AriadneTools {
    pub state: Arc<ArcSwap<GraphState>>,
    pub rebuilding: Arc<AtomicBool>,
    pub project_root: PathBuf,
    tool_router: ToolRouter<Self>,
}

impl AriadneTools {
    pub fn new(
        state: Arc<ArcSwap<GraphState>>,
        rebuilding: Arc<AtomicBool>,
        project_root: PathBuf,
    ) -> Self {
        Self {
            state,
            rebuilding,
            project_root,
            tool_router: Self::tool_router(),
        }
    }
}

#[tool_handler]
impl ServerHandler for AriadneTools {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_instructions("Ariadne structural dependency graph engine")
    }
}

// --- Tool parameter types ---

#[derive(Debug, Deserialize, JsonSchema)]
pub struct FileParam {
    /// File path relative to project root
    pub path: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct BlastRadiusParam {
    /// File path relative to project root
    pub path: String,
    /// Maximum BFS depth (optional, default unbounded)
    pub depth: Option<u32>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SubgraphParam {
    /// Center file paths
    pub paths: Vec<String>,
    /// BFS depth (default 2)
    pub depth: Option<u32>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CentralityParam {
    /// Minimum centrality threshold (default 0.0)
    pub min: Option<f64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct LayerParam {
    /// Filter to a specific layer depth (optional)
    pub layer: Option<u32>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ClusterParam {
    /// Cluster name
    pub name: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DependenciesParam {
    /// File path relative to project root
    pub path: String,
    /// Direction: "in", "out", or "both"
    pub direction: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ViewsExportParam {
    /// View level: "L0", "L1", or "L2"
    pub level: String,
    /// Cluster name (required for L1, ignored for L0)
    pub cluster: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SmellsParam {
    /// Minimum severity filter: "high", "medium", or "low" (default: all)
    pub min_severity: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ImportanceParam {
    /// Number of top files to return (default: 20)
    pub top: Option<u32>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CompressedParam {
    /// Compression level: 0 (project), 1 (cluster), 2 (file)
    pub level: u32,
    /// Focus: cluster name (level 1) or file path (level 2)
    pub focus: Option<String>,
    /// BFS depth for level 2 (default: 2)
    pub depth: Option<u32>,
}

#[tool_router]
impl AriadneTools {
    // --- T1: Overview ---

    #[tool(
        name = "ariadne_overview",
        description = "Project summary: node/edge counts, language breakdown, layer distribution, critical files, cycles count, max depth"
    )]
    fn overview(&self) -> String {
        let state = self.state.load();
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

        let result = serde_json::json!({
            "node_count": graph.nodes.len(),
            "edge_count": graph.edges.len(),
            "cluster_count": state.clusters.clusters.len(),
            "language_breakdown": lang_counts,
            "layer_distribution": layer_counts,
            "max_depth": stats.summary.max_depth,
            "bottleneck_files": stats.summary.bottleneck_files,
            "cycle_count": stats.sccs.len(),
            "orphan_files": stats.summary.orphan_files.len(),
            "rebuilding": self.rebuilding.load(Ordering::Relaxed),
            "freshness": {
                "hash_confidence": state.freshness.hash_confidence,
                "structural_confidence": state.freshness.structural_confidence,
            },
        });

        to_json(&result)
    }

    // --- T2: File detail ---

    #[tool(
        name = "ariadne_file",
        description = "File detail: type, layer, arch_depth, exports, cluster, centrality, incoming/outgoing edges"
    )]
    fn file(&self, Parameters(params): Parameters<FileParam>) -> String {
        let state = self.state.load();
        let cp = CanonicalPath::new(&params.path);

        let node = match state.graph.nodes.get(&cp) {
            Some(n) => n,
            None => {
                let result = serde_json::json!({
                    "error": "not_found",
                    "path": params.path,
                    "suggestion": format!("File may be new. Graph freshness: {:.0}%",
                        state.freshness.hash_confidence * 100.0),
                });
                return to_json(&result);
            }
        };

        let incoming: Vec<serde_json::Value> = state
            .reverse_index
            .get(&cp)
            .map(|edges| edges.iter().map(edge_to_json).collect())
            .unwrap_or_default();

        let outgoing: Vec<serde_json::Value> = state
            .forward_index
            .get(&cp)
            .map(|edges| edges.iter().map(edge_to_json).collect())
            .unwrap_or_default();

        let centrality = state.stats.centrality.get(params.path.as_str()).copied();

        let result = serde_json::json!({
            "path": params.path,
            "type": node.file_type.as_str(),
            "layer": node.layer.as_str(),
            "arch_depth": node.arch_depth,
            "lines": node.lines,
            "hash": node.hash.as_str(),
            "exports": node.exports.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
            "cluster": node.cluster.as_str(),
            "centrality": centrality,
            "incoming_edges": incoming,
            "outgoing_edges": outgoing,
        });

        to_json(&result)
    }

    // --- T3: Blast radius ---

    #[tool(
        name = "ariadne_blast_radius",
        description = "Reverse BFS: map of affected files with distances from the given file"
    )]
    fn blast_radius(&self, Parameters(params): Parameters<BlastRadiusParam>) -> String {
        let state = self.state.load();
        let cp = CanonicalPath::new(&params.path);
        let result = algo::blast_radius::blast_radius(&state.graph, &cp, params.depth);

        let json_result: BTreeMap<String, u32> = result
            .iter()
            .map(|(k, &v)| (k.as_str().to_string(), v))
            .collect();

        to_json(&json_result)
    }

    // --- T4: Subgraph ---

    #[tool(
        name = "ariadne_subgraph",
        description = "Extract filtered subgraph: nodes + edges + clusters in the neighborhood of given files"
    )]
    fn subgraph(&self, Parameters(params): Parameters<SubgraphParam>) -> String {
        let state = self.state.load();
        let paths: Vec<CanonicalPath> = params.paths.iter().map(CanonicalPath::new).collect();
        let depth = params.depth.unwrap_or(2);
        let result = algo::subgraph::extract_subgraph(&state.graph, &paths, depth);

        let nodes: BTreeMap<String, serde_json::Value> = result
            .nodes
            .iter()
            .map(|(path, node)| {
                (
                    path.as_str().to_string(),
                    serde_json::json!({
                        "type": node.file_type.as_str(),
                        "layer": node.layer.as_str(),
                        "arch_depth": node.arch_depth,
                        "cluster": node.cluster.as_str(),
                    }),
                )
            })
            .collect();

        let edges: Vec<serde_json::Value> = result.edges.iter().map(edge_to_json).collect();

        let json = serde_json::json!({
            "nodes": nodes,
            "edges": edges,
            "center_files": result.center_files.iter().map(|p| p.as_str()).collect::<Vec<_>>(),
            "depth": result.depth,
        });

        to_json(&json)
    }

    // --- T5: Centrality ---

    #[tool(
        name = "ariadne_centrality",
        description = "Bottleneck files sorted by betweenness centrality score"
    )]
    fn centrality(&self, Parameters(params): Parameters<CentralityParam>) -> String {
        let state = self.state.load();
        let min = params.min.unwrap_or(0.0);

        let mut filtered: Vec<(&String, &f64)> = state
            .stats
            .centrality
            .iter()
            .filter(|(_, &v)| v >= min)
            .collect();
        filtered.sort_by(|a, b| {
            b.1.partial_cmp(a.1)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.0.cmp(b.0))
        });

        let result: Vec<serde_json::Value> = filtered
            .iter()
            .map(|(path, &score)| serde_json::json!({"path": path, "centrality": score}))
            .collect();

        to_json(&result)
    }

    // --- T6: Cycles ---

    #[tool(
        name = "ariadne_cycles",
        description = "All strongly connected components (circular dependencies)"
    )]
    fn cycles(&self) -> String {
        let state = self.state.load();
        to_json(&state.stats.sccs)
    }

    // --- T7: Layers ---

    #[tool(
        name = "ariadne_layers",
        description = "Topological layers: files grouped by architectural depth level"
    )]
    fn layers(&self, Parameters(params): Parameters<LayerParam>) -> String {
        let state = self.state.load();

        if let Some(layer) = params.layer {
            let files: Vec<String> = state
                .layer_index
                .get(&layer)
                .map(|paths| paths.iter().map(|p| p.as_str().to_string()).collect())
                .unwrap_or_default();
            let result = serde_json::json!({ "layer": layer, "files": files });
            to_json(&result)
        } else {
            let result: BTreeMap<u32, Vec<String>> = state
                .layer_index
                .iter()
                .map(|(&depth, paths)| {
                    (
                        depth,
                        paths.iter().map(|p| p.as_str().to_string()).collect(),
                    )
                })
                .collect();
            to_json(&result)
        }
    }

    // --- T8: Cluster ---

    #[tool(
        name = "ariadne_cluster",
        description = "Cluster detail: files, internal/external deps, cohesion"
    )]
    fn cluster(&self, Parameters(params): Parameters<ClusterParam>) -> String {
        let state = self.state.load();
        let cluster_id = crate::model::ClusterId::new(&params.name);

        match state.clusters.clusters.get(&cluster_id) {
            Some(cluster) => {
                let result = serde_json::json!({
                    "name": params.name,
                    "files": cluster.files.iter().map(|p| p.as_str().to_string()).collect::<Vec<_>>(),
                    "file_count": cluster.file_count,
                    "internal_edges": cluster.internal_edges,
                    "external_edges": cluster.external_edges,
                    "cohesion": cluster.cohesion,
                });
                to_json(&result)
            }
            None => {
                let result = serde_json::json!({
                    "error": "not_found",
                    "cluster": params.name,
                    "available_clusters": state.clusters.clusters.keys().map(|k| k.as_str()).collect::<Vec<_>>(),
                });
                to_json(&result)
            }
        }
    }

    // --- T9: Dependencies ---

    #[tool(
        name = "ariadne_dependencies",
        description = "Direct dependencies of a file. Direction: 'in', 'out', or 'both'"
    )]
    fn dependencies(&self, Parameters(params): Parameters<DependenciesParam>) -> String {
        let state = self.state.load();
        let cp = CanonicalPath::new(&params.path);

        let incoming: Vec<serde_json::Value> =
            if params.direction == "in" || params.direction == "both" {
                state
                    .reverse_index
                    .get(&cp)
                    .map(|edges| edges.iter().map(edge_to_json).collect())
                    .unwrap_or_default()
            } else {
                vec![]
            };

        let outgoing: Vec<serde_json::Value> =
            if params.direction == "out" || params.direction == "both" {
                state
                    .forward_index
                    .get(&cp)
                    .map(|edges| edges.iter().map(edge_to_json).collect())
                    .unwrap_or_default()
            } else {
                vec![]
            };

        let result = serde_json::json!({
            "path": params.path,
            "direction": params.direction,
            "incoming": incoming,
            "outgoing": outgoing,
        });

        to_json(&result)
    }

    // --- T10: Freshness ---

    #[tool(
        name = "ariadne_freshness",
        description = "Graph freshness: overall confidence, stale files list, last update time"
    )]
    fn freshness(&self) -> String {
        let state = self.state.load();
        let f = &state.freshness;

        let result = serde_json::json!({
            "hash_confidence": f.hash_confidence,
            "structural_confidence": f.structural_confidence,
            "stale_files": f.stale_files.iter().map(|p| p.as_str()).collect::<Vec<_>>(),
            "structurally_changed": f.structurally_changed.iter().map(|p| p.as_str()).collect::<Vec<_>>(),
            "new_files": f.new_files.iter().map(|p| p.display().to_string()).collect::<Vec<_>>(),
            "removed_files": f.removed_files.iter().map(|p| p.as_str()).collect::<Vec<_>>(),
            "rebuilding": self.rebuilding.load(Ordering::Relaxed),
            "total_files": state.graph.nodes.len(),
        });

        to_json(&result)
    }

    // --- T12: Metrics ---

    #[tool(
        name = "ariadne_metrics",
        description = "Martin metrics per cluster: instability, abstractness, distance from main sequence, zone classification"
    )]
    fn metrics(&self) -> String {
        let state = self.state.load();
        to_json(&state.cluster_metrics)
    }

    // --- T13: Smells ---

    #[tool(
        name = "ariadne_smells",
        description = "Detect architectural smells: god files, circular dependencies, layer violations, hub-and-spoke, unstable foundations, dead clusters, shotgun surgery"
    )]
    fn smells(&self, Parameters(params): Parameters<SmellsParam>) -> String {
        let state = self.state.load();
        let smells = detect_smells(
            &state.graph,
            &state.stats,
            &state.clusters,
            &state.cluster_metrics,
        );

        let filtered: Vec<_> = if let Some(ref min_sev) = params.min_severity {
            let min = crate::model::SmellSeverity::from_str_loose(min_sev);
            smells
                .into_iter()
                .filter(|s| s.severity.level() >= min.level())
                .collect()
        } else {
            smells
        };

        to_json(&filtered)
    }

    // --- T14: Diff ---

    #[tool(
        name = "ariadne_diff",
        description = "Structural diff since last auto-update: added/removed nodes and edges, new/resolved cycles, new/resolved smells"
    )]
    fn diff(&self) -> String {
        let state = self.state.load();
        match &state.last_diff {
            Some(diff) => to_json(diff),
            None => "null".to_string(),
        }
    }

    // --- T15: Importance ---

    #[tool(
        name = "ariadne_importance",
        description = "Files ranked by combined importance score (betweenness centrality + PageRank). Returns top N files."
    )]
    fn importance(&self, Parameters(params): Parameters<ImportanceParam>) -> String {
        let state = self.state.load();
        let top = params.top.unwrap_or(20) as usize;

        let mut ranked: Vec<_> = state.combined_importance.iter().collect();
        ranked.sort_by(|a, b| {
            b.1.partial_cmp(a.1)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.0.cmp(b.0))
        });
        ranked.truncate(top);

        let result: Vec<serde_json::Value> = ranked
            .iter()
            .map(|(path, &score)| {
                serde_json::json!({
                    "path": path.as_str(),
                    "combined_score": score,
                    "centrality": state.stats.centrality.get(path.as_str()).copied().unwrap_or(0.0),
                    "pagerank": state.pagerank.get(*path).copied().unwrap_or(0.0),
                })
            })
            .collect();

        to_json(&result)
    }

    // --- T16: Compressed ---

    #[tool(
        name = "ariadne_compressed",
        description = "Hierarchical graph compression. Level 0: project overview (clusters). Level 1: cluster detail (files). Level 2: file neighborhood."
    )]
    fn compressed(&self, Parameters(params): Parameters<CompressedParam>) -> String {
        let state = self.state.load();

        match params.level {
            0 => to_json(&state.compressed_l0),
            1 => {
                let focus = match &params.focus {
                    Some(f) => f,
                    None => {
                        let result = serde_json::json!({
                            "error": "missing_focus",
                            "message": "Level 1 requires a 'focus' parameter (cluster name)",
                            "available_clusters": state.clusters.clusters.keys()
                                .map(|k| k.as_str()).collect::<Vec<_>>(),
                        });
                        return to_json(&result);
                    }
                };
                let cluster_id = crate::model::ClusterId::new(focus);
                match algo::compress::compress_l1(
                    &state.graph,
                    &state.clusters,
                    &state.stats,
                    &cluster_id,
                ) {
                    Ok(cg) => to_json(&cg),
                    Err(e) => {
                        let result = serde_json::json!({
                            "error": "not_found",
                            "message": e,
                            "available_clusters": state.clusters.clusters.keys()
                                .map(|k| k.as_str()).collect::<Vec<_>>(),
                        });
                        to_json(&result)
                    }
                }
            }
            2 => {
                let focus = match &params.focus {
                    Some(f) => f,
                    None => {
                        let result = serde_json::json!({
                            "error": "missing_focus",
                            "message": "Level 2 requires a 'focus' parameter (file path)",
                        });
                        return to_json(&result);
                    }
                };
                let cp = CanonicalPath::new(focus);
                let depth = params.depth.unwrap_or(2);
                match algo::compress::compress_l2(
                    &state.graph,
                    &state.clusters,
                    &state.stats,
                    &cp,
                    depth,
                ) {
                    Ok(cg) => to_json(&cg),
                    Err(e) => {
                        let result = serde_json::json!({
                            "error": "not_found",
                            "message": e,
                        });
                        to_json(&result)
                    }
                }
            }
            _ => {
                let result = serde_json::json!({
                    "error": "invalid_level",
                    "message": "Level must be 0, 1, or 2",
                });
                to_json(&result)
            }
        }
    }

    // --- T17: Spectral ---

    #[tool(
        name = "ariadne_spectral",
        description = "Spectral analysis: algebraic connectivity (λ₂), monolith score, natural graph bisection via Fiedler vector"
    )]
    fn spectral(&self) -> String {
        let state = self.state.load();
        to_json(&state.spectral)
    }

    // --- T11: Views export ---

    #[tool(
        name = "ariadne_views_export",
        description = "Pre-generated markdown views from .ariadne/views/. Generic markdown for any consumer."
    )]
    fn views_export(&self, Parameters(params): Parameters<ViewsExportParam>) -> String {
        let views_dir = self.project_root.join(".ariadne").join("views");

        match params.level.as_str() {
            "L0" => {
                let path = views_dir.join("index.md");
                std::fs::read_to_string(&path).unwrap_or_else(|_| {
                    "L0 index view not generated. Run `ariadne views generate` first.".to_string()
                })
            }
            "L1" => {
                if let Some(cluster) = &params.cluster {
                    let path = views_dir.join(format!("{}.md", cluster));
                    std::fs::read_to_string(&path)
                        .unwrap_or_else(|_| format!("L1 cluster view '{}' not found.", cluster))
                } else {
                    let mut views = Vec::new();
                    if let Ok(entries) = std::fs::read_dir(&views_dir) {
                        for entry in entries.flatten() {
                            let name = entry.file_name().to_string_lossy().to_string();
                            if name.ends_with(".md") && name != "index.md" {
                                views.push(name.trim_end_matches(".md").to_string());
                            }
                        }
                    }
                    views.sort();
                    format!(
                        "Available L1 cluster views: {}\nSpecify cluster parameter to view one.",
                        views.join(", ")
                    )
                }
            }
            "L2" => {
                "L2 impact views are generated on-demand via ariadne_blast_radius tool.".to_string()
            }
            _ => format!("Unknown level '{}'. Use L0, L1, or L2.", params.level),
        }
    }
}

/// Serialize to pretty JSON, returning error string on failure instead of panicking.
fn to_json<T: serde::Serialize>(value: &T) -> String {
    serde_json::to_string_pretty(value).unwrap_or_else(|e| {
        format!(
            "{{\"error\":\"serialization_failed\",\"reason\":\"{}\"}}",
            e
        )
    })
}

/// Helper: convert Edge to JSON value.
fn edge_to_json(e: &crate::model::Edge) -> serde_json::Value {
    serde_json::json!({
        "from": e.from.as_str(),
        "to": e.to.as_str(),
        "type": e.edge_type.as_str(),
        "symbols": e.symbols.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
    })
}
