use std::collections::BTreeMap;

use super::edge::Edge;
use super::node::Node;
use super::types::{CanonicalPath, ClusterId};

/// The internal project dependency graph.
pub struct ProjectGraph {
    pub nodes: BTreeMap<CanonicalPath, Node>,
    pub edges: Vec<Edge>,
}

/// A cluster of related files.
#[derive(Clone, Debug)]
pub struct Cluster {
    pub files: Vec<CanonicalPath>,
    pub file_count: usize,
    pub internal_edges: u32,
    pub external_edges: u32,
    pub cohesion: f64,
}

/// Map of all clusters in the project.
pub struct ClusterMap {
    pub clusters: BTreeMap<ClusterId, Cluster>,
}
