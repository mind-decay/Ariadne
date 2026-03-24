use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::model::edge::Edge;
use crate::model::node::Node;
use crate::model::symbol::{LineSpan, SymbolDef, SymbolKind};
use crate::model::types::CanonicalPath;

/// Index of all symbols across the project graph, built at load time (D-078).
/// Supports lookup by name, by file, and usage tracking.
#[derive(Debug)]
pub struct SymbolIndex {
    by_name: BTreeMap<String, Vec<SymbolLocation>>,
    by_file: BTreeMap<CanonicalPath, Vec<SymbolDef>>,
    usages: BTreeMap<(CanonicalPath, String), Vec<SymbolUsage>>,
}

/// A symbol's location in the project.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct SymbolLocation {
    pub file: CanonicalPath,
    pub name: String,
    pub kind: SymbolKind,
    pub span: LineSpan,
}

/// A usage of a symbol from another file.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct SymbolUsage {
    pub file: CanonicalPath,
    pub line: u32,
    pub usage_kind: UsageKind,
}

/// The kind of symbol usage.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UsageKind {
    Import,
    Call,
    TypeReference,
    Inheritance,
}

impl SymbolIndex {
    /// Build a SymbolIndex from the project graph nodes and edges.
    /// Nodes provide symbol definitions; edges provide usage information.
    pub fn build(nodes: &BTreeMap<CanonicalPath, Node>, edges: &[Edge]) -> Self {
        let mut by_name: BTreeMap<String, Vec<SymbolLocation>> = BTreeMap::new();
        let mut by_file: BTreeMap<CanonicalPath, Vec<SymbolDef>> = BTreeMap::new();
        let mut usages: BTreeMap<(CanonicalPath, String), Vec<SymbolUsage>> = BTreeMap::new();

        // Index all symbol definitions from nodes
        for (path, node) in nodes {
            if !node.symbols.is_empty() {
                by_file.insert(path.clone(), node.symbols.clone());
            }

            for sym in &node.symbols {
                by_name
                    .entry(sym.name.clone())
                    .or_default()
                    .push(SymbolLocation {
                        file: path.clone(),
                        name: sym.name.clone(),
                        kind: sym.kind,
                        span: sym.span,
                    });
            }
        }

        // Build usage index from edges: each edge's symbols represent imports
        for edge in edges {
            for symbol_name in &edge.symbols {
                let key = (edge.to.clone(), symbol_name.as_str().to_string());
                usages.entry(key).or_default().push(SymbolUsage {
                    file: edge.from.clone(),
                    line: 0, // Line info not available from edges
                    usage_kind: UsageKind::Import,
                });
            }
        }

        // Sort all vectors for deterministic output
        for locs in by_name.values_mut() {
            locs.sort();
        }
        for usages_vec in usages.values_mut() {
            usages_vec.sort();
        }

        Self {
            by_name,
            by_file,
            usages,
        }
    }

    /// Get all symbols defined in a file.
    pub fn symbols_for_file(&self, path: &CanonicalPath) -> Option<&[SymbolDef]> {
        self.by_file.get(path).map(|v| v.as_slice())
    }

    /// Search symbols by name (case-insensitive substring match, D-080).
    /// Optionally filter by SymbolKind. Returns up to 100 results.
    pub fn search(&self, query: &str, kind: Option<SymbolKind>) -> Vec<&SymbolLocation> {
        let query_lower = query.to_lowercase();
        let mut results = Vec::new();

        for (name, locations) in &self.by_name {
            if !name.to_lowercase().contains(&query_lower) {
                continue;
            }
            for loc in locations {
                if let Some(kind_filter) = kind {
                    if loc.kind != kind_filter {
                        continue;
                    }
                }
                results.push(loc);
                if results.len() >= 100 {
                    return results;
                }
            }
        }

        results
    }

    /// Get usages of a symbol defined in a specific file.
    pub fn usages_of(&self, file: &CanonicalPath, name: &str) -> Option<&[SymbolUsage]> {
        let key = (file.clone(), name.to_string());
        self.usages.get(&key).map(|v| v.as_slice())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::node::{ArchLayer, FileType};
    use crate::model::symbol::Visibility;
    use crate::model::types::{ClusterId, ContentHash};

    fn make_node(path: &str, symbols: Vec<SymbolDef>) -> (CanonicalPath, Node) {
        let cp = CanonicalPath::new(path);
        let node = Node {
            file_type: FileType::Source,
            layer: ArchLayer::Util,
            fsd_layer: None,
            arch_depth: 0,
            lines: 10,
            hash: ContentHash::new("abc123".to_string()),
            exports: Vec::new(),
            cluster: ClusterId::new("root"),
            symbols,
        };
        (cp, node)
    }

    fn make_sym(name: &str, kind: SymbolKind) -> SymbolDef {
        SymbolDef {
            name: name.to_string(),
            kind,
            visibility: Visibility::Public,
            signature: None,
            span: LineSpan { start: 1, end: 5 },
            parent: None,
        }
    }

    #[test]
    fn empty_query_matches_all_symbols() {
        let (cp, node) = make_node(
            "src/a.ts",
            vec![
                make_sym("foo", SymbolKind::Function),
                make_sym("bar", SymbolKind::Function),
                make_sym("Baz", SymbolKind::Class),
            ],
        );
        let nodes: BTreeMap<CanonicalPath, Node> =
            std::iter::once((cp, node)).collect();
        let idx = SymbolIndex::build(&nodes, &[]);

        // Empty string contains("") is always true — matches everything
        let results = idx.search("", None);
        assert_eq!(
            results.len(),
            3,
            "Empty query matches all symbols due to str::contains behavior"
        );
    }

    #[test]
    fn symbols_for_file_returns_none_for_missing() {
        let (cp, node) = make_node("src/a.ts", vec![make_sym("foo", SymbolKind::Function)]);
        let nodes: BTreeMap<CanonicalPath, Node> =
            std::iter::once((cp, node)).collect();
        let idx = SymbolIndex::build(&nodes, &[]);

        let missing = CanonicalPath::new("src/nonexistent.ts");
        assert!(idx.symbols_for_file(&missing).is_none());
    }

    #[test]
    fn symbols_for_file_finds_existing() {
        let (cp, node) = make_node(
            "src/a.ts",
            vec![
                make_sym("foo", SymbolKind::Function),
                make_sym("bar", SymbolKind::Class),
            ],
        );
        let nodes: BTreeMap<CanonicalPath, Node> =
            std::iter::once((cp.clone(), node)).collect();
        let idx = SymbolIndex::build(&nodes, &[]);

        let syms = idx.symbols_for_file(&cp).unwrap();
        assert_eq!(syms.len(), 2);
        assert!(syms.iter().any(|s| s.name == "foo"));
        assert!(syms.iter().any(|s| s.name == "bar"));
    }
}
