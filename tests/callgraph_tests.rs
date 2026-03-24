//! Integration tests for Phase 4c — Call Graph (D-079).
//!
//! Tests callers_of, callees_of, symbol blast radius truncation,
//! and circular import handling via CallGraph + SymbolIndex.

use std::collections::BTreeMap;

use ariadne_graph::algo::callgraph::{CallEdgeKind, CallGraph};
use ariadne_graph::model::edge::{Edge, EdgeType};
use ariadne_graph::model::node::{ArchLayer, FileType, Node};
use ariadne_graph::model::symbol::{LineSpan, SymbolDef, Visibility};
use ariadne_graph::model::symbol_index::SymbolIndex;
use ariadne_graph::model::types::{CanonicalPath, ClusterId, ContentHash, Symbol};

// ──────────────────────────────────────────────────────────────────────────────
// Helpers
// ──────────────────────────────────────────────────────────────────────────────

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
        kind: ariadne_graph::model::SymbolKind::Function,
        visibility: Visibility::Public,
        span: LineSpan { start: 1, end: 5 },
        signature: None,
        parent: None,
    }
}

fn make_edge(from: &str, to: &str, symbols: &[&str]) -> Edge {
    Edge {
        from: CanonicalPath::new(from),
        to: CanonicalPath::new(to),
        edge_type: EdgeType::Imports,
        symbols: symbols
            .iter()
            .map(|s| Symbol::new(s.to_string()))
            .collect(),
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// T1: ariadne_callers — verify callers_of returns correct edges
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn callers_of_returns_correct_cross_file_edges() {
    // Setup: file C defines "shared", files A and B both import "shared" from C
    let cp_a = CanonicalPath::new("src/a.ts");
    let cp_b = CanonicalPath::new("src/b.ts");
    let cp_c = CanonicalPath::new("src/c.ts");

    let mut nodes = BTreeMap::new();
    nodes.insert(cp_a.clone(), make_node(vec![make_sym("helperA")]));
    nodes.insert(cp_b.clone(), make_node(vec![make_sym("helperB")]));
    nodes.insert(
        cp_c.clone(),
        make_node(vec![make_sym("shared"), make_sym("private_fn")]),
    );

    let edges = vec![
        make_edge("src/a.ts", "src/c.ts", &["shared"]),
        make_edge("src/b.ts", "src/c.ts", &["shared"]),
    ];

    let idx = SymbolIndex::build(&nodes, &edges);
    let cg = CallGraph::build(&edges, &idx);

    // "shared" in C should have 2 callers: A and B
    let callers = cg.callers_of(&cp_c, "shared");
    assert_eq!(callers.len(), 2, "shared should have 2 callers");

    let caller_files: Vec<&str> = callers.iter().map(|c| c.file.as_str()).collect();
    assert!(caller_files.contains(&"src/a.ts"));
    assert!(caller_files.contains(&"src/b.ts"));

    // All edges should be Import kind
    for ce in callers {
        assert_eq!(ce.edge_kind, CallEdgeKind::Import);
    }

    // "private_fn" in C should have no callers (nobody imports it)
    assert!(cg.callers_of(&cp_c, "private_fn").is_empty());
}

// ──────────────────────────────────────────────────────────────────────────────
// T2: ariadne_callees — verify callees_of returns correct edges
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn callees_of_returns_correct_cross_file_edges() {
    // Setup: file A imports "foo" and "bar" from file B, "baz" from file C
    let cp_a = CanonicalPath::new("src/a.ts");
    let cp_b = CanonicalPath::new("src/b.ts");
    let cp_c = CanonicalPath::new("src/c.ts");

    let mut nodes = BTreeMap::new();
    nodes.insert(cp_a.clone(), make_node(vec![]));
    nodes.insert(
        cp_b.clone(),
        make_node(vec![make_sym("foo"), make_sym("bar")]),
    );
    nodes.insert(cp_c.clone(), make_node(vec![make_sym("baz")]));

    let edges = vec![
        make_edge("src/a.ts", "src/b.ts", &["foo", "bar"]),
        make_edge("src/a.ts", "src/c.ts", &["baz"]),
    ];

    let idx = SymbolIndex::build(&nodes, &edges);
    let cg = CallGraph::build(&edges, &idx);

    // A's usage of "foo" should point to B
    let callees_foo = cg.callees_of(&cp_a, "foo");
    assert_eq!(callees_foo.len(), 1);
    assert_eq!(callees_foo[0].file, cp_b);

    // A's usage of "bar" should point to B
    let callees_bar = cg.callees_of(&cp_a, "bar");
    assert_eq!(callees_bar.len(), 1);
    assert_eq!(callees_bar[0].file, cp_b);

    // A's usage of "baz" should point to C
    let callees_baz = cg.callees_of(&cp_a, "baz");
    assert_eq!(callees_baz.len(), 1);
    assert_eq!(callees_baz[0].file, cp_c);

    // Nonexistent symbol returns empty
    assert!(cg.callees_of(&cp_a, "nonexistent").is_empty());
}

// ──────────────────────────────────────────────────────────────────────────────
// T3: W021/truncation — symbol blast radius BFS truncation detection
// ──────────────────────────────────────────────────────────────────────────────

/// Simulates the symbol_blast_radius BFS logic from tools.rs to verify
/// that truncation is correctly detected when depth limit cuts off traversal.
#[test]
fn symbol_blast_radius_truncation_detected() {
    // Build a chain: A imports "sym" from B, B imports "sym" from C, C imports "sym" from D
    // With depth=1, BFS from A should find B but not C or D → truncated=true
    let cp_a = CanonicalPath::new("src/a.ts");
    let cp_b = CanonicalPath::new("src/b.ts");
    let cp_c = CanonicalPath::new("src/c.ts");
    let cp_d = CanonicalPath::new("src/d.ts");

    let mut nodes = BTreeMap::new();
    nodes.insert(cp_a.clone(), make_node(vec![make_sym("sym")]));
    nodes.insert(cp_b.clone(), make_node(vec![make_sym("sym")]));
    nodes.insert(cp_c.clone(), make_node(vec![make_sym("sym")]));
    nodes.insert(cp_d.clone(), make_node(vec![make_sym("sym")]));

    let edges = vec![
        // B imports sym from A (B is a caller of A's sym)
        make_edge("src/b.ts", "src/a.ts", &["sym"]),
        // C imports sym from B (C is a caller of B's sym)
        make_edge("src/c.ts", "src/b.ts", &["sym"]),
        // D imports sym from C (D is a caller of C's sym)
        make_edge("src/d.ts", "src/c.ts", &["sym"]),
    ];

    let idx = SymbolIndex::build(&nodes, &edges);
    let cg = CallGraph::build(&edges, &idx);

    // Verify the chain exists
    assert_eq!(cg.callers_of(&cp_a, "sym").len(), 1); // B calls A's sym
    assert_eq!(cg.callers_of(&cp_b, "sym").len(), 1); // C calls B's sym
    assert_eq!(cg.callers_of(&cp_c, "sym").len(), 1); // D calls C's sym
    assert!(cg.callers_of(&cp_d, "sym").is_empty()); // nobody calls D's sym

    // Simulate BFS with depth=1 (same logic as tools.rs symbol_blast_radius)
    let max_depth: u32 = 1;
    let mut visited: std::collections::BTreeSet<(String, String)> =
        std::collections::BTreeSet::new();
    let mut truncated = false;

    let start_key = (cp_a.as_str().to_string(), "sym".to_string());
    visited.insert(start_key);

    let mut frontier: std::collections::VecDeque<(CanonicalPath, String, u32)> =
        std::collections::VecDeque::new();
    frontier.push_back((cp_a.clone(), "sym".to_string(), 0));

    let reverse_index: BTreeMap<CanonicalPath, Vec<&Edge>> = {
        let mut ri: BTreeMap<CanonicalPath, Vec<&Edge>> = BTreeMap::new();
        for e in &edges {
            ri.entry(e.to.clone()).or_default().push(e);
        }
        ri
    };

    while let Some((file, sym_name, distance)) = frontier.pop_front() {
        if distance >= max_depth {
            // Check for truncation: are there unvisited neighbors?
            if let Some(rev_edges) = reverse_index.get(&file) {
                for edge in rev_edges {
                    if edge.symbols.iter().any(|s| s.as_str() == sym_name) {
                        let key = (edge.from.as_str().to_string(), sym_name.clone());
                        if !visited.contains(&key) {
                            truncated = true;
                        }
                    }
                }
            }
            continue;
        }

        // Expand: check reverse edges (callers)
        if let Some(rev_edges) = reverse_index.get(&file) {
            for edge in rev_edges {
                if edge.symbols.iter().any(|s| s.as_str() == sym_name) {
                    let key = (edge.from.as_str().to_string(), sym_name.clone());
                    if !visited.contains(&key) {
                        visited.insert(key);
                        frontier.push_back((edge.from.clone(), sym_name.clone(), distance + 1));
                    }
                }
            }
        }
    }

    assert!(
        truncated,
        "BFS with depth=1 on a 4-node chain should detect truncation"
    );
}

#[test]
fn symbol_blast_radius_no_truncation_when_fully_explored() {
    // Build a simple A<-B chain. With depth=5, BFS should NOT truncate.
    let cp_a = CanonicalPath::new("src/a.ts");
    let cp_b = CanonicalPath::new("src/b.ts");

    let mut nodes = BTreeMap::new();
    nodes.insert(cp_a.clone(), make_node(vec![make_sym("sym")]));
    nodes.insert(cp_b.clone(), make_node(vec![make_sym("sym")]));

    let edges = vec![make_edge("src/b.ts", "src/a.ts", &["sym"])];

    let idx = SymbolIndex::build(&nodes, &edges);
    let _cg = CallGraph::build(&edges, &idx);

    // BFS with generous depth
    let max_depth: u32 = 5;
    let mut visited: std::collections::BTreeSet<(String, String)> =
        std::collections::BTreeSet::new();
    let mut truncated = false;

    let start_key = (cp_a.as_str().to_string(), "sym".to_string());
    visited.insert(start_key);

    let mut frontier: std::collections::VecDeque<(CanonicalPath, String, u32)> =
        std::collections::VecDeque::new();
    frontier.push_back((cp_a.clone(), "sym".to_string(), 0));

    let reverse_index: BTreeMap<CanonicalPath, Vec<&Edge>> = {
        let mut ri: BTreeMap<CanonicalPath, Vec<&Edge>> = BTreeMap::new();
        for e in &edges {
            ri.entry(e.to.clone()).or_default().push(e);
        }
        ri
    };

    while let Some((file, sym_name, distance)) = frontier.pop_front() {
        if distance >= max_depth {
            if let Some(rev_edges) = reverse_index.get(&file) {
                for edge in rev_edges {
                    if edge.symbols.iter().any(|s| s.as_str() == sym_name) {
                        let key = (edge.from.as_str().to_string(), sym_name.clone());
                        if !visited.contains(&key) {
                            truncated = true;
                        }
                    }
                }
            }
            continue;
        }

        if let Some(rev_edges) = reverse_index.get(&file) {
            for edge in rev_edges {
                if edge.symbols.iter().any(|s| s.as_str() == sym_name) {
                    let key = (edge.from.as_str().to_string(), sym_name.clone());
                    if !visited.contains(&key) {
                        visited.insert(key);
                        frontier.push_back((edge.from.clone(), sym_name.clone(), distance + 1));
                    }
                }
            }
        }
    }

    assert!(
        !truncated,
        "BFS with depth=5 on a 2-node chain should NOT truncate"
    );
}

// ──────────────────────────────────────────────────────────────────────────────
// T4: Circular chain — BFS terminates correctly with visited set
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn circular_imports_bfs_terminates() {
    // Build a cycle: A->B->C->A (each imports "sym" from the next)
    let cp_a = CanonicalPath::new("src/a.ts");
    let cp_b = CanonicalPath::new("src/b.ts");
    let cp_c = CanonicalPath::new("src/c.ts");

    let mut nodes = BTreeMap::new();
    nodes.insert(cp_a.clone(), make_node(vec![make_sym("sym")]));
    nodes.insert(cp_b.clone(), make_node(vec![make_sym("sym")]));
    nodes.insert(cp_c.clone(), make_node(vec![make_sym("sym")]));

    let edges = vec![
        make_edge("src/a.ts", "src/b.ts", &["sym"]),
        make_edge("src/b.ts", "src/c.ts", &["sym"]),
        make_edge("src/c.ts", "src/a.ts", &["sym"]),
    ];

    let idx = SymbolIndex::build(&nodes, &edges);
    let cg = CallGraph::build(&edges, &idx);

    // Verify cycle exists: A's sym is called by C, B's sym by A, C's sym by B
    assert_eq!(cg.callers_of(&cp_a, "sym").len(), 1); // C calls A's sym
    assert_eq!(cg.callers_of(&cp_b, "sym").len(), 1); // A calls B's sym
    assert_eq!(cg.callers_of(&cp_c, "sym").len(), 1); // B calls C's sym

    // BFS should terminate (not infinite loop) even with a cycle
    let max_depth: u32 = 10;
    let mut visited: std::collections::BTreeSet<(String, String)> =
        std::collections::BTreeSet::new();
    let mut affected_count: u32 = 0;

    let start_key = (cp_a.as_str().to_string(), "sym".to_string());
    visited.insert(start_key);

    let mut frontier: std::collections::VecDeque<(CanonicalPath, String, u32)> =
        std::collections::VecDeque::new();
    frontier.push_back((cp_a.clone(), "sym".to_string(), 0));

    let reverse_index: BTreeMap<CanonicalPath, Vec<&Edge>> = {
        let mut ri: BTreeMap<CanonicalPath, Vec<&Edge>> = BTreeMap::new();
        for e in &edges {
            ri.entry(e.to.clone()).or_default().push(e);
        }
        ri
    };

    while let Some((file, sym_name, distance)) = frontier.pop_front() {
        if distance > 0 {
            affected_count += 1;
        }
        if distance >= max_depth {
            continue;
        }

        if let Some(rev_edges) = reverse_index.get(&file) {
            for edge in rev_edges {
                if edge.symbols.iter().any(|s| s.as_str() == sym_name) {
                    let key = (edge.from.as_str().to_string(), sym_name.clone());
                    if !visited.contains(&key) {
                        visited.insert(key);
                        frontier.push_back((edge.from.clone(), sym_name.clone(), distance + 1));
                    }
                }
            }
        }
    }

    // Should have visited exactly 2 additional nodes (B and C) despite the cycle
    assert_eq!(
        affected_count, 2,
        "BFS on 3-node cycle from A should find exactly 2 affected nodes (B, C)"
    );

    // Visited set should contain all 3 nodes
    assert_eq!(visited.len(), 3, "all 3 nodes should be visited");
}
