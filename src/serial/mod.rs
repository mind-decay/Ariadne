pub mod convert;
pub mod json;

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::diagnostic::FatalError;
use crate::model::StatsOutput;

/// Output model for graph.json (D-022).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GraphOutput {
    pub version: u32,
    pub project_root: String,
    pub node_count: usize,
    pub edge_count: usize,
    pub nodes: BTreeMap<String, NodeOutput>,
    pub edges: Vec<(String, String, String, Vec<String>)>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub generated: Option<String>,
}

/// Output model for a single node.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NodeOutput {
    #[serde(rename = "type")]
    pub file_type: String,
    pub layer: String,
    pub arch_depth: u32,
    pub lines: u32,
    pub hash: String,
    pub exports: Vec<String>,
    pub cluster: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub fsd_layer: Option<String>,
}

/// Output model for clusters.json.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ClusterOutput {
    pub clusters: BTreeMap<String, ClusterEntryOutput>,
}

/// Output model for a single cluster entry.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ClusterEntryOutput {
    pub files: Vec<String>,
    pub file_count: usize,
    pub internal_edges: u32,
    pub external_edges: u32,
    pub cohesion: f64,
}

/// Output model for `ariadne query file --format json`.
#[derive(Clone, Debug, Serialize)]
pub struct FileQueryOutput {
    pub path: String,
    pub node: NodeOutput,
    pub incoming_edges: Vec<(String, String, String, Vec<String>)>,
    pub outgoing_edges: Vec<(String, String, String, Vec<String>)>,
    pub centrality: Option<f64>,
    pub cluster: String,
}

/// Serializable raw import for freshness engine (D-054).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct RawImportOutput {
    pub path: String,
    pub symbols: Vec<String>,
    pub is_type_only: bool,
}

/// Output writing abstraction.
pub trait GraphSerializer: Send + Sync {
    fn write_graph(&self, output: &GraphOutput, dir: &Path) -> Result<(), FatalError>;
    fn write_clusters(&self, clusters: &ClusterOutput, dir: &Path) -> Result<(), FatalError>;
    fn write_stats(&self, stats: &StatsOutput, dir: &Path) -> Result<(), FatalError>;
    fn write_raw_imports(
        &self,
        imports: &BTreeMap<String, Vec<RawImportOutput>>,
        dir: &Path,
    ) -> Result<(), FatalError>;
}

/// Read-side counterpart to GraphSerializer. Separate trait because
/// read and write have different error semantics (D-032).
pub trait GraphReader: Send + Sync {
    fn read_graph(&self, dir: &Path) -> Result<GraphOutput, FatalError>;
    fn read_clusters(&self, dir: &Path) -> Result<ClusterOutput, FatalError>;
    fn read_stats(&self, dir: &Path) -> Result<Option<StatsOutput>, FatalError>;
    fn read_raw_imports(
        &self,
        dir: &Path,
    ) -> Result<Option<BTreeMap<String, Vec<RawImportOutput>>>, FatalError>;
}
