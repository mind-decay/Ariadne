use serde::Serialize;

use super::types::{CanonicalPath, Symbol};

/// Edge type classification.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeType {
    Imports,
    Tests,
    ReExports,
    TypeImports,
}

/// A directed edge in the dependency graph.
#[derive(Clone, Debug)]
pub struct Edge {
    pub from: CanonicalPath,
    pub to: CanonicalPath,
    pub edge_type: EdgeType,
    pub symbols: Vec<Symbol>,
}
