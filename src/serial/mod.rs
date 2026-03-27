pub mod convert;
pub mod json;

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::diagnostic::FatalError;
use crate::model::semantic::{Boundary, BoundaryKind, BoundaryRole, SemanticEdge, SemanticState};
use crate::model::symbol::SymbolDef;
use crate::model::types::CanonicalPath;
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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub symbols: Vec<SymbolDef>,
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

/// Output model for boundaries.json (D-103).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BoundaryOutput {
    pub version: u32,
    pub boundaries: BTreeMap<String, Vec<BoundaryEntry>>,
    pub edges: Vec<SemanticEdgeEntry>,
    pub route_count: u32,
    pub event_count: u32,
    pub orphan_routes: Vec<String>,
    pub orphan_events: Vec<String>,
}

/// A single boundary entry within a file.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BoundaryEntry {
    pub kind: String,
    pub name: String,
    pub role: String,
    pub line: u32,
    pub framework: Option<String>,
    pub method: Option<String>,
}

/// A semantic edge connecting two files via a shared boundary.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SemanticEdgeEntry {
    pub from: String,
    pub to: String,
    pub kind: String,
    pub name: String,
    pub confidence: f64,
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
    fn write_boundaries(
        &self,
        output: &BoundaryOutput,
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
    fn read_boundaries(
        &self,
        dir: &Path,
    ) -> Result<Option<BoundaryOutput>, FatalError>;
}

/// Convert a `SemanticState` into a serializable `BoundaryOutput`.
pub fn semantic_state_to_boundary_output(state: &SemanticState) -> BoundaryOutput {
    let boundaries: BTreeMap<String, Vec<BoundaryEntry>> = state
        .boundaries
        .iter()
        .map(|(path, bs)| {
            let entries = bs
                .iter()
                .map(|b| BoundaryEntry {
                    kind: format!("{:?}", b.kind),
                    name: b.name.clone(),
                    role: format!("{:?}", b.role),
                    line: b.line,
                    framework: b.framework.clone(),
                    method: b.method.clone(),
                })
                .collect();
            (path.as_str().to_string(), entries)
        })
        .collect();

    let edges: Vec<SemanticEdgeEntry> = state
        .edges
        .iter()
        .map(|e| SemanticEdgeEntry {
            from: e.from.as_str().to_string(),
            to: e.to.as_str().to_string(),
            kind: format!("{:?}", e.boundary_kind),
            name: e.name.clone(),
            confidence: e.confidence,
        })
        .collect();

    BoundaryOutput {
        version: 1,
        boundaries,
        edges,
        route_count: state.route_count,
        event_count: state.event_count,
        orphan_routes: state.orphan_routes.clone(),
        orphan_events: state.orphan_events.clone(),
    }
}

/// Convert a `BoundaryOutput` back into a `SemanticState`.
///
/// This is the inverse of `semantic_state_to_boundary_output`. String-based
/// kind/role fields are parsed back into their enum equivalents; unrecognized
/// values default to `HttpRoute` / `Producer`.
pub fn boundary_output_to_semantic_state(output: &BoundaryOutput) -> SemanticState {
    let boundaries: BTreeMap<CanonicalPath, Vec<Boundary>> = output
        .boundaries
        .iter()
        .map(|(path, entries)| {
            let cp = CanonicalPath::new(path);
            let bs = entries
                .iter()
                .map(|e| Boundary {
                    kind: parse_boundary_kind(&e.kind),
                    name: e.name.clone(),
                    role: parse_boundary_role(&e.role),
                    file: cp.clone(),
                    line: e.line,
                    framework: e.framework.clone(),
                    method: e.method.clone(),
                })
                .collect();
            (cp, bs)
        })
        .collect();

    let edges: Vec<SemanticEdge> = output
        .edges
        .iter()
        .map(|e| SemanticEdge {
            from: CanonicalPath::new(&e.from),
            to: CanonicalPath::new(&e.to),
            boundary_kind: parse_boundary_kind(&e.kind),
            name: e.name.clone(),
            confidence: e.confidence,
        })
        .collect();

    SemanticState {
        boundaries,
        edges,
        route_count: output.route_count,
        event_count: output.event_count,
        orphan_routes: output.orphan_routes.clone(),
        orphan_events: output.orphan_events.clone(),
    }
}

fn parse_boundary_kind(s: &str) -> BoundaryKind {
    match s {
        "HttpRoute" => BoundaryKind::HttpRoute,
        "EventChannel" => BoundaryKind::EventChannel,
        _ => BoundaryKind::HttpRoute,
    }
}

fn parse_boundary_role(s: &str) -> BoundaryRole {
    match s {
        "Producer" => BoundaryRole::Producer,
        "Consumer" => BoundaryRole::Consumer,
        "Both" => BoundaryRole::Both,
        _ => BoundaryRole::Producer,
    }
}
