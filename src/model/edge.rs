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

impl EdgeType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Imports => "imports",
            Self::Tests => "tests",
            Self::ReExports => "re_exports",
            Self::TypeImports => "type_imports",
        }
    }
}

/// A directed edge in the dependency graph.
#[derive(Clone, Debug)]
pub struct Edge {
    pub from: CanonicalPath,
    pub to: CanonicalPath,
    pub edge_type: EdgeType,
    pub symbols: Vec<Symbol>,
}
