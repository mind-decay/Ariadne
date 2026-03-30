use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use arc_swap::ArcSwap;
use rmcp::handler::server::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::*;
use rmcp::service::RequestContext;
use rmcp::{tool, tool_handler, tool_router, RoleServer, ServerHandler};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::algo;
use crate::algo::context::{assemble_context, TaskType};
use crate::algo::impact::analyze_impact;
use crate::algo::reading_order::compute_reading_order;
use crate::algo::test_map::find_tests_for;
use crate::analysis::smells::detect_smells;
use crate::recommend;
use crate::mcp::state::GraphState;
use crate::mcp::tools_context::{ContextParam, PlanImpactParam, ReadingOrderParam, TestsForParam};
use crate::mcp::tools_semantic::{
    BoundariesParam, BoundaryForParam, EventMapParam, RouteMapParam,
};
use crate::mcp::tools_recommend::{RefactorOpportunitiesParam, SuggestPlacementParam, SuggestSplitParam};
use crate::mcp::tools_temporal::{
    ChurnParam, CouplingParam, HiddenDepsParam, HotspotsParam, OwnershipParam,
};
use crate::mcp::user_state::UserStateManager;
use crate::model::{AnnotationTarget, CanonicalPath};

/// MCP tool handler struct. Each tool is a thin wrapper around existing algo functions.
#[derive(Debug, Clone)]
pub struct AriadneTools {
    pub state: Arc<ArcSwap<GraphState>>,
    pub rebuilding: Arc<AtomicBool>,
    pub project_root: PathBuf,
    pub user_state: Arc<UserStateManager>,
    tool_router: ToolRouter<Self>,
}

impl AriadneTools {
    pub fn new(
        state: Arc<ArcSwap<GraphState>>,
        rebuilding: Arc<AtomicBool>,
        project_root: PathBuf,
        user_state: Arc<UserStateManager>,
    ) -> Self {
        Self {
            state,
            rebuilding,
            project_root,
            user_state,
            tool_router: Self::tool_router(),
        }
    }
}

#[tool_handler]
impl ServerHandler for AriadneTools {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(
            ServerCapabilities::builder()
                .enable_tools()
                .enable_resources()
                .enable_prompts()
                .build(),
        )
        .with_instructions("Ariadne structural dependency graph engine")
    }

    fn list_resources(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<ListResourcesResult, rmcp::ErrorData>> + Send + '_ {
        let state = self.state.load();
        std::future::ready(Ok(crate::mcp::resources::list_resources_impl(&state)))
    }

    fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<ReadResourceResult, rmcp::ErrorData>> + Send + '_ {
        let state = self.state.load();
        std::future::ready(Ok(crate::mcp::resources::read_resource_impl(
            &request.uri, &state,
        )))
    }

    fn list_prompts(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<ListPromptsResult, rmcp::ErrorData>> + Send + '_ {
        std::future::ready(Ok(crate::mcp::prompts::list_prompts_impl()))
    }

    fn get_prompt(
        &self,
        request: GetPromptRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<GetPromptResult, rmcp::ErrorData>> + Send + '_ {
        let state = self.state.load();
        // Convert JsonObject (Map<String, Value>) to HashMap<String, String>
        let args: Option<std::collections::HashMap<String, String>> =
            request.arguments.map(|obj| {
                obj.into_iter()
                    .map(|(k, v)| {
                        let s = match v {
                            serde_json::Value::String(s) => s,
                            other => other.to_string(),
                        };
                        (k, s)
                    })
                    .collect()
            });
        std::future::ready(Ok(crate::mcp::prompts::get_prompt_impl(
            &request.name,
            args,
            &state,
        )))
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
    /// When true, also follow semantic edges (routes/events) for 1 additional hop
    pub include_semantic: Option<bool>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SubgraphParam {
    /// Center file paths
    pub paths: Vec<String>,
    /// BFS depth (default 2)
    pub depth: Option<u32>,
    /// Optional bookmark name — if provided, its expanded paths are merged with `paths`
    pub bookmark: Option<String>,
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

// --- Annotation/Bookmark tool parameter types ---

#[derive(Debug, Deserialize, JsonSchema)]
pub struct AnnotateParam {
    /// Target type: "file", "cluster", or "edge"
    pub target_type: String,
    /// Target path or name (file path for "file", cluster name for "cluster", "from->to" for "edge")
    pub target_path: String,
    /// Target symbol name (optional, for file-level symbol annotations)
    pub target_symbol: Option<String>,
    /// Annotation label (e.g., "entry-point", "deprecated", "hot-path")
    pub label: String,
    /// Optional freeform note
    pub note: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct AnnotationsParam {
    /// Filter by label (exact match)
    pub tag: Option<String>,
    /// Filter by target type: "file", "cluster", or "edge"
    pub target_type: Option<String>,
    /// Filter by target path/name (substring match)
    pub target_path: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RemoveAnnotationParam {
    /// Annotation id (e.g., "ann-1")
    pub id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct BookmarkParam {
    /// Bookmark name (unique identifier)
    pub name: String,
    /// File paths or directory prefixes to include
    pub paths: Vec<String>,
    /// Optional description
    pub description: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RemoveBookmarkParam {
    /// Bookmark name to remove
    pub name: String,
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

        let mut result = serde_json::json!({
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

        // Add temporal summary when available
        if let Some(ref temporal) = state.temporal {
            let total_commits_30d: u32 = temporal.churn.values().map(|c| c.commits_30d).sum();
            let hidden_dep_count = temporal
                .co_changes
                .iter()
                .filter(|cc| !cc.has_structural_link)
                .count();
            result["temporal"] = serde_json::json!({
                "total_commits_30d": total_commits_30d,
                "hotspot_count": temporal.hotspots.len(),
                "hidden_dep_count": hidden_dep_count,
                "commits_analyzed": temporal.commits_analyzed,
            });
        }

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

        let annotations = crate::mcp::annotations::annotations_for_file(
            &self.user_state, &params.path,
        );

        let mut result = serde_json::json!({
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
            "annotations": annotations,
        });

        // Add temporal data when available
        if let Some(ref temporal) = state.temporal {
            if let Some(churn) = temporal.churn.get(&cp) {
                result["temporal"] = serde_json::json!({
                    "commits_30d": churn.commits_30d,
                    "commits_90d": churn.commits_90d,
                    "commits_1y": churn.commits_1y,
                    "last_changed": churn.last_changed,
                    "top_authors": churn.top_authors,
                });
            }
        }

        // Add semantic boundary data when available
        if let Some(ref semantic) = state.semantic {
            let boundaries: Vec<serde_json::Value> = semantic
                .boundaries
                .get(params.path.as_str())
                .map(|bs| {
                    bs.iter()
                        .map(|b| {
                            serde_json::json!({
                                "kind": b.kind,
                                "name": b.name,
                                "role": b.role,
                                "line": b.line,
                                "framework": b.framework,
                                "method": b.method,
                            })
                        })
                        .collect()
                })
                .unwrap_or_default();
            result["boundaries"] = serde_json::json!(boundaries);
        } else {
            result["boundaries"] = serde_json::json!([]);
        }

        to_json(&result)
    }

    // --- T3: Blast radius ---

    #[tool(
        name = "ariadne_blast_radius",
        description = "Reverse BFS: map of affected files with distances from the given file. When 'symbol' is provided, traces symbol-level blast radius instead. Set 'include_semantic' to also follow semantic edges."
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
        let mut result = algo::blast_radius::blast_radius(&state.graph, &cp, params.depth, &index);

        // When include_semantic is true, expand with files connected via semantic edges (1 hop)
        if params.include_semantic.unwrap_or(false) {
            if let Some(ref semantic) = state.semantic {
                let structural_files: Vec<CanonicalPath> =
                    result.keys().cloned().collect();
                let mut semantic_files: Vec<(CanonicalPath, u32)> = Vec::new();

                // For each file in the structural blast radius, find semantic edges from it
                for file in &structural_files {
                    let file_str = file.as_str();
                    for edge in &semantic.edges {
                        if edge.from == file_str {
                            let target = CanonicalPath::new(&edge.to);
                            if !result.contains_key(&target) {
                                let source_dist = result.get(file).copied().unwrap_or(0);
                                semantic_files.push((target, source_dist + 1));
                            }
                        }
                        if edge.to == file_str {
                            let target = CanonicalPath::new(&edge.from);
                            if !result.contains_key(&target) {
                                let source_dist = result.get(file).copied().unwrap_or(0);
                                semantic_files.push((target, source_dist + 1));
                            }
                        }
                    }
                }

                // Also check semantic edges from the origin file itself
                let origin_str = cp.as_str();
                for edge in &semantic.edges {
                    if edge.from == origin_str {
                        let target = CanonicalPath::new(&edge.to);
                        if !result.contains_key(&target) {
                            semantic_files.push((target, 1));
                        }
                    }
                    if edge.to == origin_str {
                        let target = CanonicalPath::new(&edge.from);
                        if !result.contains_key(&target) {
                            semantic_files.push((target, 1));
                        }
                    }
                }

                for (file, dist) in semantic_files {
                    result.entry(file).or_insert(dist);
                }
            }
        }

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

        // Merge explicit paths with bookmark paths if provided
        let mut all_paths = params.paths.clone();
        if let Some(ref bm_name) = params.bookmark {
            match crate::mcp::bookmarks::resolve_bookmark(&self.user_state, bm_name, &state.graph)
            {
                Ok(expanded) => all_paths.extend(expanded),
                Err(e) => {
                    let result = serde_json::json!({
                        "error": "bookmark_not_found",
                        "message": e,
                    });
                    return to_json(&result);
                }
            }
        }

        let paths: Vec<CanonicalPath> = all_paths.iter().map(CanonicalPath::new).collect();
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
                let annotations = crate::mcp::annotations::annotations_for_cluster(
                    &self.user_state, &params.name,
                );
                let result = serde_json::json!({
                    "name": params.name,
                    "files": cluster.files.iter().map(|p| p.as_str().to_string()).collect::<Vec<_>>(),
                    "file_count": cluster.file_count,
                    "internal_edges": cluster.internal_edges,
                    "external_edges": cluster.external_edges,
                    "cohesion": cluster.cohesion,
                    "annotations": annotations,
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

        // Build semantic_deps from semantic edges
        let semantic_deps: Vec<serde_json::Value> = if let Some(ref semantic) = state.semantic {
            let path_str = params.path.as_str();
            semantic
                .edges
                .iter()
                .filter(|e| e.from == path_str)
                .map(|e| {
                    serde_json::json!({
                        "via": format!("{} {}", e.kind, e.name),
                        "target": e.to,
                        "confidence": e.confidence,
                    })
                })
                .collect()
        } else {
            vec![]
        };

        let result = serde_json::json!({
            "path": params.path,
            "direction": params.direction,
            "incoming": incoming,
            "outgoing": outgoing,
            "symbol_edges": symbol_edges,
            "semantic_deps": semantic_deps,
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
        description = "Detect architectural smells: god files, circular dependencies, layer violations, hub-and-spoke, unstable foundations, dead clusters, shotgun surgery, orphan routes/events"
    )]
    fn smells(&self, Parameters(params): Parameters<SmellsParam>) -> String {
        let state = self.state.load();
        let semantic = state.semantic.as_ref().map(crate::serial::boundary_output_to_semantic_state);
        let smells = detect_smells(
            &state.graph,
            &state.stats,
            &state.clusters,
            &state.cluster_metrics,
            state.temporal.as_ref(),
            semantic.as_ref(),
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

        // Enhanced formula when temporal data available:
        // structural_importance * (1.0 + ln(1 + churn_30d) / max_log_churn)
        let temporal_boost: Option<(BTreeMap<&CanonicalPath, f64>, f64)> =
            state.temporal.as_ref().map(|temporal| {
                let max_log_churn = temporal
                    .churn
                    .values()
                    .map(|c| (1.0 + c.commits_30d as f64).ln())
                    .fold(0.0_f64, f64::max)
                    .max(1.0); // avoid division by zero
                let boosts: BTreeMap<&CanonicalPath, f64> = temporal
                    .churn
                    .iter()
                    .map(|(path, c)| {
                        let log_churn = (1.0 + c.commits_30d as f64).ln();
                        (path, log_churn)
                    })
                    .collect();
                (boosts, max_log_churn)
            });

        let mut ranked: Vec<(&CanonicalPath, f64)> = state
            .combined_importance
            .iter()
            .map(|(path, &score)| {
                let boosted = if let Some((ref boosts, max_log)) = temporal_boost {
                    let log_churn = boosts.get(path).copied().unwrap_or(0.0);
                    score * (1.0 + log_churn / max_log)
                } else {
                    score
                };
                (path, boosted)
            })
            .collect();

        ranked.sort_by(|a, b| {
            b.1.partial_cmp(&a.1)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.0.cmp(b.0))
        });
        ranked.truncate(top);

        let result: Vec<serde_json::Value> = ranked
            .iter()
            .map(|(path, score)| {
                let mut obj = serde_json::json!({
                    "path": path.as_str(),
                    "combined_score": score,
                    "centrality": state.stats.centrality.get(path.as_str()).copied().unwrap_or(0.0),
                    "pagerank": state.pagerank.get(*path).copied().unwrap_or(0.0),
                });
                if let Some(ref temporal) = state.temporal {
                    if let Some(churn) = temporal.churn.get(*path) {
                        obj["churn_30d"] = serde_json::json!(churn.commits_30d);
                    }
                }
                obj
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

        // Merge explicit files with bookmark paths if provided
        let mut all_files = params.files.clone();
        if let Some(ref bm_name) = params.bookmark {
            match crate::mcp::bookmarks::resolve_bookmark(&self.user_state, bm_name, &state.graph)
            {
                Ok(expanded) => all_files.extend(expanded),
                Err(e) => {
                    let result = serde_json::json!({
                        "error": "bookmark_not_found",
                        "message": e,
                    });
                    return to_json(&result);
                }
            }
        }

        // Convert paths, collecting warnings for invalid ones
        let mut warnings: Vec<String> = Vec::new();
        let mut valid_paths: Vec<CanonicalPath> = Vec::new();
        for path in &all_files {
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

        // Compute temporal churn boost when available:
        // High-churn files get a relevance boost to surface them in context
        let churn_boost: Option<(BTreeMap<&CanonicalPath, f64>, f64)> =
            state.temporal.as_ref().map(|temporal| {
                let max_log_churn = temporal
                    .churn
                    .values()
                    .map(|c| (1.0 + c.commits_30d as f64).ln())
                    .fold(0.0_f64, f64::max)
                    .max(1.0);
                let boosts: BTreeMap<&CanonicalPath, f64> = temporal
                    .churn
                    .iter()
                    .map(|(path, c)| (path, (1.0 + c.commits_30d as f64).ln()))
                    .collect();
                (boosts, max_log_churn)
            });

        let selected: Vec<serde_json::Value> = ctx
            .selected
            .iter()
            .map(|c| {
                let boosted_relevance = if let Some((ref boosts, max_log)) = churn_boost {
                    let log_churn = boosts.get(&c.path).copied().unwrap_or(0.0);
                    // Add up to 0.15 bonus based on churn
                    let bonus = 0.15 * (log_churn / max_log);
                    (c.relevance + bonus).min(1.0)
                } else {
                    c.relevance
                };

                let mut obj = serde_json::json!({
                    "path": c.path.as_str(),
                    "relevance": boosted_relevance,
                    "tokens": c.tokens,
                    "tier": c.tier.as_str(),
                });
                if let Some((ref boosts, _)) = churn_boost {
                    if let Some(&log_churn) = boosts.get(&c.path) {
                        if log_churn > 0.0 {
                            obj["churn_boost"] = serde_json::json!(true);
                        }
                    }
                }
                obj
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

    // --- T27: Annotate ---

    #[tool(
        name = "ariadne_annotate",
        description = "Add or update an annotation on a graph element (file, cluster, or edge). Labels act as tags. Upserts: same target+label updates the existing annotation."
    )]
    fn annotate(&self, Parameters(params): Parameters<AnnotateParam>) -> String {
        let target = match params.target_type.as_str() {
            "file" => AnnotationTarget::File {
                path: params.target_path,
            },
            "cluster" => AnnotationTarget::Cluster {
                name: params.target_path,
            },
            "edge" => {
                let parts: Vec<&str> = params.target_path.splitn(2, "->").collect();
                if parts.len() != 2 {
                    let result = serde_json::json!({
                        "error": "invalid_edge_target",
                        "message": "Edge target must be in 'from->to' format",
                    });
                    return to_json(&result);
                }
                AnnotationTarget::Edge {
                    from: parts[0].trim().to_string(),
                    to: parts[1].trim().to_string(),
                }
            }
            other => {
                let result = serde_json::json!({
                    "error": "invalid_target_type",
                    "value": other,
                    "valid_values": ["file", "cluster", "edge"],
                });
                return to_json(&result);
            }
        };

        match crate::mcp::annotations::annotate(&self.user_state, target, params.label, params.note)
        {
            Ok(val) => to_json(&val),
            Err(e) => {
                let result = serde_json::json!({ "error": "annotation_failed", "message": e });
                to_json(&result)
            }
        }
    }

    // --- T28: List annotations ---

    #[tool(
        name = "ariadne_annotations",
        description = "List annotations with optional filters: by label (tag), target type, or target path substring"
    )]
    fn annotations(&self, Parameters(params): Parameters<AnnotationsParam>) -> String {
        let result = crate::mcp::annotations::list_annotations(
            &self.user_state,
            params.tag,
            params.target_type,
            params.target_path,
        );
        to_json(&result)
    }

    // --- T29: Remove annotation ---

    #[tool(
        name = "ariadne_remove_annotation",
        description = "Remove an annotation by its id (e.g., 'ann-1')"
    )]
    fn remove_annotation(&self, Parameters(params): Parameters<RemoveAnnotationParam>) -> String {
        match crate::mcp::annotations::remove_annotation(&self.user_state, params.id) {
            Ok(val) => to_json(&val),
            Err(e) => {
                let result = serde_json::json!({ "error": "remove_failed", "message": e });
                to_json(&result)
            }
        }
    }

    // --- T30: Bookmark ---

    #[tool(
        name = "ariadne_bookmark",
        description = "Create or update a named bookmark pointing to file paths or directory prefixes. Bookmarks can be used with ariadne_subgraph and ariadne_context."
    )]
    fn bookmark(&self, Parameters(params): Parameters<BookmarkParam>) -> String {
        match crate::mcp::bookmarks::bookmark(
            &self.user_state,
            params.name,
            params.paths,
            params.description,
        ) {
            Ok(val) => to_json(&val),
            Err(e) => {
                let result = serde_json::json!({ "error": "bookmark_failed", "message": e });
                to_json(&result)
            }
        }
    }

    // --- T31: List bookmarks ---

    #[tool(
        name = "ariadne_bookmarks",
        description = "List all saved bookmarks with their paths and descriptions"
    )]
    fn bookmarks(&self) -> String {
        let result = crate::mcp::bookmarks::list_bookmarks(&self.user_state);
        to_json(&result)
    }

    // --- T32: Remove bookmark ---

    #[tool(
        name = "ariadne_remove_bookmark",
        description = "Remove a bookmark by name"
    )]
    fn remove_bookmark(&self, Parameters(params): Parameters<RemoveBookmarkParam>) -> String {
        match crate::mcp::bookmarks::remove_bookmark(&self.user_state, params.name) {
            Ok(val) => to_json(&val),
            Err(e) => {
                let result = serde_json::json!({ "error": "remove_failed", "message": e });
                to_json(&result)
            }
        }
    }

    // --- T33: Churn ---

    #[tool(
        name = "ariadne_churn",
        description = "File change frequency from git history: commit counts, lines changed, authors per file across time windows (30d/90d/1y)"
    )]
    fn churn(&self, Parameters(params): Parameters<ChurnParam>) -> String {
        let state = self.state.load();
        let temporal = match &state.temporal {
            Some(t) => t,
            None => {
                return to_json(&serde_json::json!({
                    "error": "temporal_unavailable",
                    "reason": "git history not available",
                }));
            }
        };

        let period = params.period.as_deref().unwrap_or("30d");
        // Validate period
        if !matches!(period, "30d" | "90d" | "1y") {
            return to_json(&serde_json::json!({
                "error": "invalid_period",
                "value": period,
                "valid_values": ["30d", "90d", "1y"],
                "message": "Period must be one of: '30d', '90d', '1y'",
            }));
        }

        let top = params.top.unwrap_or(20);
        if top == 0 {
            return to_json(&serde_json::json!({
                "error": "invalid_top",
                "value": top,
                "message": "'top' must be greater than 0",
            }));
        }

        let mut entries: Vec<_> = temporal
            .churn
            .iter()
            .map(|(path, churn)| {
                let commits = match period {
                    "30d" => churn.commits_30d,
                    "90d" => churn.commits_90d,
                    "1y" => churn.commits_1y,
                    _ => churn.commits_30d,
                };
                let lines_changed = match period {
                    "30d" => churn.lines_changed_30d,
                    "90d" => churn.lines_changed_90d,
                    _ => churn.lines_changed_30d, // 1y uses 30d lines as best available
                };
                (path, churn, commits, lines_changed)
            })
            .collect();

        // Sort by commit count descending, then path for determinism
        entries.sort_by(|a, b| {
            b.2.cmp(&a.2)
                .then_with(|| a.0.cmp(b.0))
        });
        entries.truncate(top as usize);

        let result: Vec<serde_json::Value> = entries
            .iter()
            .map(|(path, churn, commits, lines_changed)| {
                serde_json::json!({
                    "path": path.as_str(),
                    "commits": commits,
                    "lines_changed": lines_changed,
                    "authors": churn.authors_30d,
                    "last_changed": churn.last_changed,
                    "top_authors": churn.top_authors,
                })
            })
            .collect();

        to_json(&result)
    }

    // --- T34: Coupling ---

    #[tool(
        name = "ariadne_coupling",
        description = "Co-change coupling from git history: file pairs that frequently change together, with confidence scores and structural link status"
    )]
    fn coupling(&self, Parameters(params): Parameters<CouplingParam>) -> String {
        let state = self.state.load();
        let temporal = match &state.temporal {
            Some(t) => t,
            None => {
                return to_json(&serde_json::json!({
                    "error": "temporal_unavailable",
                    "reason": "git history not available",
                }));
            }
        };

        let min_confidence = params.min_confidence.unwrap_or(0.3);
        if !(0.0..=1.0).contains(&min_confidence) {
            return to_json(&serde_json::json!({
                "error": "invalid_min_confidence",
                "value": min_confidence,
                "message": "'min_confidence' must be in [0.0, 1.0]",
            }));
        }

        let mut filtered: Vec<_> = temporal
            .co_changes
            .iter()
            .filter(|cc| cc.confidence >= min_confidence)
            .collect();

        // Sort by confidence descending, then file_a+file_b for determinism
        filtered.sort_by(|a, b| {
            b.confidence
                .partial_cmp(&a.confidence)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.file_a.cmp(&b.file_a))
                .then_with(|| a.file_b.cmp(&b.file_b))
        });

        let result: Vec<serde_json::Value> = filtered
            .iter()
            .map(|cc| {
                serde_json::json!({
                    "file_a": cc.file_a.as_str(),
                    "file_b": cc.file_b.as_str(),
                    "co_change_count": cc.co_change_count,
                    "confidence": cc.confidence,
                    "has_structural_link": cc.has_structural_link,
                })
            })
            .collect();

        to_json(&result)
    }

    // --- T35: Hotspots ---

    #[tool(
        name = "ariadne_hotspots",
        description = "High-risk change hotspots: files with high combined churn, size, and blast radius scores"
    )]
    fn hotspots(&self, Parameters(params): Parameters<HotspotsParam>) -> String {
        let state = self.state.load();
        let temporal = match &state.temporal {
            Some(t) => t,
            None => {
                return to_json(&serde_json::json!({
                    "error": "temporal_unavailable",
                    "reason": "git history not available",
                }));
            }
        };

        let top = params.top.unwrap_or(20);
        if top == 0 {
            return to_json(&serde_json::json!({
                "error": "invalid_top",
                "value": top,
                "message": "'top' must be greater than 0",
            }));
        }

        // Hotspots are pre-sorted by score descending
        let result: Vec<serde_json::Value> = temporal
            .hotspots
            .iter()
            .take(top as usize)
            .map(|h| {
                serde_json::json!({
                    "path": h.path.as_str(),
                    "score": h.score,
                    "churn_rank": h.churn_rank,
                    "loc_rank": h.loc_rank,
                    "blast_radius_rank": h.blast_radius_rank,
                })
            })
            .collect();

        to_json(&result)
    }

    // --- T36: Ownership ---

    #[tool(
        name = "ariadne_ownership",
        description = "File ownership from git history: last author, top contributors, author count. Query a specific file or get project-wide top authors."
    )]
    fn ownership(&self, Parameters(params): Parameters<OwnershipParam>) -> String {
        let state = self.state.load();
        let temporal = match &state.temporal {
            Some(t) => t,
            None => {
                return to_json(&serde_json::json!({
                    "error": "temporal_unavailable",
                    "reason": "git history not available",
                }));
            }
        };

        if let Some(ref path) = params.path {
            let cp = CanonicalPath::new(path);
            match temporal.ownership.get(&cp) {
                Some(info) => to_json(&serde_json::json!({
                    "path": path,
                    "last_author": info.last_author,
                    "top_contributors": info.top_contributors,
                    "author_count": info.author_count,
                })),
                None => to_json(&serde_json::json!({
                    "error": "not_found",
                    "path": path,
                    "message": "No ownership data for this file",
                })),
            }
        } else {
            // Aggregate: top authors across the project
            let mut author_counts: BTreeMap<String, u32> = BTreeMap::new();
            for info in temporal.ownership.values() {
                for (name, count) in &info.top_contributors {
                    *author_counts.entry(name.clone()).or_default() += count;
                }
            }
            let mut sorted: Vec<(String, u32)> = author_counts.into_iter().collect();
            sorted.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
            sorted.truncate(20);

            let total_authors: std::collections::BTreeSet<&str> = temporal
                .ownership
                .values()
                .flat_map(|info| info.top_contributors.iter().map(|(n, _)| n.as_str()))
                .collect();

            to_json(&serde_json::json!({
                "top_contributors": sorted,
                "author_count": total_authors.len(),
                "files_with_ownership": temporal.ownership.len(),
            }))
        }
    }

    // --- T37: Hidden dependencies ---

    #[tool(
        name = "ariadne_hidden_deps",
        description = "Hidden dependencies: file pairs that co-change frequently but have NO structural import link — potential missing abstractions or implicit coupling"
    )]
    fn hidden_deps(&self, Parameters(_params): Parameters<HiddenDepsParam>) -> String {
        let state = self.state.load();
        let temporal = match &state.temporal {
            Some(t) => t,
            None => {
                return to_json(&serde_json::json!({
                    "error": "temporal_unavailable",
                    "reason": "git history not available",
                }));
            }
        };

        let mut hidden: Vec<_> = temporal
            .co_changes
            .iter()
            .filter(|cc| !cc.has_structural_link)
            .collect();

        // Sort by confidence descending
        hidden.sort_by(|a, b| {
            b.confidence
                .partial_cmp(&a.confidence)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.file_a.cmp(&b.file_a))
                .then_with(|| a.file_b.cmp(&b.file_b))
        });

        let result: Vec<serde_json::Value> = hidden
            .iter()
            .map(|cc| {
                serde_json::json!({
                    "file_a": cc.file_a.as_str(),
                    "file_b": cc.file_b.as_str(),
                    "co_change_count": cc.co_change_count,
                    "confidence": cc.confidence,
                })
            })
            .collect();

        to_json(&result)
    }

    // --- T38: Semantic boundaries ---

    #[tool(
        name = "ariadne_boundaries",
        description = "List all detected semantic boundaries (HTTP routes, event channels) grouped by file. Optionally filter by kind."
    )]
    fn boundaries(&self, Parameters(params): Parameters<BoundariesParam>) -> String {
        let state = self.state.load();
        let semantic = match state.semantic.as_ref() {
            Some(s) => s,
            None => {
                return to_json(&serde_json::json!({
                    "message": "No semantic boundary data available. Run `ariadne build` on a project with HTTP routes or event channels to generate.",
                    "hint": "Semantic analysis detects Express/Fastify routes, EventEmitter patterns, and similar."
                }));
            }
        };

        let filtered: BTreeMap<&String, Vec<&crate::serial::BoundaryEntry>> = semantic
            .boundaries
            .iter()
            .map(|(file, entries)| {
                let matched: Vec<&crate::serial::BoundaryEntry> = entries
                    .iter()
                    .filter(|e| {
                        params
                            .kind
                            .as_ref()
                            .map_or(true, |k| e.kind.to_lowercase().contains(&k.to_lowercase()))
                    })
                    .collect();
                (file, matched)
            })
            .filter(|(_, entries)| !entries.is_empty())
            .collect();

        to_json(&serde_json::json!({
            "file_count": filtered.len(),
            "route_count": semantic.route_count,
            "event_count": semantic.event_count,
            "boundaries": filtered.iter().map(|(file, entries)| {
                serde_json::json!({
                    "file": file,
                    "entries": entries.iter().map(|e| {
                        serde_json::json!({
                            "kind": e.kind,
                            "name": e.name,
                            "role": e.role,
                            "line": e.line,
                            "framework": e.framework,
                            "method": e.method,
                        })
                    }).collect::<Vec<_>>(),
                })
            }).collect::<Vec<_>>(),
        }))
    }

    // --- T39: Route map ---

    #[tool(
        name = "ariadne_route_map",
        description = "HTTP route map: routes grouped by path, showing handler file and consumer files"
    )]
    fn route_map(&self, Parameters(_params): Parameters<RouteMapParam>) -> String {
        let state = self.state.load();
        let semantic = match state.semantic.as_ref() {
            Some(s) => s,
            None => {
                return to_json(&serde_json::json!({
                    "message": "No semantic boundary data available. Run `ariadne build` on a project with HTTP routes to generate.",
                }));
            }
        };

        // Collect routes: group by route name, find handlers (define role) and consumers (consume role)
        let mut route_map: BTreeMap<String, serde_json::Value> = BTreeMap::new();

        for (file, entries) in &semantic.boundaries {
            for entry in entries {
                if !entry.kind.to_lowercase().contains("http") && !entry.kind.to_lowercase().contains("route") {
                    continue;
                }
                let route = route_map
                    .entry(entry.name.clone())
                    .or_insert_with(|| serde_json::json!({
                        "path": entry.name,
                        "method": entry.method,
                        "handlers": [],
                        "consumers": [],
                    }));

                let role_lower = entry.role.to_lowercase();
                if role_lower.contains("define") || role_lower.contains("handler") || role_lower.contains("producer") {
                    if let Some(handlers) = route.get_mut("handlers") {
                        if let Some(arr) = handlers.as_array_mut() {
                            arr.push(serde_json::json!({
                                "file": file,
                                "line": entry.line,
                                "framework": entry.framework,
                            }));
                        }
                    }
                } else {
                    if let Some(consumers) = route.get_mut("consumers") {
                        if let Some(arr) = consumers.as_array_mut() {
                            arr.push(serde_json::json!({
                                "file": file,
                                "line": entry.line,
                            }));
                        }
                    }
                }
            }
        }

        to_json(&serde_json::json!({
            "route_count": semantic.route_count,
            "orphan_routes": semantic.orphan_routes,
            "routes": route_map.values().collect::<Vec<_>>(),
        }))
    }

    // --- T40: Event map ---

    #[tool(
        name = "ariadne_event_map",
        description = "Event channel map: events grouped by name, showing producer and consumer files"
    )]
    fn event_map(&self, Parameters(_params): Parameters<EventMapParam>) -> String {
        let state = self.state.load();
        let semantic = match state.semantic.as_ref() {
            Some(s) => s,
            None => {
                return to_json(&serde_json::json!({
                    "message": "No semantic boundary data available. Run `ariadne build` on a project with event channels to generate.",
                }));
            }
        };

        // Collect events: group by event name, find producers and consumers
        let mut event_map: BTreeMap<String, serde_json::Value> = BTreeMap::new();

        for (file, entries) in &semantic.boundaries {
            for entry in entries {
                if !entry.kind.to_lowercase().contains("event") {
                    continue;
                }
                let event = event_map
                    .entry(entry.name.clone())
                    .or_insert_with(|| serde_json::json!({
                        "name": entry.name,
                        "producers": [],
                        "consumers": [],
                    }));

                let role_lower = entry.role.to_lowercase();
                if role_lower.contains("produce") || role_lower.contains("emit") || role_lower.contains("define") {
                    if let Some(producers) = event.get_mut("producers") {
                        if let Some(arr) = producers.as_array_mut() {
                            arr.push(serde_json::json!({
                                "file": file,
                                "line": entry.line,
                                "framework": entry.framework,
                            }));
                        }
                    }
                } else {
                    if let Some(consumers) = event.get_mut("consumers") {
                        if let Some(arr) = consumers.as_array_mut() {
                            arr.push(serde_json::json!({
                                "file": file,
                                "line": entry.line,
                            }));
                        }
                    }
                }
            }
        }

        to_json(&serde_json::json!({
            "event_count": semantic.event_count,
            "orphan_events": semantic.orphan_events,
            "events": event_map.values().collect::<Vec<_>>(),
        }))
    }

    // --- T41: Boundary for file ---

    #[tool(
        name = "ariadne_boundary_for",
        description = "All semantic boundaries (routes, events) defined in a specific file"
    )]
    fn boundary_for(&self, Parameters(params): Parameters<BoundaryForParam>) -> String {
        let state = self.state.load();
        let semantic = match state.semantic.as_ref() {
            Some(s) => s,
            None => {
                return to_json(&serde_json::json!({
                    "message": "No semantic boundary data available. Run `ariadne build` on a project with HTTP routes or event channels to generate.",
                }));
            }
        };

        let entries = semantic.boundaries.get(&params.path);
        match entries {
            Some(entries) => {
                // Also find semantic edges involving this file
                let related_edges: Vec<serde_json::Value> = semantic
                    .edges
                    .iter()
                    .filter(|e| e.from == params.path || e.to == params.path)
                    .map(|e| {
                        serde_json::json!({
                            "from": e.from,
                            "to": e.to,
                            "kind": e.kind,
                            "name": e.name,
                            "confidence": e.confidence,
                        })
                    })
                    .collect();

                to_json(&serde_json::json!({
                    "path": params.path,
                    "boundary_count": entries.len(),
                    "boundaries": entries.iter().map(|e| {
                        serde_json::json!({
                            "kind": e.kind,
                            "name": e.name,
                            "role": e.role,
                            "line": e.line,
                            "framework": e.framework,
                            "method": e.method,
                        })
                    }).collect::<Vec<_>>(),
                    "related_edges": related_edges,
                }))
            }
            None => {
                to_json(&serde_json::json!({
                    "path": params.path,
                    "boundary_count": 0,
                    "boundaries": [],
                    "related_edges": [],
                    "message": "No boundaries found for this file",
                }))
            }
        }
    }

    // --- Recommend: refactor_opportunities ---

    #[tool(
        name = "ariadne_refactor_opportunities",
        description = "Analyze the project for refactoring opportunities. Detects cycles, god files, coupling issues, merge candidates, and interface extraction candidates. Returns Pareto-ranked recommendations with effort/impact estimates."
    )]
    fn refactor_opportunities(
        &self,
        Parameters(params): Parameters<RefactorOpportunitiesParam>,
    ) -> String {
        let state = self.state.load();

        // Validate min_impact parameter (E008)
        let min_impact = if let Some(ref val) = params.min_impact {
            match val.as_str() {
                "low" => Some(recommend::Impact::Low),
                "medium" => Some(recommend::Impact::Medium),
                "high" => Some(recommend::Impact::High),
                other => {
                    return format!(
                        "{{\"error\":\"E008\",\"message\":\"Invalid min_impact value: {}. Expected: low, medium, high\"}}",
                        other
                    );
                }
            }
        } else {
            None
        };

        // Validate scope parameter (E009)
        if let Some(ref scope) = params.scope {
            let has_match = state.graph.nodes.keys().any(|k| k.as_str().starts_with(scope.as_str()));
            if !has_match {
                return format!(
                    "{{\"error\":\"E009\",\"message\":\"No files found in scope: {}\"}}",
                    scope
                );
            }
        }

        // Build AdjacencyIndex
        let index = algo::AdjacencyIndex::build(&state.graph.edges, algo::is_architectural);

        // Compute smells
        let semantic = state.semantic.as_ref().map(crate::serial::boundary_output_to_semantic_state);
        let smells = detect_smells(
            &state.graph,
            &state.stats,
            &state.clusters,
            &state.cluster_metrics,
            state.temporal.as_ref(),
            semantic.as_ref(),
        );

        // Convert centrality keys from String to CanonicalPath
        let centrality: BTreeMap<CanonicalPath, f64> = state
            .stats
            .centrality
            .iter()
            .map(|(k, v)| (CanonicalPath::new(k), *v))
            .collect();

        let result = recommend::refactor::find_refactor_opportunities(
            params.scope.as_deref(),
            &state.graph,
            &index,
            &smells,
            Some(&state.symbol_index),
            Some(&state.call_graph),
            state.temporal.as_ref(),
            &centrality,
            min_impact,
        );

        to_json(&result)
    }

    // --- Recommend: suggest_split ---

    #[tool(
        name = "ariadne_suggest_split",
        description = "Analyze a file and suggest how to split it into smaller, more cohesive modules based on symbol coupling analysis"
    )]
    fn suggest_split(&self, Parameters(params): Parameters<SuggestSplitParam>) -> String {
        let state = self.state.load();
        let path = CanonicalPath::new(&params.path);

        // Look up precomputed centrality
        let centrality = state.stats.centrality.get(params.path.as_str()).copied();

        let result = recommend::split::analyze_split(
            &path,
            &state.graph,
            Some(&state.symbol_index),
            Some(&state.call_graph),
            state.temporal.as_ref(),
            centrality,
        );

        to_json(&result)
    }

    // --- Recommend: suggest_placement ---

    #[tool(
        name = "ariadne_suggest_placement",
        description = "Suggest optimal file location for a new module based on its dependency relationships. Analyzes cluster membership, architectural layers, and dependency patterns to recommend where a new file should be placed."
    )]
    fn suggest_placement(
        &self,
        Parameters(params): Parameters<SuggestPlacementParam>,
    ) -> String {
        let state = self.state.load();

        let depends_on: Vec<CanonicalPath> = params
            .depends_on
            .iter()
            .map(|s| CanonicalPath::new(s))
            .collect();

        let depended_by: Vec<CanonicalPath> = params
            .depended_by
            .iter()
            .map(|s| CanonicalPath::new(s))
            .collect();

        let result = recommend::suggest_placement(
            &params.description,
            &depends_on,
            &depended_by,
            &state.graph.nodes,
            &state.clusters.clusters,
            &state.layer_index,
        );

        to_json(&result)
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
