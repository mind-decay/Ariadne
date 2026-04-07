use std::collections::BTreeMap;

use crate::conventions::types::{ImportCategory, ImportPattern};
use crate::model::{EdgeType, FileType, ProjectGraph};

/// Analyze import patterns across the codebase.
///
/// Single-pass edge scan. Categorizes each edge by target node's FileType:
/// - Target is Style → ImportCategory::Style
/// - Edge type is Tests → ImportCategory::Test
/// - Otherwise → ImportCategory::Internal
///
/// Counts distinct source files per category. Sorts by file_count descending.
pub fn import_patterns(
    graph: &ProjectGraph,
    scope: Option<&str>,
) -> Vec<ImportPattern> {
    let scope_prefix = scope.map(|s| {
        if s.ends_with('/') { s.to_string() } else { format!("{s}/") }
    });

    // Count source files in scope (denominator for percentage)
    let total_source_files = graph.nodes.iter()
        .filter(|(path, node)| {
            if let Some(ref prefix) = scope_prefix {
                if !path.as_str().starts_with(prefix.as_str()) {
                    return false;
                }
            }
            node.file_type == FileType::Source || node.file_type == FileType::TypeDef
        })
        .count();

    if total_source_files == 0 {
        return Vec::new();
    }

    // Single-pass: collect source files per category using BTreeSet for determinism
    let mut category_files: BTreeMap<ImportCategory, std::collections::BTreeSet<&str>> =
        BTreeMap::new();

    for edge in &graph.edges {
        // Filter: source file must be in scope
        if let Some(ref prefix) = scope_prefix {
            if !edge.from.as_str().starts_with(prefix.as_str()) {
                continue;
            }
        }

        // Source file must be in graph (should always be true)
        let from_node = match graph.nodes.get(&edge.from) {
            Some(n) => n,
            None => continue,
        };

        // Only count edges from source/typedef files
        if from_node.file_type != FileType::Source && from_node.file_type != FileType::TypeDef {
            continue;
        }

        // Classify by edge type and target node type
        let category = if edge.edge_type == EdgeType::Tests {
            ImportCategory::Test
        } else if let Some(target_node) = graph.nodes.get(&edge.to) {
            if target_node.file_type == FileType::Style {
                ImportCategory::Style
            } else {
                ImportCategory::Internal
            }
        } else {
            // Target not in graph (external/unresolved) — skip
            continue;
        };

        category_files
            .entry(category)
            .or_default()
            .insert(edge.from.as_str());
    }

    // Build result sorted by file_count descending
    let mut patterns: Vec<ImportPattern> = category_files
        .into_iter()
        .map(|(category, files)| {
            let file_count = files.len();
            let example_files: Vec<String> = files
                .iter()
                .take(3)
                .map(|s| s.to_string())
                .collect();

            let identifier = match category {
                ImportCategory::Style => "style-imports".to_string(),
                ImportCategory::Internal => "internal-imports".to_string(),
                ImportCategory::Test => "test-imports".to_string(),
            };

            let percentage = (file_count as f64 / total_source_files as f64) * 100.0;

            ImportPattern {
                category,
                identifier,
                file_count,
                percentage,
                trend: None, // Populated by caller with temporal data
                example_files,
            }
        })
        .collect();

    patterns.sort_by(|a, b| b.file_count.cmp(&a.file_count));
    patterns
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{
        ArchLayer, CanonicalPath, ClusterId, ContentHash, Edge, EdgeType,
        FileType, Node, Symbol,
    };
    use std::collections::BTreeMap;

    fn make_node(file_type: FileType) -> Node {
        Node {
            file_type,
            layer: ArchLayer::Unknown,
            fsd_layer: None,
            arch_depth: 0,
            lines: 10,
            hash: ContentHash::new("0000000000000000".to_string()),
            exports: vec![],
            cluster: ClusterId::new("default"),
            symbols: vec![],
        }
    }

    fn make_edge(from: &str, to: &str, edge_type: EdgeType) -> Edge {
        Edge {
            from: CanonicalPath::new(from),
            to: CanonicalPath::new(to),
            edge_type,
            symbols: vec![Symbol::new("default")],
        }
    }

    #[test]
    fn style_imports_identified() {
        let mut nodes = BTreeMap::new();
        nodes.insert(CanonicalPath::new("src/app.ts"), make_node(FileType::Source));
        nodes.insert(CanonicalPath::new("src/styles.css"), make_node(FileType::Style));

        let edges = vec![
            make_edge("src/app.ts", "src/styles.css", EdgeType::Imports),
        ];

        let graph = ProjectGraph { nodes, edges };
        let patterns = import_patterns(&graph, None);

        assert_eq!(patterns.len(), 1);
        assert_eq!(patterns[0].category, ImportCategory::Style);
        assert_eq!(patterns[0].file_count, 1);
        assert_eq!(patterns[0].identifier, "style-imports");
    }

    #[test]
    fn internal_imports_counted() {
        let mut nodes = BTreeMap::new();
        nodes.insert(CanonicalPath::new("src/a.ts"), make_node(FileType::Source));
        nodes.insert(CanonicalPath::new("src/b.ts"), make_node(FileType::Source));
        nodes.insert(CanonicalPath::new("src/c.ts"), make_node(FileType::Source));

        let edges = vec![
            make_edge("src/a.ts", "src/b.ts", EdgeType::Imports),
            make_edge("src/a.ts", "src/c.ts", EdgeType::Imports),
            make_edge("src/b.ts", "src/c.ts", EdgeType::Imports),
        ];

        let graph = ProjectGraph { nodes, edges };
        let patterns = import_patterns(&graph, None);

        let internal = patterns.iter().find(|p| p.category == ImportCategory::Internal).unwrap();
        // a imports b,c; b imports c → 2 distinct source files
        assert_eq!(internal.file_count, 2);
    }

    #[test]
    fn percentage_correct() {
        let mut nodes = BTreeMap::new();
        nodes.insert(CanonicalPath::new("src/a.ts"), make_node(FileType::Source));
        nodes.insert(CanonicalPath::new("src/b.ts"), make_node(FileType::Source));
        nodes.insert(CanonicalPath::new("src/c.ts"), make_node(FileType::Source));
        nodes.insert(CanonicalPath::new("src/d.ts"), make_node(FileType::Source));

        // Only 2 of 4 files import anything
        let edges = vec![
            make_edge("src/a.ts", "src/b.ts", EdgeType::Imports),
            make_edge("src/c.ts", "src/d.ts", EdgeType::Imports),
        ];

        let graph = ProjectGraph { nodes, edges };
        let patterns = import_patterns(&graph, None);

        let internal = patterns.iter().find(|p| p.category == ImportCategory::Internal).unwrap();
        assert!((internal.percentage - 50.0).abs() < 0.01);
    }

    #[test]
    fn sorted_by_frequency_descending() {
        let mut nodes = BTreeMap::new();
        nodes.insert(CanonicalPath::new("src/a.ts"), make_node(FileType::Source));
        nodes.insert(CanonicalPath::new("src/b.ts"), make_node(FileType::Source));
        nodes.insert(CanonicalPath::new("src/c.ts"), make_node(FileType::Source));
        nodes.insert(CanonicalPath::new("src/style.css"), make_node(FileType::Style));
        nodes.insert(CanonicalPath::new("src/types.ts"), make_node(FileType::Source));

        // 3 files import internally, 1 imports style
        let edges = vec![
            make_edge("src/a.ts", "src/types.ts", EdgeType::Imports),
            make_edge("src/b.ts", "src/types.ts", EdgeType::Imports),
            make_edge("src/c.ts", "src/types.ts", EdgeType::Imports),
            make_edge("src/a.ts", "src/style.css", EdgeType::Imports),
        ];

        let graph = ProjectGraph { nodes, edges };
        let patterns = import_patterns(&graph, None);

        assert!(patterns.len() >= 2);
        // Internal (3 files) should come before Style (1 file)
        assert_eq!(patterns[0].category, ImportCategory::Internal);
        assert_eq!(patterns[1].category, ImportCategory::Style);
        assert!(patterns[0].file_count >= patterns[1].file_count);
    }

    #[test]
    fn scope_filter_works() {
        let mut nodes = BTreeMap::new();
        nodes.insert(CanonicalPath::new("src/auth/login.ts"), make_node(FileType::Source));
        nodes.insert(CanonicalPath::new("src/auth/types.ts"), make_node(FileType::Source));
        nodes.insert(CanonicalPath::new("src/utils/format.ts"), make_node(FileType::Source));
        nodes.insert(CanonicalPath::new("src/utils/types.ts"), make_node(FileType::Source));

        let edges = vec![
            make_edge("src/auth/login.ts", "src/auth/types.ts", EdgeType::Imports),
            make_edge("src/utils/format.ts", "src/utils/types.ts", EdgeType::Imports),
        ];

        let graph = ProjectGraph { nodes, edges };
        let patterns = import_patterns(&graph, Some("src/auth"));

        let internal = patterns.iter().find(|p| p.category == ImportCategory::Internal).unwrap();
        // Only 1 file in src/auth/ imports
        assert_eq!(internal.file_count, 1);
    }

    #[test]
    fn example_files_capped_at_3() {
        let mut nodes = BTreeMap::new();
        let mut edges = Vec::new();
        nodes.insert(CanonicalPath::new("src/shared.ts"), make_node(FileType::Source));

        for i in 0..10 {
            let path = format!("src/file{i}.ts");
            nodes.insert(CanonicalPath::new(&path), make_node(FileType::Source));
            edges.push(make_edge(&path, "src/shared.ts", EdgeType::Imports));
        }

        let graph = ProjectGraph { nodes, edges };
        let patterns = import_patterns(&graph, None);

        let internal = patterns.iter().find(|p| p.category == ImportCategory::Internal).unwrap();
        assert!(internal.example_files.len() <= 3);
    }

    #[test]
    fn empty_graph_returns_empty() {
        let graph = ProjectGraph {
            nodes: BTreeMap::new(),
            edges: vec![],
        };
        let patterns = import_patterns(&graph, None);
        assert!(patterns.is_empty());
    }

    #[test]
    fn test_edge_type_classified() {
        let mut nodes = BTreeMap::new();
        nodes.insert(CanonicalPath::new("src/app.ts"), make_node(FileType::Source));
        nodes.insert(CanonicalPath::new("tests/app.test.ts"), make_node(FileType::Test));

        let edges = vec![
            make_edge("tests/app.test.ts", "src/app.ts", EdgeType::Tests),
        ];

        let graph = ProjectGraph { nodes, edges };
        // Tests edge from a Test file — the Test file isn't Source, so won't be counted
        let patterns = import_patterns(&graph, None);
        // No patterns because the from-file is a Test file, not Source
        assert!(patterns.is_empty());
    }
}
