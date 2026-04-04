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
    References,
    ProjectRef,
}

impl EdgeType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Imports => "imports",
            Self::Tests => "tests",
            Self::ReExports => "re_exports",
            Self::TypeImports => "type_imports",
            Self::References => "references",
            Self::ProjectRef => "project_ref",
        }
    }
}

impl EdgeType {
    /// Whether this edge type represents an architectural dependency.
    /// Excludes test edges per D-034.
    pub fn is_architectural(self) -> bool {
        matches!(self, Self::Imports | Self::ReExports | Self::TypeImports | Self::ProjectRef)
    }
}

/// A directed edge in the dependency graph.
#[derive(Clone, Debug, Serialize)]
pub struct Edge {
    pub from: CanonicalPath,
    pub to: CanonicalPath,
    pub edge_type: EdgeType,
    pub symbols: Vec<Symbol>,
}
