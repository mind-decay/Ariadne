pub mod json;

use std::collections::BTreeMap;
use std::path::Path;

use serde::Serialize;

use crate::diagnostic::FatalError;

/// Output model for graph.json (D-022).
#[derive(Clone, Debug, Serialize)]
pub struct GraphOutput {
    pub version: u32,
    pub project_root: String,
    pub node_count: usize,
    pub edge_count: usize,
    pub nodes: BTreeMap<String, NodeOutput>,
    pub edges: Vec<(String, String, String, Vec<String>)>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub generated: Option<String>,
}

/// Output model for a single node.
#[derive(Clone, Debug, Serialize)]
pub struct NodeOutput {
    #[serde(rename = "type")]
    pub file_type: String,
    pub layer: String,
    pub arch_depth: u32,
    pub lines: u32,
    pub hash: String,
    pub exports: Vec<String>,
    pub cluster: String,
}

/// Output model for clusters.json.
#[derive(Clone, Debug, Serialize)]
pub struct ClusterOutput {
    pub clusters: BTreeMap<String, ClusterEntryOutput>,
}

/// Output model for a single cluster entry.
#[derive(Clone, Debug, Serialize)]
pub struct ClusterEntryOutput {
    pub files: Vec<String>,
    pub file_count: usize,
    pub internal_edges: u32,
    pub external_edges: u32,
    pub cohesion: f64,
}

/// Output writing abstraction.
pub trait GraphSerializer: Send + Sync {
    fn write_graph(&self, output: &GraphOutput, dir: &Path) -> Result<(), FatalError>;
    fn write_clusters(&self, clusters: &ClusterOutput, dir: &Path) -> Result<(), FatalError>;
}
