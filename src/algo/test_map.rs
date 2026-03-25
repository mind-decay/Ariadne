use std::collections::BTreeMap;

use crate::algo::AdjacencyIndex;
use crate::model::{CanonicalPath, EdgeType, FileType, ProjectGraph};

/// Confidence level for test detection.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum TestConfidence {
    Low,
    Medium,
    High,
}

impl TestConfidence {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
        }
    }
}

/// A detected test file with confidence and reason.
#[derive(Clone, Debug)]
pub struct TestHit {
    pub path: CanonicalPath,
    pub confidence: TestConfidence,
    pub reason: String,
}

/// Result of test mapping.
#[derive(Clone, Debug)]
pub struct TestMapResult {
    pub tests: Vec<TestHit>,
    pub warnings: Vec<String>,
}

/// Find test files for the given paths using reverse edges, file type, name heuristics,
/// and transitive analysis.
pub fn find_tests_for(
    paths: &[CanonicalPath],
    graph: &ProjectGraph,
    index: &AdjacencyIndex,
) -> TestMapResult {
    let mut warnings = Vec::new();
    // path -> (confidence, reason) — keep highest confidence per test file
    let mut hits: BTreeMap<CanonicalPath, (TestConfidence, String)> = BTreeMap::new();

    for target in paths {
        if !graph.nodes.contains_key(target) {
            warnings.push(format!("Path not in graph: {}", target));
            continue;
        }

        // Strategy 1: Reverse edges with EdgeType::Tests pointing to target
        for edge in &graph.edges {
            if &edge.to == target && edge.edge_type == EdgeType::Tests {
                insert_if_higher(
                    &mut hits,
                    edge.from.clone(),
                    TestConfidence::High,
                    format!("Tests edge to {}", target),
                );
            }
        }

        // Strategy 2: FileType::Test files that import target (any edge type)
        for edge in &graph.edges {
            if &edge.to == target {
                if let Some(node) = graph.nodes.get(&edge.from) {
                    if node.file_type == FileType::Test {
                        insert_if_higher(
                            &mut hits,
                            edge.from.clone(),
                            TestConfidence::High,
                            format!("Test file imports {}", target),
                        );
                    }
                }
            }
        }

        // Strategy 3: Name heuristics
        let basename = strip_extension(target.file_name());
        for (path, node) in &graph.nodes {
            if path == target {
                continue;
            }
            let candidate_name = path.file_name();
            let candidate_base = strip_extension(candidate_name);

            let is_match = candidate_base == format!("{}_test", basename)
                || candidate_base == format!("test_{}", basename)
                || candidate_base == format!("{}.test", basename)
                || candidate_base == format!("{}.spec", basename)
                || (node.file_type == FileType::Test && candidate_base == basename);

            // Also check tests/ directory pattern
            let in_tests_dir = path.as_str().contains("tests/") || path.as_str().contains("test/");
            let tests_dir_match = in_tests_dir && candidate_base == basename;

            if is_match || tests_dir_match {
                insert_if_higher(
                    &mut hits,
                    path.clone(),
                    TestConfidence::Medium,
                    format!("Name heuristic for {}", target),
                );
            }
        }

        // Strategy 4: Transitive — test files importing a direct dep of target
        if let Some(deps) = index.forward.get(target) {
            for dep in deps {
                // Find test files that import this dep
                for edge in &graph.edges {
                    if &edge.to == *dep {
                        if let Some(node) = graph.nodes.get(&edge.from) {
                            if node.file_type == FileType::Test {
                                insert_if_higher(
                                    &mut hits,
                                    edge.from.clone(),
                                    TestConfidence::Low,
                                    format!("Transitive: tests dep {} of {}", dep, target),
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    let tests: Vec<TestHit> = hits
        .into_iter()
        .map(|(path, (confidence, reason))| TestHit {
            path,
            confidence,
            reason,
        })
        .collect();

    TestMapResult { tests, warnings }
}

/// Insert into hits map, keeping the highest confidence for each path.
fn insert_if_higher(
    hits: &mut BTreeMap<CanonicalPath, (TestConfidence, String)>,
    path: CanonicalPath,
    confidence: TestConfidence,
    reason: String,
) {
    let entry = hits.entry(path);
    match entry {
        std::collections::btree_map::Entry::Vacant(e) => {
            e.insert((confidence, reason));
        }
        std::collections::btree_map::Entry::Occupied(mut e) => {
            if confidence > e.get().0 {
                e.insert((confidence, reason));
            }
        }
    }
}

/// Strip extension from a filename.
fn strip_extension(name: &str) -> String {
    match name.rfind('.') {
        Some(i) => name[..i].to_string(),
        None => name.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::algo::is_architectural;
    use crate::model::*;
    use std::collections::BTreeMap;

    fn make_graph_with_types(
        nodes: &[(&str, FileType)],
        edges: &[(&str, &str, EdgeType)],
    ) -> ProjectGraph {
        let mut node_map = BTreeMap::new();
        for (name, ft) in nodes {
            node_map.insert(
                CanonicalPath::new(*name),
                Node {
                    file_type: *ft,
                    layer: ArchLayer::Unknown,
                    fsd_layer: None,
                    arch_depth: 0,
                    lines: 10,
                    hash: ContentHash::new("0".to_string()),
                    exports: vec![],
                    cluster: ClusterId::new("default"),
                    symbols: Vec::new(),
                },
            );
        }
        let edge_list = edges
            .iter()
            .map(|(from, to, et)| Edge {
                from: CanonicalPath::new(*from),
                to: CanonicalPath::new(*to),
                edge_type: *et,
                symbols: vec![],
            })
            .collect();
        ProjectGraph {
            nodes: node_map,
            edges: edge_list,
        }
    }

    #[test]
    fn direct_test_edge_high_confidence() {
        let graph = make_graph_with_types(
            &[
                ("src/lib.rs", FileType::Source),
                ("tests/lib_test.rs", FileType::Test),
            ],
            &[("tests/lib_test.rs", "src/lib.rs", EdgeType::Tests)],
        );
        let index = AdjacencyIndex::build(&graph.edges, |_| true);
        let result = find_tests_for(&[CanonicalPath::new("src/lib.rs")], &graph, &index);
        assert_eq!(result.tests.len(), 1);
        assert_eq!(result.tests[0].confidence, TestConfidence::High);
        assert_eq!(result.tests[0].path.as_str(), "tests/lib_test.rs");
    }

    #[test]
    fn name_heuristic_medium_confidence() {
        let graph = make_graph_with_types(
            &[
                ("src/foo.rs", FileType::Source),
                ("tests/foo_test.rs", FileType::Test),
            ],
            &[], // No edges — pure name heuristic
        );
        let index = AdjacencyIndex::build(&graph.edges, is_architectural);
        let result = find_tests_for(&[CanonicalPath::new("src/foo.rs")], &graph, &index);
        assert_eq!(result.tests.len(), 1);
        assert_eq!(result.tests[0].confidence, TestConfidence::Medium);
    }

    #[test]
    fn transitive_low_confidence() {
        // src/a.rs -> src/b.rs, tests/b_test.rs imports src/b.rs
        let graph = make_graph_with_types(
            &[
                ("src/a.rs", FileType::Source),
                ("src/b.rs", FileType::Source),
                ("tests/b_test.rs", FileType::Test),
            ],
            &[
                ("src/a.rs", "src/b.rs", EdgeType::Imports),
                ("tests/b_test.rs", "src/b.rs", EdgeType::Imports),
            ],
        );
        let index = AdjacencyIndex::build(&graph.edges, is_architectural);
        let result = find_tests_for(&[CanonicalPath::new("src/a.rs")], &graph, &index);
        assert_eq!(result.tests.len(), 1);
        assert_eq!(result.tests[0].confidence, TestConfidence::Low);
    }

    #[test]
    fn empty_input_empty_result() {
        let graph = make_graph_with_types(
            &[("src/a.rs", FileType::Source)],
            &[],
        );
        let index = AdjacencyIndex::build(&graph.edges, is_architectural);
        let result = find_tests_for(&[], &graph, &index);
        assert!(result.tests.is_empty());
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn nonexistent_path_produces_warning() {
        let graph = make_graph_with_types(
            &[("src/a.rs", FileType::Source)],
            &[],
        );
        let index = AdjacencyIndex::build(&graph.edges, is_architectural);
        let result = find_tests_for(&[CanonicalPath::new("nonexistent.rs")], &graph, &index);
        assert!(result.tests.is_empty());
        assert_eq!(result.warnings.len(), 1);
        assert!(result.warnings[0].contains("nonexistent.rs"));
    }

    #[test]
    fn dedup_keeps_highest_confidence() {
        // tests/lib_test.rs has both a Tests edge AND imports the target
        let graph = make_graph_with_types(
            &[
                ("src/lib.rs", FileType::Source),
                ("tests/lib_test.rs", FileType::Test),
            ],
            &[
                ("tests/lib_test.rs", "src/lib.rs", EdgeType::Tests),
                ("tests/lib_test.rs", "src/lib.rs", EdgeType::Imports),
            ],
        );
        let index = AdjacencyIndex::build(&graph.edges, |_| true);
        let result = find_tests_for(&[CanonicalPath::new("src/lib.rs")], &graph, &index);
        // Should be deduplicated to one entry with High confidence
        assert_eq!(result.tests.len(), 1);
        assert_eq!(result.tests[0].confidence, TestConfidence::High);
    }
}
