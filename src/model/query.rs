use std::collections::BTreeMap;

use super::edge::Edge;
use super::node::Node;
use super::types::CanonicalPath;

/// Result of a subgraph extraction query.
#[derive(Debug, Clone)]
pub struct SubgraphResult {
    pub nodes: BTreeMap<CanonicalPath, Node>,
    pub edges: Vec<Edge>,
    pub center_files: Vec<CanonicalPath>,
    pub depth: u32,
}
