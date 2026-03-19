use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::algo::{compress, pagerank, spectral};
use crate::analysis::metrics::{compute_martin_metrics, ClusterMetrics};
use crate::diagnostic::FatalError;
use crate::model::*;
use crate::serial::RawImportOutput;

/// Core in-memory state for the MCP server.
/// Contains the graph plus precomputed indices for O(1) lookups.
#[derive(Debug)]
pub struct GraphState {
    pub graph: ProjectGraph,
    pub stats: StatsOutput,
    pub clusters: ClusterMap,
    /// Precomputed reverse adjacency: target -> edges pointing to it.
    pub reverse_index: BTreeMap<CanonicalPath, Vec<Edge>>,
    /// Precomputed forward adjacency: source -> edges going out from it.
    pub forward_index: BTreeMap<CanonicalPath, Vec<Edge>>,
    /// Arch depth -> files at that layer.
    pub layer_index: BTreeMap<u32, Vec<CanonicalPath>>,
    /// File path -> content hash (from graph nodes).
    pub file_hashes: BTreeMap<CanonicalPath, ContentHash>,
    /// Raw imports per file (for structural freshness checks).
    pub raw_imports: BTreeMap<String, Vec<RawImportOutput>>,
    /// Precomputed Martin metrics per cluster.
    pub cluster_metrics: BTreeMap<ClusterId, ClusterMetrics>,
    /// Precomputed PageRank scores.
    pub pagerank: BTreeMap<CanonicalPath, f64>,
    /// Precomputed combined importance (centrality + PageRank).
    pub combined_importance: BTreeMap<CanonicalPath, f64>,
    /// Precomputed L0 compressed graph.
    pub compressed_l0: CompressedGraph,
    /// Precomputed spectral analysis result.
    pub spectral: spectral::SpectralResult,
    /// Structural diff from the last auto-update (None before first update).
    pub last_diff: Option<StructuralDiff>,
    pub freshness: FreshnessState,
    pub loaded_at: SystemTime,
}

/// Freshness tracking with two-level confidence (D-053).
#[derive(Debug)]
pub struct FreshnessState {
    /// Files whose content hash differs from the in-memory graph.
    pub stale_files: BTreeSet<CanonicalPath>,
    /// Files that are stale AND have changed imports (structural change).
    pub structurally_changed: BTreeSet<CanonicalPath>,
    /// Files on disk not in the graph.
    pub new_files: Vec<PathBuf>,
    /// Files in graph not on disk.
    pub removed_files: Vec<CanonicalPath>,
    /// Hash-level confidence: 1.0 - (stale / total).
    pub hash_confidence: f64,
    /// Structural confidence: accounts for body-only changes.
    pub structural_confidence: f64,
    pub last_full_check: SystemTime,
}

impl FreshnessState {
    /// Create a fresh state with 1.0 confidence (just loaded).
    pub fn new() -> Self {
        Self {
            stale_files: BTreeSet::new(),
            structurally_changed: BTreeSet::new(),
            new_files: Vec::new(),
            removed_files: Vec::new(),
            hash_confidence: 1.0,
            structural_confidence: 1.0,
            last_full_check: SystemTime::now(),
        }
    }

    /// Compute confidence scores from current state and total file count.
    pub fn recompute_confidence(&mut self, total_files: usize) {
        if total_files == 0 {
            self.hash_confidence = 1.0;
            self.structural_confidence = 1.0;
            return;
        }
        let stale = self.stale_files.len() + self.new_files.len() + self.removed_files.len();
        self.hash_confidence = 1.0 - (stale as f64 / total_files as f64);

        let structural_changes =
            self.structurally_changed.len() + self.new_files.len() + self.removed_files.len();
        self.structural_confidence = 1.0 - (structural_changes as f64 / total_files as f64);

        // Clamp to [0.0, 1.0]
        self.hash_confidence = self.hash_confidence.clamp(0.0, 1.0);
        self.structural_confidence = self.structural_confidence.clamp(0.0, 1.0);
    }
}

impl Default for FreshnessState {
    fn default() -> Self {
        Self::new()
    }
}

impl GraphState {
    /// Build GraphState from loaded data, constructing derived indices.
    pub fn from_loaded_data(
        graph: ProjectGraph,
        stats: StatsOutput,
        clusters: ClusterMap,
        raw_imports: BTreeMap<String, Vec<RawImportOutput>>,
    ) -> Self {
        let reverse_index = Self::build_reverse_index(&graph);
        let forward_index = Self::build_forward_index(&graph);
        let layer_index = Self::build_layer_index(&graph);
        let file_hashes = graph
            .nodes
            .iter()
            .map(|(path, node)| (path.clone(), node.hash.clone()))
            .collect();
        let cluster_metrics = compute_martin_metrics(&graph, &clusters);

        let pr = pagerank::pagerank(&graph, 0.85, 100, 1e-6);
        let combined = pagerank::combined_importance(&stats.centrality, &pr);
        let compressed_l0 = compress::compress_l0(&graph, &clusters, &stats);
        let spectral_result = spectral::spectral_analysis(&graph, 200, 1e-6);

        Self {
            graph,
            stats,
            clusters,
            reverse_index,
            forward_index,
            layer_index,
            file_hashes,
            raw_imports,
            cluster_metrics,
            pagerank: pr,
            combined_importance: combined,
            compressed_l0,
            spectral: spectral_result,
            last_diff: None,
            freshness: FreshnessState::new(),
            loaded_at: SystemTime::now(),
        }
    }

    fn build_reverse_index(graph: &ProjectGraph) -> BTreeMap<CanonicalPath, Vec<Edge>> {
        let mut index: BTreeMap<CanonicalPath, Vec<Edge>> = BTreeMap::new();
        for edge in &graph.edges {
            index.entry(edge.to.clone()).or_default().push(edge.clone());
        }
        index
    }

    fn build_forward_index(graph: &ProjectGraph) -> BTreeMap<CanonicalPath, Vec<Edge>> {
        let mut index: BTreeMap<CanonicalPath, Vec<Edge>> = BTreeMap::new();
        for edge in &graph.edges {
            index
                .entry(edge.from.clone())
                .or_default()
                .push(edge.clone());
        }
        index
    }

    fn build_layer_index(graph: &ProjectGraph) -> BTreeMap<u32, Vec<CanonicalPath>> {
        let mut index: BTreeMap<u32, Vec<CanonicalPath>> = BTreeMap::new();
        for (path, node) in &graph.nodes {
            index.entry(node.arch_depth).or_default().push(path.clone());
        }
        index
    }
}

/// Load graph state from disk, or build if missing.
pub fn load_graph_state(
    output_dir: &Path,
    reader: &dyn crate::serial::GraphReader,
) -> Result<GraphState, FatalError> {
    let graph_output = reader.read_graph(output_dir)?;
    let graph: ProjectGraph =
        graph_output
            .try_into()
            .map_err(|e: String| FatalError::GraphCorrupted {
                path: output_dir.join("graph.json"),
                reason: e,
            })?;

    let cluster_output = reader.read_clusters(output_dir)?;
    let clusters: ClusterMap =
        cluster_output
            .try_into()
            .map_err(|e: String| FatalError::GraphCorrupted {
                path: output_dir.join("clusters.json"),
                reason: e,
            })?;

    let stats = reader
        .read_stats(output_dir)?
        .ok_or_else(|| FatalError::StatsNotFound {
            path: output_dir.to_path_buf(),
        })?;

    let raw_imports = reader.read_raw_imports(output_dir)?.unwrap_or_default();

    Ok(GraphState::from_loaded_data(
        graph,
        stats,
        clusters,
        raw_imports,
    ))
}
