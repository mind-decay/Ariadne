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
use crate::algo::context::{assemble_context, TaskType};
use crate::algo::impact::analyze_impact;
use crate::algo::reading_order::compute_reading_order;
use crate::algo::test_map::find_tests_for;
use crate::analysis::smells::detect_smells;
use crate::mcp::state::GraphState;
use crate::mcp::tools_context::{ContextParam, PlanImpactParam, ReadingOrderParam, TestsForParam};
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
    /// Optional symbol name for symbol-level blast radius
    pub symbol: Option<String>,
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

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SymbolsParam {
    /// File path relative to project root
    pub path: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SymbolSearchParam {
    /// Search query (case-insensitive substring match)
    pub query: String,
    /// Optional kind filter: function, method, class, struct, interface, trait, type, enum, const, variable, module
    pub kind: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SymbolBlastRadiusParam {
    /// File path relative to project root
    pub path: String,
    /// Symbol name to trace
    pub symbol: String,
    /// BFS depth (default 3, max 10)
    pub depth: Option<u32>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CallersParam {
    /// File path relative to project root
    pub path: String,
    /// Symbol name to look up callers for
    pub symbol: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CalleesParam {
    /// File path relative to project root
    pub path: String,
    /// Symbol name to look up callees for
    pub symbol: String,
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

        let symbols: Vec<serde_json::Value> = state
            .symbol_index
            .symbols_for_file(&cp)
            .unwrap_or_default()
            .iter()
            .map(|s| {
                let mut obj = serde_json::json!({
                    "name": s.name,
                    "kind": s.kind,
                    "visibility": s.visibility,
                    "span": { "start": s.span.start, "end": s.span.end },
                });
                if let Some(ref sig) = s.signature {
                    obj["signature"] = serde_json::Value::String(sig.clone());
                }
                if let Some(ref parent) = s.parent {
                    obj["parent"] = serde_json::Value::String(parent.clone());
                }
                obj
            })
            .collect();

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
            "symbols": symbols,
        });

        to_json(&result)
    }

    // --- T3: Blast radius ---

    #[tool(
        name = "ariadne_blast_radius",
        description = "Reverse BFS: map of affected files with distances from the given file. When 'symbol' is provided, traces symbol-level blast radius instead."
    )]
    fn blast_radius(&self, Parameters(params): Parameters<BlastRadiusParam>) -> String {
        let state = self.state.load();
        let cp = CanonicalPath::new(&params.path);

        // When symbol is provided, delegate to symbol blast radius logic
        if let Some(ref symbol) = params.symbol {
            if !state.graph.nodes.contains_key(&cp) {
                let result = serde_json::json!({
                    "error": "not_found",
                    "path": params.path,
                });
                return to_json(&result);
            }

            let sym_exists = state
                .symbol_index
                .symbols_for_file(&cp)
                .map(|syms| syms.iter().any(|s| s.name == *symbol))
                .unwrap_or(false);

            if !sym_exists {
                let result = serde_json::json!({
                    "error": "symbol_not_found",
                    "path": params.path,
                    "symbol": symbol,
                    "suggestion": "Use ariadne_symbols to list symbols in this file",
                });
                return to_json(&result);
            }

            let max_depth = params.depth.unwrap_or(3).min(10);
            let sbr_params = SymbolBlastRadiusParam {
                path: params.path,
                symbol: symbol.clone(),
                depth: Some(max_depth),
            };
            return self.symbol_blast_radius(Parameters(sbr_params));
        }

        // File-level blast radius — check file exists in graph first
        if !state.graph.nodes.contains_key(&cp) {
            let result = serde_json::json!({
                "error": "not_found",
                "path": params.path,
                "suggestion": format!("File may be new. Graph freshness: {:.0}%",
                    state.freshness.hash_confidence * 100.0),
            });
            return to_json(&result);
        }

        let index = algo::AdjacencyIndex::build(&state.graph.edges, algo::is_architectural);
        let result = algo::blast_radius::blast_radius(&state.graph, &cp, params.depth, &index);

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

        // Validate direction parameter
        match params.direction.as_str() {
            "in" | "out" | "both" => {}
            other => {
                let result = serde_json::json!({
                    "error": "invalid_direction",
                    "value": other,
                    "valid_values": ["in", "out", "both"],
                    "message": "Direction must be one of: 'in', 'out', 'both'",
                });
                return to_json(&result);
            }
        }

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

        // Build symbol_edges from call graph
        let symbol_edges: Vec<serde_json::Value> = {
            let mut edges_out: Vec<serde_json::Value> = Vec::new();

            if params.direction == "out" || params.direction == "both" {
                for (sym_name, call_edges) in state.call_graph.all_callees_for_file(&cp) {
                    for ce in call_edges {
                        edges_out.push(serde_json::json!({
                            "direction": "out",
                            "symbol": sym_name,
                            "target_file": ce.file.as_str(),
                            "target_symbol": ce.symbol,
                            "edge_kind": ce.edge_kind.as_str(),
                        }));
                    }
                }
            }

            if params.direction == "in" || params.direction == "both" {
                for (sym_name, call_edges) in state.call_graph.all_callers_for_file(&cp) {
                    for ce in call_edges {
                        edges_out.push(serde_json::json!({
                            "direction": "in",
                            "symbol": sym_name,
                            "source_file": ce.file.as_str(),
                            "source_symbol": ce.symbol,
                            "edge_kind": ce.edge_kind.as_str(),
                        }));
                    }
                }
            }

            // Sort for determinism
            edges_out.sort_by(|a, b| {
                let a_str = a.to_string();
                let b_str = b.to_string();
                a_str.cmp(&b_str)
            });
            edges_out
        };

        let result = serde_json::json!({
            "path": params.path,
            "direction": params.direction,
            "incoming": incoming,
            "outgoing": outgoing,
            "symbol_edges": symbol_edges,
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
                    Ok(cg) => {
                        // Enhance L2 with symbol_edges from call graph
                        let symbol_edges: Vec<serde_json::Value> = {
                            let mut edges_out = Vec::new();
                            for (sym_name, call_edges) in
                                state.call_graph.all_callees_for_file(&cp)
                            {
                                for ce in call_edges {
                                    edges_out.push(serde_json::json!({
                                        "direction": "out",
                                        "symbol": sym_name,
                                        "target_file": ce.file.as_str(),
                                        "target_symbol": ce.symbol,
                                        "edge_kind": ce.edge_kind.as_str(),
                                    }));
                                }
                            }
                            for (sym_name, call_edges) in
                                state.call_graph.all_callers_for_file(&cp)
                            {
                                for ce in call_edges {
                                    edges_out.push(serde_json::json!({
                                        "direction": "in",
                                        "symbol": sym_name,
                                        "source_file": ce.file.as_str(),
                                        "source_symbol": ce.symbol,
                                        "edge_kind": ce.edge_kind.as_str(),
                                    }));
                                }
                            }
                            edges_out.sort_by_key(|a| a.to_string());
                            edges_out
                        };

                        // Serialize compressed graph then merge symbol_edges
                        let mut json_val = serde_json::to_value(&cg)
                            .unwrap_or_else(|_| serde_json::json!({}));
                        if !symbol_edges.is_empty() {
                            if let serde_json::Value::Object(ref mut map) = json_val {
                                map.insert(
                                    "symbol_edges".to_string(),
                                    serde_json::Value::Array(symbol_edges),
                                );
                            }
                        }
                        serde_json::to_string_pretty(&json_val).unwrap_or_else(|e| {
                            format!(
                                "{{\"error\":\"serialization_failed\",\"reason\":\"{}\"}}",
                                e
                            )
                        })
                    }
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

    // --- T18: Symbols ---

    #[tool(
        name = "ariadne_symbols",
        description = "Symbol definitions in a file: functions, classes, methods, interfaces, constants with visibility and line spans"
    )]
    fn symbols(&self, Parameters(params): Parameters<SymbolsParam>) -> String {
        let state = self.state.load();
        let cp = CanonicalPath::new(&params.path);

        if !state.graph.nodes.contains_key(&cp) {
            let result = serde_json::json!({
                "error": "not_found",
                "path": params.path,
                "suggestion": format!("File not in graph. Graph freshness: {:.0}%",
                    state.freshness.hash_confidence * 100.0),
            });
            return to_json(&result);
        }

        let symbols = state
            .symbol_index
            .symbols_for_file(&cp)
            .unwrap_or_default();

        let sym_json: Vec<serde_json::Value> = symbols
            .iter()
            .map(|s| {
                let mut obj = serde_json::json!({
                    "name": s.name,
                    "kind": s.kind,
                    "visibility": s.visibility,
                    "span": { "start": s.span.start, "end": s.span.end },
                });
                if let Some(ref sig) = s.signature {
                    obj["signature"] = serde_json::Value::String(sig.clone());
                }
                if let Some(ref parent) = s.parent {
                    obj["parent"] = serde_json::Value::String(parent.clone());
                }
                obj
            })
            .collect();

        let result = serde_json::json!({
            "file": params.path,
            "symbols": sym_json,
        });

        to_json(&result)
    }

    // --- T19: Symbol search ---

    #[tool(
        name = "ariadne_symbol_search",
        description = "Search symbols by name across the entire project. Case-insensitive substring match with optional kind filter."
    )]
    fn symbol_search(&self, Parameters(params): Parameters<SymbolSearchParam>) -> String {
        if params.query.trim().is_empty() {
            let result = serde_json::json!({
                "error": "empty_query",
                "message": "Query must be a non-empty string",
            });
            return to_json(&result);
        }

        let state = self.state.load();

        let kind_filter = params.kind.as_deref().and_then(parse_symbol_kind);

        let matches = state.symbol_index.search(&params.query, kind_filter);

        let match_json: Vec<serde_json::Value> = matches
            .iter()
            .map(|loc| {
                serde_json::json!({
                    "file": loc.file.as_str(),
                    "name": loc.name,
                    "kind": loc.kind,
                    "span": { "start": loc.span.start, "end": loc.span.end },
                })
            })
            .collect();

        let result = serde_json::json!({
            "query": params.query,
            "kind": params.kind,
            "matches": match_json,
            "count": match_json.len(),
        });

        to_json(&result)
    }

    // --- T20: Symbol blast radius ---

    #[tool(
        name = "ariadne_symbol_blast_radius",
        description = "Trace symbol blast radius: BFS through usages to find all affected symbols and files at increasing depths"
    )]
    fn symbol_blast_radius(
        &self,
        Parameters(params): Parameters<SymbolBlastRadiusParam>,
    ) -> String {
        let state = self.state.load();
        let cp = CanonicalPath::new(&params.path);

        if !state.graph.nodes.contains_key(&cp) {
            let result = serde_json::json!({
                "error": "not_found",
                "path": params.path,
            });
            return to_json(&result);
        }

        let sym_exists = state
            .symbol_index
            .symbols_for_file(&cp)
            .map(|syms| syms.iter().any(|s| s.name == params.symbol))
            .unwrap_or(false);

        if !sym_exists {
            let result = serde_json::json!({
                "error": "symbol_not_found",
                "path": params.path,
                "symbol": params.symbol,
                "suggestion": "Use ariadne_symbols to list symbols in this file",
            });
            return to_json(&result);
        }

        let max_depth = params.depth.unwrap_or(3).min(10);

        // BFS on usages index
        let mut visited: std::collections::BTreeSet<(String, String)> = std::collections::BTreeSet::new();
        let mut affected_symbols: Vec<serde_json::Value> = Vec::new();
        let mut affected_files: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
        let mut truncated = false;

        // Seed: the starting symbol
        let start_key = (cp.as_str().to_string(), params.symbol.clone());
        visited.insert(start_key.clone());

        let mut frontier: std::collections::VecDeque<(CanonicalPath, String, u32)> =
            std::collections::VecDeque::new();
        frontier.push_back((cp.clone(), params.symbol.clone(), 0));

        while let Some((file, sym_name, distance)) = frontier.pop_front() {
            if distance > 0 {
                affected_symbols.push(serde_json::json!({
                    "file": file.as_str(),
                    "symbol": sym_name,
                    "distance": distance,
                }));
                affected_files.insert(file.as_str().to_string());
            }

            if distance >= max_depth {
                // If we hit the depth limit, check whether there would be
                // more nodes to explore — if so, the traversal is truncated.
                let has_more_usages = state
                    .symbol_index
                    .usages_of(&file, &sym_name)
                    .map(|u| u.iter().any(|usage| {
                        let key = (usage.file.as_str().to_string(), sym_name.clone());
                        !visited.contains(&key)
                    }))
                    .unwrap_or(false);

                let has_more_reverse = state
                    .reverse_index
                    .get(&file)
                    .map(|rev_edges| {
                        rev_edges.iter().any(|edge| {
                            edge.symbols.iter().any(|s| s.as_str() == sym_name)
                                && !visited.contains(&(
                                    edge.from.as_str().to_string(),
                                    sym_name.clone(),
                                ))
                        })
                    })
                    .unwrap_or(false);

                if has_more_usages || has_more_reverse {
                    truncated = true;
                }
                continue;
            }

            // Find usages of this symbol
            if let Some(usages) = state.symbol_index.usages_of(&file, &sym_name) {
                for usage in usages {
                    let key = (usage.file.as_str().to_string(), sym_name.clone());
                    if !visited.contains(&key) {
                        visited.insert(key);
                        frontier.push_back((
                            usage.file.clone(),
                            sym_name.clone(),
                            distance + 1,
                        ));
                    }
                }
            }

            // Also check reverse edges: files that import from this file
            // and reference this symbol name
            if let Some(rev_edges) = state.reverse_index.get(&file) {
                for edge in rev_edges {
                    if edge.symbols.iter().any(|s| s.as_str() == sym_name) {
                        let caller_key =
                            (edge.from.as_str().to_string(), sym_name.clone());
                        if !visited.contains(&caller_key) {
                            visited.insert(caller_key);
                            frontier.push_back((
                                edge.from.clone(),
                                sym_name.clone(),
                                distance + 1,
                            ));
                        }
                    }
                }
            }
        }

        // Sort for deterministic output
        affected_symbols.sort_by(|a, b| {
            let ad = a["distance"].as_u64().unwrap_or(0);
            let bd = b["distance"].as_u64().unwrap_or(0);
            ad.cmp(&bd)
                .then_with(|| {
                    a["file"]
                        .as_str()
                        .unwrap_or("")
                        .cmp(b["file"].as_str().unwrap_or(""))
                })
                .then_with(|| {
                    a["symbol"]
                        .as_str()
                        .unwrap_or("")
                        .cmp(b["symbol"].as_str().unwrap_or(""))
                })
        });

        let affected_files_sorted: Vec<&str> = {
            let mut v: Vec<&str> = affected_files.iter().map(|s| s.as_str()).collect();
            v.sort();
            v
        };

        if truncated {
            eprintln!(
                "warn[W021]: {}: call graph traversal for '{}' truncated at depth {} (more nodes exist beyond limit)",
                params.path, params.symbol, max_depth
            );
        }

        let result = serde_json::json!({
            "file": params.path,
            "symbol": params.symbol,
            "depth": max_depth,
            "affected_symbols": affected_symbols,
            "affected_files": affected_files_sorted,
            "total_affected": affected_symbols.len(),
            "truncated": truncated,
        });

        to_json(&result)
    }

    // --- T21: Callers ---

    #[tool(
        name = "ariadne_callers",
        description = "Cross-file callers of a symbol: which files import/reference this symbol definition"
    )]
    fn callers(&self, Parameters(params): Parameters<CallersParam>) -> String {
        let state = self.state.load();
        let cp = CanonicalPath::new(&params.path);

        if !state.graph.nodes.contains_key(&cp) {
            let result = serde_json::json!({
                "error": "not_found",
                "path": params.path,
                "suggestion": format!("File not in graph. Graph freshness: {:.0}%",
                    state.freshness.hash_confidence * 100.0),
            });
            return to_json(&result);
        }

        let sym_exists = state
            .symbol_index
            .symbols_for_file(&cp)
            .map(|syms| syms.iter().any(|s| s.name == params.symbol))
            .unwrap_or(false);

        if !sym_exists {
            let result = serde_json::json!({
                "error": "symbol_not_found",
                "path": params.path,
                "symbol": params.symbol,
                "suggestion": "Use ariadne_symbols to list symbols in this file",
            });
            return to_json(&result);
        }

        let call_edges = state.call_graph.callers_of(&cp, &params.symbol);
        let callers_json: Vec<serde_json::Value> = call_edges
            .iter()
            .map(|ce| {
                serde_json::json!({
                    "file": ce.file.as_str(),
                    "symbol": ce.symbol,
                    "edge_kind": ce.edge_kind.as_str(),
                })
            })
            .collect();

        let result = serde_json::json!({
            "file": params.path,
            "symbol": params.symbol,
            "callers": callers_json,
            "count": callers_json.len(),
        });

        to_json(&result)
    }

    // --- T22: Callees ---

    #[tool(
        name = "ariadne_callees",
        description = "Cross-file callees of a symbol: which files/symbols does this symbol's file import via this name"
    )]
    fn callees(&self, Parameters(params): Parameters<CalleesParam>) -> String {
        let state = self.state.load();
        let cp = CanonicalPath::new(&params.path);

        if !state.graph.nodes.contains_key(&cp) {
            let result = serde_json::json!({
                "error": "not_found",
                "path": params.path,
                "suggestion": format!("File not in graph. Graph freshness: {:.0}%",
                    state.freshness.hash_confidence * 100.0),
            });
            return to_json(&result);
        }

        // Validate symbol existence: check if it's defined in this file or has call graph entries
        let defined_here = state
            .symbol_index
            .symbols_for_file(&cp)
            .map(|syms| syms.iter().any(|s| s.name == params.symbol))
            .unwrap_or(false);

        let call_edges = state.call_graph.callees_of(&cp, &params.symbol);

        // If the symbol is neither defined here nor has callee edges, it doesn't exist
        if !defined_here && call_edges.is_empty() {
            let result = serde_json::json!({
                "error": "symbol_not_found",
                "path": params.path,
                "symbol": params.symbol,
                "suggestion": "Use ariadne_symbols to list symbols in this file",
            });
            return to_json(&result);
        }

        if call_edges.is_empty() && defined_here {
            // Symbol exists but has no outgoing call edges (it's defined here, not imported)
            let result = serde_json::json!({
                "file": params.path,
                "symbol": params.symbol,
                "callees": [],
                "count": 0,
                "note": "Symbol is defined in this file. Use ariadne_callers to find who calls it.",
            });
            return to_json(&result);
        }

        let callees_json: Vec<serde_json::Value> = call_edges
            .iter()
            .map(|ce| {
                serde_json::json!({
                    "file": ce.file.as_str(),
                    "symbol": ce.symbol,
                    "edge_kind": ce.edge_kind.as_str(),
                })
            })
            .collect();

        let result = serde_json::json!({
            "file": params.path,
            "symbol": params.symbol,
            "callees": callees_json,
            "count": callees_json.len(),
        });

        to_json(&result)
    }

    // --- T23: Smart context assembly ---

    #[tool(
        name = "ariadne_context",
        description = "Assemble optimal file context for a task. Returns ranked files within a token budget, scored by relevance to the seed files and task type."
    )]
    fn context(&self, Parameters(params): Parameters<ContextParam>) -> String {
        let state = self.state.load();

        // Convert paths, collecting warnings for invalid ones
        let mut warnings: Vec<String> = Vec::new();
        let mut valid_paths: Vec<CanonicalPath> = Vec::new();
        for path in &params.files {
            let cp = CanonicalPath::new(path);
            if state.graph.nodes.contains_key(&cp) {
                valid_paths.push(cp);
            } else {
                warnings.push(format!("File not in graph: {}", path));
            }
        }

        if valid_paths.is_empty() {
            let result = serde_json::json!({
                "error": "no_valid_files",
                "message": "None of the provided files exist in the graph",
                "warnings": warnings,
            });
            return to_json(&result);
        }

        let task = params
            .task
            .as_deref()
            .and_then(TaskType::parse)
            .unwrap_or(TaskType::Understand);
        let budget = params.budget_tokens.unwrap_or(8000);
        let depth = params.depth.unwrap_or(3);

        let index = algo::AdjacencyIndex::build(&state.graph.edges, algo::is_architectural);
        let ctx = assemble_context(
            &valid_paths,
            &state.graph,
            &index,
            &state.stats,
            &state.clusters,
            task,
            budget,
            depth,
        );

        let mut all_warnings = ctx.warnings;
        all_warnings.extend(warnings);

        let selected: Vec<serde_json::Value> = ctx
            .selected
            .iter()
            .map(|c| {
                serde_json::json!({
                    "path": c.path.as_str(),
                    "relevance": c.relevance,
                    "tokens": c.tokens,
                    "tier": c.tier.as_str(),
                })
            })
            .collect();

        let result = serde_json::json!({
            "selected": selected,
            "total_tokens": ctx.total_tokens,
            "budget_used": ctx.budget_used,
            "budget": budget,
            "task": format!("{:?}", task),
            "depth": depth,
            "file_count": ctx.selected.len(),
            "warnings": all_warnings,
        });

        to_json(&result)
    }

    // --- T24: Test mapping ---

    #[tool(
        name = "ariadne_tests_for",
        description = "Find test files covering the given source files. Returns tests with confidence levels (high/medium/low) and detection reasons."
    )]
    fn tests_for(&self, Parameters(params): Parameters<TestsForParam>) -> String {
        let state = self.state.load();

        let mut warnings: Vec<String> = Vec::new();
        let mut valid_paths: Vec<CanonicalPath> = Vec::new();
        for path in &params.paths {
            let cp = CanonicalPath::new(path);
            if state.graph.nodes.contains_key(&cp) {
                valid_paths.push(cp);
            } else {
                warnings.push(format!("File not in graph: {}", path));
            }
        }

        if valid_paths.is_empty() && !params.paths.is_empty() {
            let result = serde_json::json!({
                "error": "no_valid_files",
                "message": "None of the provided files exist in the graph",
                "warnings": warnings,
            });
            return to_json(&result);
        }

        let index = algo::AdjacencyIndex::build(&state.graph.edges, algo::is_architectural);
        let map_result = find_tests_for(&valid_paths, &state.graph, &index);

        let mut all_warnings = map_result.warnings;
        all_warnings.extend(warnings);

        let tests: Vec<serde_json::Value> = map_result
            .tests
            .iter()
            .map(|hit| {
                serde_json::json!({
                    "path": hit.path.as_str(),
                    "confidence": hit.confidence.as_str(),
                    "reason": hit.reason,
                })
            })
            .collect();

        let result = serde_json::json!({
            "tests": tests,
            "count": tests.len(),
            "warnings": all_warnings,
        });

        to_json(&result)
    }

    // --- T25: Reading order ---

    #[tool(
        name = "ariadne_reading_order",
        description = "Topologically sorted reading order for understanding a set of files. Leaves (no dependencies) come first, then layers build up."
    )]
    fn reading_order(&self, Parameters(params): Parameters<ReadingOrderParam>) -> String {
        let state = self.state.load();

        let mut warnings: Vec<String> = Vec::new();
        let mut valid_paths: Vec<CanonicalPath> = Vec::new();
        for path in &params.paths {
            let cp = CanonicalPath::new(path);
            if state.graph.nodes.contains_key(&cp) {
                valid_paths.push(cp);
            } else {
                warnings.push(format!("File not in graph: {}", path));
            }
        }

        if valid_paths.is_empty() && !params.paths.is_empty() {
            let result = serde_json::json!({
                "error": "no_valid_files",
                "message": "None of the provided files exist in the graph",
                "warnings": warnings,
            });
            return to_json(&result);
        }

        let depth = params.depth.unwrap_or(3);
        let order_result = compute_reading_order(&valid_paths, &state.graph, depth);

        let mut all_warnings = order_result.warnings;
        all_warnings.extend(warnings);

        let entries: Vec<serde_json::Value> = order_result
            .entries
            .iter()
            .map(|e| {
                let depth_value = if e.depth == u32::MAX {
                    serde_json::Value::Null
                } else {
                    serde_json::json!(e.depth)
                };
                serde_json::json!({
                    "path": e.path.as_str(),
                    "reason": e.reason,
                    "layer": e.layer,
                    "depth": depth_value,
                })
            })
            .collect();

        let result = serde_json::json!({
            "entries": entries,
            "total_files": order_result.total_files,
            "depth": depth,
            "warnings": all_warnings,
        });

        to_json(&result)
    }

    // --- T26: Plan impact analysis ---

    #[tool(
        name = "ariadne_plan_impact",
        description = "Analyze the structural impact of planned file changes. Returns blast radius, affected tests, risk assessment, and change classification."
    )]
    fn plan_impact(&self, Parameters(params): Parameters<PlanImpactParam>) -> String {
        let state = self.state.load();

        let mut warnings: Vec<String> = Vec::new();
        let mut valid_paths: Vec<CanonicalPath> = Vec::new();
        for entry in &params.changes {
            let cp = CanonicalPath::new(&entry.path);
            if state.graph.nodes.contains_key(&cp) {
                valid_paths.push(cp);
            } else {
                warnings.push(format!("Changed file not in graph: {}", entry.path));
            }
        }

        if valid_paths.is_empty() && !params.changes.is_empty() {
            let result = serde_json::json!({
                "error": "no_valid_files",
                "message": "None of the changed files exist in the graph",
                "warnings": warnings,
            });
            return to_json(&result);
        }

        let index = algo::AdjacencyIndex::build(&state.graph.edges, algo::is_architectural);

        let impact = analyze_impact(
            &valid_paths,
            &state.graph,
            &index,
            &state.stats,
            &state.clusters,
        );

        let mut all_warnings = impact.warnings;
        all_warnings.extend(warnings);

        let affected: Vec<serde_json::Value> = impact
            .affected_files
            .iter()
            .map(|(path, &dist)| {
                serde_json::json!({
                    "path": path.as_str(),
                    "distance": dist,
                })
            })
            .collect();

        let tests: Vec<serde_json::Value> = impact
            .affected_tests
            .iter()
            .map(|hit| {
                serde_json::json!({
                    "path": hit.path.as_str(),
                    "confidence": hit.confidence.as_str(),
                    "reason": hit.reason,
                })
            })
            .collect();

        let result = serde_json::json!({
            "total_affected": impact.total_affected,
            "affected_files": affected,
            "affected_tests": tests,
            "layers_crossed": impact.layers_crossed,
            "layer_direction": impact.layer_direction,
            "clusters_affected": impact.clusters_affected,
            "risks": impact.risks,
            "change_class": impact.change_class.as_str(),
            "token_estimate": impact.token_estimate,
            "warnings": all_warnings,
        });

        to_json(&result)
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
                let clusters_dir = views_dir.join("clusters");
                if let Some(cluster) = &params.cluster {
                    let path = clusters_dir.join(format!("{}.md", cluster));
                    std::fs::read_to_string(&path)
                        .unwrap_or_else(|_| format!("L1 cluster view '{}' not found.", cluster))
                } else {
                    let mut views = Vec::new();
                    if let Ok(entries) = std::fs::read_dir(&clusters_dir) {
                        for entry in entries.flatten() {
                            let name = entry.file_name().to_string_lossy().to_string();
                            if name.ends_with(".md") {
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

/// Parse a SymbolKind from a string (case-insensitive).
fn parse_symbol_kind(s: &str) -> Option<crate::model::SymbolKind> {
    use crate::model::SymbolKind;
    match s.to_lowercase().as_str() {
        "function" => Some(SymbolKind::Function),
        "method" => Some(SymbolKind::Method),
        "class" => Some(SymbolKind::Class),
        "struct" => Some(SymbolKind::Struct),
        "interface" => Some(SymbolKind::Interface),
        "trait" => Some(SymbolKind::Trait),
        "type" => Some(SymbolKind::Type),
        "enum" => Some(SymbolKind::Enum),
        "const" => Some(SymbolKind::Const),
        "variable" => Some(SymbolKind::Variable),
        "module" => Some(SymbolKind::Module),
        _ => None,
    }
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
