pub mod dotnet;
pub mod edges;
pub mod events;
pub mod http;
pub mod java;
pub mod nextjs;
pub mod react;

pub use java::JavaBoundaryExtractor;
pub use nextjs::NextBoundaryExtractor;
pub use react::ReactBoundaryExtractor;

use std::collections::{BTreeMap, HashMap};

use crate::model::semantic::{Boundary, BoundaryKind, SemanticState};
use crate::model::types::CanonicalPath;

/// Trait for extracting semantic boundaries from parsed source files.
///
/// Implementations detect patterns like HTTP route handlers, event emitters/listeners,
/// and other architectural boundary markers. Multiple extractors can be registered
/// per file extension (unlike `SymbolExtractor` which is one-per-extension).
pub trait BoundaryExtractor: Send + Sync {
    /// File extensions this extractor handles (e.g., ["ts", "tsx", "js"]).
    fn extensions(&self) -> &[&str];

    /// Extract boundaries from a parsed tree-sitter tree.
    fn extract(
        &self,
        tree: &tree_sitter::Tree,
        source: &[u8],
        path: &CanonicalPath,
    ) -> Vec<Boundary>;
}

/// Registry of boundary extractors, indexed by file extension.
///
/// Unlike `ParserRegistry` (one parser per extension), multiple extractors
/// can be registered for the same extension (e.g., HTTP + events for .ts files).
pub struct ExtractorRegistry {
    extractors: Vec<Box<dyn BoundaryExtractor>>,
    extension_index: HashMap<String, Vec<usize>>,
}

impl ExtractorRegistry {
    pub fn new() -> Self {
        Self {
            extractors: Vec::new(),
            extension_index: HashMap::new(),
        }
    }

    /// Register a boundary extractor for its declared extensions.
    pub fn register(&mut self, extractor: Box<dyn BoundaryExtractor>) {
        let index = self.extractors.len();
        for ext in extractor.extensions() {
            self.extension_index
                .entry(ext.to_string())
                .or_default()
                .push(index);
        }
        self.extractors.push(extractor);
    }

    /// Look up all boundary extractors for a given file extension.
    pub fn extractors_for(&self, extension: &str) -> Vec<&dyn BoundaryExtractor> {
        self.extension_index
            .get(extension)
            .map(|indices| {
                indices
                    .iter()
                    .map(|&i| self.extractors[i].as_ref())
                    .collect()
            })
            .unwrap_or_default()
    }
}

impl Default for ExtractorRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Orchestrate semantic analysis from collected boundary data.
///
/// Takes all boundaries discovered during parsing and builds semantic edges
/// connecting files that share the same boundary names (e.g., producer/consumer
/// of the same HTTP route or event channel).
///
/// Returns a `SemanticState` containing boundaries, edges, and orphan lists.
pub fn analyze(file_boundaries: BTreeMap<CanonicalPath, Vec<Boundary>>) -> SemanticState {
    // Count routes and events
    let mut route_count: u32 = 0;
    let mut event_count: u32 = 0;
    for boundaries in file_boundaries.values() {
        for b in boundaries {
            match b.kind {
                BoundaryKind::HttpRoute => route_count += 1,
                BoundaryKind::EventChannel => event_count += 1,
            }
        }
    }

    // Build semantic edges (stub — full implementation in ST-5)
    let (edges, orphan_routes, orphan_events) = edges::build_semantic_edges(&file_boundaries);

    SemanticState {
        boundaries: file_boundaries,
        edges,
        route_count,
        event_count,
        orphan_routes,
        orphan_events,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_registry_returns_no_extractors() {
        let registry = ExtractorRegistry::new();
        assert!(registry.extractors_for("ts").is_empty());
    }

    #[test]
    fn analyze_empty_boundaries_returns_empty_state() {
        let state = analyze(BTreeMap::new());
        assert!(state.edges.is_empty());
        assert_eq!(state.route_count, 0);
        assert_eq!(state.event_count, 0);
        assert!(state.orphan_routes.is_empty());
        assert!(state.orphan_events.is_empty());
    }
}
