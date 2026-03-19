use serde::{Deserialize, Serialize};

/// Hierarchical compression level for graph views (D-041).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum CompressionLevel {
    /// L0: cluster-level view (~10-30 nodes)
    Project,
    /// L1: file-level within a cluster (~50-200 nodes)
    Cluster,
    /// L2: single file + N-hop neighborhood
    File,
}

/// Node type in a compressed graph.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum CompressedNodeType {
    Cluster,
    File,
}

/// A node in a compressed graph view.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressedNode {
    pub name: String,
    pub node_type: CompressedNodeType,
    /// Number of files (L0 only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_count: Option<u32>,
    /// Cluster cohesion (L0 only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cohesion: Option<f64>,
    /// Top files by centrality (L0: top-3; L1/L2: empty).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub key_files: Vec<String>,
    /// File type classification (L1/L2 only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_type: Option<String>,
    /// Architectural layer (L1/L2 only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub layer: Option<String>,
    /// Betweenness centrality (L1/L2 only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub centrality: Option<f64>,
}

/// An edge in a compressed graph view.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressedEdge {
    pub from: String,
    pub to: String,
    /// L0: count of inter-cluster edges; L1/L2: 1.
    pub weight: u32,
    /// Edge type (L1/L2 only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub edge_type: Option<String>,
}

/// A compressed graph at a specific zoom level (D-041, D-062).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressedGraph {
    pub level: CompressionLevel,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub focus: Option<String>,
    pub nodes: Vec<CompressedNode>,
    pub edges: Vec<CompressedEdge>,
    /// Estimated token count (JSON bytes / 4).
    pub token_estimate: u32,
}
