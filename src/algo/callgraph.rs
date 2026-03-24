use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::model::edge::{Edge, EdgeType};
use crate::model::symbol_index::SymbolIndex;
use crate::model::types::CanonicalPath;

/// Cross-file call graph built from import edges matched against symbol definitions (D-079).
///
/// Scope: cross-file via imports ONLY. Matches imported symbol names against
/// SymbolDefs in the target file. No intra-file body analysis.
#[derive(Debug)]
pub struct CallGraph {
    /// (target_file, symbol_name) -> list of files that import/call this symbol
    callers: BTreeMap<(CanonicalPath, String), Vec<CallEdge>>,
    /// (source_file, symbol_name) -> list of symbols this file imports/calls from other files
    callees: BTreeMap<(CanonicalPath, String), Vec<CallEdge>>,
}

/// A single edge in the call graph, representing a cross-file symbol reference.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct CallEdge {
    pub file: CanonicalPath,
    pub symbol: Option<String>,
    pub edge_kind: CallEdgeKind,
}

/// The kind of cross-file symbol reference.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CallEdgeKind {
    Import,
    TypeImport,
    ReExport,
}

impl CallEdgeKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Import => "import",
            Self::TypeImport => "type_import",
            Self::ReExport => "re_export",
        }
    }
}

impl CallGraph {
    /// Build a call graph from graph edges and symbol index.
    ///
    /// For each architectural edge:
    /// 1. Get source (importing file) and target (exporting file)
    /// 2. Get imported symbol names from edge.symbols
    /// 3. For each symbol, check if it exists in symbol_index for the target file
    /// 4. If found: create bidirectional call edges
    /// 5. Determine edge_kind from EdgeType
    pub fn build(edges: &[Edge], symbol_index: &SymbolIndex) -> Self {
        let mut callers: BTreeMap<(CanonicalPath, String), Vec<CallEdge>> = BTreeMap::new();
        let mut callees: BTreeMap<(CanonicalPath, String), Vec<CallEdge>> = BTreeMap::new();

        for edge in edges {
            let edge_kind = match edge.edge_type {
                EdgeType::Imports => CallEdgeKind::Import,
                EdgeType::TypeImports => CallEdgeKind::TypeImport,
                EdgeType::ReExports => CallEdgeKind::ReExport,
                // Skip non-architectural edges (Tests, References)
                _ => continue,
            };

            let source = &edge.from; // importing file
            let target = &edge.to; // exporting file

            // Get target file's symbol definitions
            let target_symbols = match symbol_index.symbols_for_file(target) {
                Some(syms) => syms,
                None => continue,
            };

            for imported_name in &edge.symbols {
                let name_str = imported_name.as_str();

                // Check if this imported name matches a symbol defined in the target file
                let found = target_symbols.iter().any(|s| s.name == name_str);
                if !found {
                    continue;
                }

                // Caller edge: target file's symbol is called by source file
                callers
                    .entry((target.clone(), name_str.to_string()))
                    .or_default()
                    .push(CallEdge {
                        file: source.clone(),
                        symbol: Some(name_str.to_string()),
                        edge_kind,
                    });

                // Callee edge: source file calls target file's symbol
                callees
                    .entry((source.clone(), name_str.to_string()))
                    .or_default()
                    .push(CallEdge {
                        file: target.clone(),
                        symbol: Some(name_str.to_string()),
                        edge_kind,
                    });
            }
        }

        // Sort all vectors for deterministic output
        for edges_vec in callers.values_mut() {
            edges_vec.sort();
            edges_vec.dedup();
        }
        for edges_vec in callees.values_mut() {
            edges_vec.sort();
            edges_vec.dedup();
        }

        Self { callers, callees }
    }

    /// Get callers of a symbol defined in a file.
    /// Returns the list of files/symbols that import this symbol.
    pub fn callers_of(&self, file: &CanonicalPath, symbol: &str) -> &[CallEdge] {
        let key = (file.clone(), symbol.to_string());
        self.callers.get(&key).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Get callees from a symbol usage in a file.
    /// Returns the list of files/symbols that this file imports via this symbol name.
    pub fn callees_of(&self, file: &CanonicalPath, symbol: &str) -> &[CallEdge] {
        let key = (file.clone(), symbol.to_string());
        self.callees.get(&key).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Get all caller entries for a given file (all symbols).
    pub fn all_callers_for_file(
        &self,
        file: &CanonicalPath,
    ) -> Vec<(&str, &[CallEdge])> {
        let mut results: Vec<(&str, &[CallEdge])> = Vec::new();
        for ((f, sym), edges) in &self.callers {
            if f == file {
                results.push((sym.as_str(), edges.as_slice()));
            }
        }
        results
    }

    /// Get all callee entries for a given file (all symbols).
    pub fn all_callees_for_file(
        &self,
        file: &CanonicalPath,
    ) -> Vec<(&str, &[CallEdge])> {
        let mut results: Vec<(&str, &[CallEdge])> = Vec::new();
        for ((f, sym), edges) in &self.callees {
            if f == file {
                results.push((sym.as_str(), edges.as_slice()));
            }
        }
        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::node::{ArchLayer, FileType, Node};
    use crate::model::symbol::{LineSpan, SymbolDef, Visibility};
    use crate::model::types::{ClusterId, ContentHash, Symbol};
    use std::collections::BTreeMap;

    fn make_node(symbols: Vec<SymbolDef>) -> Node {
        Node {
            file_type: FileType::Source,
            layer: ArchLayer::Util,
            fsd_layer: None,
            arch_depth: 0,
            lines: 10,
            hash: ContentHash::new("abc123".to_string()),
            exports: Vec::new(),
            cluster: ClusterId::new("root"),
            symbols,
        }
    }

    fn make_sym(name: &str) -> SymbolDef {
        SymbolDef {
            name: name.to_string(),
            kind: crate::model::SymbolKind::Function,
            visibility: Visibility::Public,
            span: LineSpan { start: 1, end: 5 },
            signature: None,
            parent: None,
        }
    }

    fn make_edge(from: &str, to: &str, edge_type: EdgeType, symbols: &[&str]) -> Edge {
        Edge {
            from: CanonicalPath::new(from),
            to: CanonicalPath::new(to),
            edge_type,
            symbols: symbols.iter().map(|s| Symbol::new(s.to_string())).collect(),
        }
    }

    #[test]
    fn empty_graph() {
        let idx = SymbolIndex::build(&BTreeMap::new(), &[]);
        let cg = CallGraph::build(&[], &idx);
        let cp = CanonicalPath::new("src/a.ts");
        assert!(cg.callers_of(&cp, "foo").is_empty());
        assert!(cg.callees_of(&cp, "foo").is_empty());
    }

    #[test]
    fn simple_import_creates_bidirectional_edges() {
        // File B defines "greet", File A imports "greet" from B
        let cp_a = CanonicalPath::new("src/a.ts");
        let cp_b = CanonicalPath::new("src/b.ts");

        let mut nodes = BTreeMap::new();
        nodes.insert(cp_a.clone(), make_node(vec![]));
        nodes.insert(cp_b.clone(), make_node(vec![make_sym("greet")]));

        let edges = vec![make_edge("src/a.ts", "src/b.ts", EdgeType::Imports, &["greet"])];
        let idx = SymbolIndex::build(&nodes, &edges);
        let cg = CallGraph::build(&edges, &idx);

        // B's "greet" is called by A
        let callers = cg.callers_of(&cp_b, "greet");
        assert_eq!(callers.len(), 1);
        assert_eq!(callers[0].file, cp_a);
        assert_eq!(callers[0].edge_kind, CallEdgeKind::Import);

        // A's usage of "greet" points to B
        let callees = cg.callees_of(&cp_a, "greet");
        assert_eq!(callees.len(), 1);
        assert_eq!(callees[0].file, cp_b);
        assert_eq!(callees[0].edge_kind, CallEdgeKind::Import);
    }

    #[test]
    fn unmatched_symbol_ignored() {
        // File B defines "greet", but edge imports "nonexistent"
        let cp_a = CanonicalPath::new("src/a.ts");
        let cp_b = CanonicalPath::new("src/b.ts");

        let mut nodes = BTreeMap::new();
        nodes.insert(cp_a.clone(), make_node(vec![]));
        nodes.insert(cp_b.clone(), make_node(vec![make_sym("greet")]));

        let edges = vec![make_edge(
            "src/a.ts",
            "src/b.ts",
            EdgeType::Imports,
            &["nonexistent"],
        )];
        let idx = SymbolIndex::build(&nodes, &edges);
        let cg = CallGraph::build(&edges, &idx);

        assert!(cg.callers_of(&cp_b, "greet").is_empty());
        assert!(cg.callees_of(&cp_a, "nonexistent").is_empty());
    }

    #[test]
    fn test_edges_skipped() {
        let cp_a = CanonicalPath::new("src/a.ts");
        let cp_b = CanonicalPath::new("src/b.ts");

        let mut nodes = BTreeMap::new();
        nodes.insert(cp_a.clone(), make_node(vec![]));
        nodes.insert(cp_b.clone(), make_node(vec![make_sym("greet")]));

        let edges = vec![make_edge("src/a.ts", "src/b.ts", EdgeType::Tests, &["greet"])];
        let idx = SymbolIndex::build(&nodes, &edges);
        let cg = CallGraph::build(&edges, &idx);

        assert!(cg.callers_of(&cp_b, "greet").is_empty());
    }

    #[test]
    fn type_import_edge_kind() {
        let cp_a = CanonicalPath::new("src/a.ts");
        let cp_b = CanonicalPath::new("src/b.ts");

        let mut nodes = BTreeMap::new();
        nodes.insert(cp_a.clone(), make_node(vec![]));
        nodes.insert(cp_b.clone(), make_node(vec![make_sym("MyType")]));

        let edges = vec![make_edge(
            "src/a.ts",
            "src/b.ts",
            EdgeType::TypeImports,
            &["MyType"],
        )];
        let idx = SymbolIndex::build(&nodes, &edges);
        let cg = CallGraph::build(&edges, &idx);

        let callers = cg.callers_of(&cp_b, "MyType");
        assert_eq!(callers.len(), 1);
        assert_eq!(callers[0].edge_kind, CallEdgeKind::TypeImport);
    }

    #[test]
    fn re_export_edge_kind() {
        let cp_a = CanonicalPath::new("src/index.ts");
        let cp_b = CanonicalPath::new("src/b.ts");

        let mut nodes = BTreeMap::new();
        nodes.insert(cp_a.clone(), make_node(vec![]));
        nodes.insert(cp_b.clone(), make_node(vec![make_sym("helper")]));

        let edges = vec![make_edge(
            "src/index.ts",
            "src/b.ts",
            EdgeType::ReExports,
            &["helper"],
        )];
        let idx = SymbolIndex::build(&nodes, &edges);
        let cg = CallGraph::build(&edges, &idx);

        let callers = cg.callers_of(&cp_b, "helper");
        assert_eq!(callers.len(), 1);
        assert_eq!(callers[0].edge_kind, CallEdgeKind::ReExport);
    }

    #[test]
    fn multiple_callers_same_symbol() {
        let cp_a = CanonicalPath::new("src/a.ts");
        let cp_b = CanonicalPath::new("src/b.ts");
        let cp_c = CanonicalPath::new("src/c.ts");

        let mut nodes = BTreeMap::new();
        nodes.insert(cp_a.clone(), make_node(vec![]));
        nodes.insert(cp_b.clone(), make_node(vec![]));
        nodes.insert(cp_c.clone(), make_node(vec![make_sym("shared")]));

        let edges = vec![
            make_edge("src/a.ts", "src/c.ts", EdgeType::Imports, &["shared"]),
            make_edge("src/b.ts", "src/c.ts", EdgeType::Imports, &["shared"]),
        ];
        let idx = SymbolIndex::build(&nodes, &edges);
        let cg = CallGraph::build(&edges, &idx);

        let callers = cg.callers_of(&cp_c, "shared");
        assert_eq!(callers.len(), 2);
    }

    #[test]
    fn all_callers_for_file_groups_by_symbol() {
        let cp_a = CanonicalPath::new("src/a.ts");
        let cp_b = CanonicalPath::new("src/b.ts");

        let mut nodes = BTreeMap::new();
        nodes.insert(cp_a.clone(), make_node(vec![]));
        nodes.insert(
            cp_b.clone(),
            make_node(vec![make_sym("foo"), make_sym("bar")]),
        );

        let edges = vec![make_edge(
            "src/a.ts",
            "src/b.ts",
            EdgeType::Imports,
            &["foo", "bar"],
        )];
        let idx = SymbolIndex::build(&nodes, &edges);
        let cg = CallGraph::build(&edges, &idx);

        let all = cg.all_callers_for_file(&cp_b);
        assert_eq!(all.len(), 2); // "bar" and "foo" (sorted by BTreeMap key)
    }
}
