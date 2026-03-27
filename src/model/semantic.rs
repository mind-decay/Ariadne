use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::types::CanonicalPath;

/// A detected semantic boundary (route, event, etc.)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct Boundary {
    pub kind: BoundaryKind,
    pub name: String,
    pub role: BoundaryRole,
    pub file: CanonicalPath,
    pub line: u32,
    pub framework: Option<String>,
    pub method: Option<String>,
}

/// Classification of boundary types.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BoundaryKind {
    HttpRoute,
    EventChannel,
    // Future Phase 8b: DatabaseTable, ConfigReference, DiBinding, GrpcService, MessageQueue
}

/// Role of a file with respect to a boundary.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BoundaryRole {
    Producer,
    Consumer,
    Both,
}

/// A probabilistic edge connecting two files via a shared boundary.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SemanticEdge {
    pub from: CanonicalPath,
    pub to: CanonicalPath,
    pub boundary_kind: BoundaryKind,
    pub name: String,
    pub confidence: f64,
}

/// Top-level container for all semantic boundary data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticState {
    pub boundaries: BTreeMap<CanonicalPath, Vec<Boundary>>,
    pub edges: Vec<SemanticEdge>,
    pub route_count: u32,
    pub event_count: u32,
    pub orphan_routes: Vec<String>,
    pub orphan_events: Vec<String>,
}
