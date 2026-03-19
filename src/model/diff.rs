use serde::Serialize;

use super::edge::Edge;
use super::smell::ArchSmell;
use super::types::{CanonicalPath, ClusterId};

/// Structural changes between two graph snapshots.
#[derive(Debug, Clone, Serialize)]
pub struct StructuralDiff {
    pub added_nodes: Vec<CanonicalPath>,
    pub removed_nodes: Vec<CanonicalPath>,
    pub added_edges: Vec<Edge>,
    pub removed_edges: Vec<Edge>,
    pub changed_layers: Vec<LayerChange>,
    pub changed_clusters: Vec<ClusterChange>,
    pub new_cycles: Vec<Vec<CanonicalPath>>,
    pub resolved_cycles: Vec<Vec<CanonicalPath>>,
    pub new_smells: Vec<ArchSmell>,
    pub resolved_smells: Vec<ArchSmell>,
    pub summary: DiffSummary,
}

/// A file whose architectural depth changed.
#[derive(Debug, Clone, Serialize)]
pub struct LayerChange {
    pub file: CanonicalPath,
    pub old_depth: u32,
    pub new_depth: u32,
}

/// A file whose cluster assignment changed.
#[derive(Debug, Clone, Serialize)]
pub struct ClusterChange {
    pub file: CanonicalPath,
    pub old_cluster: ClusterId,
    pub new_cluster: ClusterId,
}

/// Summary statistics for a structural diff.
#[derive(Debug, Clone, Serialize)]
pub struct DiffSummary {
    pub structural_change_magnitude: f64,
    pub change_type: ChangeClassification,
}

/// How the change is classified based on heuristics.
#[derive(Debug, Clone, Copy, Serialize, PartialEq)]
pub enum ChangeClassification {
    Additive,
    Refactor,
    Migration,
    Breaking,
}
