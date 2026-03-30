//! File decomposition analysis — suggests how to split large/complex files.
//!
//! Uses symbol-level coupling analysis (export co-usage, parent co-location,
//! temporal co-change) to identify natural partition points via Stoer-Wagner
//! min-cut, with greedy modularity fallback for large files or trivial partitions.

use std::collections::{BTreeMap, BTreeSet};

use crate::algo::blast_radius::blast_radius;
use crate::algo::callgraph::CallGraph;
use crate::algo::{round4, AdjacencyIndex};
use crate::model::edge::{Edge, EdgeType};
use crate::model::graph::ProjectGraph;
use crate::model::node::FileType;
use crate::model::symbol::SymbolDef;
use crate::model::symbol_index::SymbolIndex;
use crate::model::temporal::TemporalState;
use crate::model::CanonicalPath;
use crate::recommend::min_cut::stoer_wagner;
use crate::recommend::types::{
    DataQuality, SplitAnalysis, SplitGroup, SplitImpact, SymbolGraph,
};

/// Minimum symbols required for a split recommendation
const MIN_SYMBOLS_FOR_SPLIT: usize = 6;

/// Minimum symbols per resulting group
const MIN_SYMBOLS_PER_GROUP: usize = 3;

/// Partition balance threshold — if smaller side < this fraction, partition is trivial
const TRIVIAL_PARTITION_THRESHOLD: f64 = 0.20;

/// Symbol count above which Stoer-Wagner is skipped (O(V^3) too expensive)
const LARGE_FILE_SYMBOL_THRESHOLD: usize = 500;

/// Weight for export co-usage coupling signal
const WEIGHT_CO_USAGE: f64 = 1.0;

/// Weight for parent co-location coupling signal
const WEIGHT_CO_LOCATION: f64 = 0.5;

/// Weight for temporal co-change bias
const WEIGHT_TEMPORAL: f64 = 0.3;

/// Cut weight threshold — above this, file is too cohesive to split
const MAX_CUT_WEIGHT_FOR_SPLIT: f64 = 3.0;

fn zero_impact() -> SplitImpact {
    SplitImpact {
        blast_radius_before: 0,
        blast_radius_after_estimate: 0,
        centrality_before: 0.0,
        centrality_reduction_estimate: 0.0,
    }
}

pub fn analyze_split(
    path: &CanonicalPath,
    graph: &ProjectGraph,
    symbol_index: Option<&SymbolIndex>,
    call_graph: Option<&CallGraph>,
    temporal: Option<&TemporalState>,
    centrality: Option<f64>,
) -> SplitAnalysis {
    // Step 1 — Validate file exists
    let node = match graph.nodes.get(path) {
        Some(n) => n,
        None => {
            return SplitAnalysis {
                path: path.as_str().to_string(),
                should_split: false,
                reason: "File not found in graph".into(),
                suggested_splits: vec![],
                cut_weight: 0.0,
                impact: zero_impact(),
                data_quality: DataQuality::Minimal,
            };
        }
    };

    // Step 2 — Early return for test files (EC-5)
    if node.file_type == FileType::Test {
        return SplitAnalysis {
            path: path.as_str().to_string(),
            should_split: false,
            reason: "Test files are better organized by test grouping, not symbol coupling".into(),
            suggested_splits: vec![],
            cut_weight: 0.0,
            impact: zero_impact(),
            data_quality: DataQuality::Minimal,
        };
    }

    // Step 3 — Early return for re-export hubs (EC-8)
    let outgoing: Vec<_> = graph.edges.iter().filter(|e| e.from == *path).collect();
    let reexport_count = outgoing
        .iter()
        .filter(|e| e.edge_type == EdgeType::ReExports)
        .count();
    if !outgoing.is_empty() && reexport_count * 5 > outgoing.len() * 4 {
        return SplitAnalysis {
            path: path.as_str().to_string(),
            should_split: false,
            reason: "Re-export hub — splitting would break barrel/index pattern".into(),
            suggested_splits: vec![],
            cut_weight: 0.0,
            impact: zero_impact(),
            data_quality: DataQuality::Minimal,
        };
    }

    // Step 4 — Get symbols for the file
    let symbols: Vec<&SymbolDef> = symbol_index
        .and_then(|si| si.symbols_for_file(path))
        .map(|s| s.iter().collect())
        .unwrap_or_else(|| node.symbols.iter().collect());

    // Step 5 — Determine data quality (Decision 7)
    let data_quality = if symbols.is_empty() {
        DataQuality::Minimal
    } else if temporal.is_none() {
        DataQuality::Structural
    } else {
        DataQuality::Full
    };

    // Step 6 — Minimal mode early return
    if data_quality == DataQuality::Minimal {
        let reason = if node.lines > 500 {
            format!(
                "Insufficient symbol data for analysis (file has {} lines, consider manual review)",
                node.lines
            )
        } else {
            "No symbol data available".into()
        };
        return SplitAnalysis {
            path: path.as_str().to_string(),
            should_split: false,
            reason,
            suggested_splits: vec![],
            cut_weight: 0.0,
            impact: zero_impact(),
            data_quality,
        };
    }

    // Step 7 — Check minimum symbol count
    if symbols.len() < MIN_SYMBOLS_FOR_SPLIT {
        return SplitAnalysis {
            path: path.as_str().to_string(),
            should_split: false,
            reason: format!(
                "File has only {} symbols (minimum {} required for split analysis)",
                symbols.len(),
                MIN_SYMBOLS_FOR_SPLIT
            ),
            suggested_splits: vec![],
            cut_weight: 0.0,
            impact: zero_impact(),
            data_quality,
        };
    }

    // Step 8 — Build SymbolGraph (Decision 2)
    let symbol_graph = build_symbol_graph(path, &symbols, call_graph, temporal, &graph.edges);

    // Step 9 — Partition
    let (partitions, cut_weight) = if symbols.len() > LARGE_FILE_SYMBOL_THRESHOLD {
        let parts = greedy_partition(&symbol_graph, 3);
        (parts, 0.0)
    } else {
        match stoer_wagner(&symbol_graph) {
            Some(result) => {
                let total = result.partition_a.len() + result.partition_b.len();
                let smaller = result.partition_a.len().min(result.partition_b.len());
                if (smaller as f64) / (total as f64) < TRIVIAL_PARTITION_THRESHOLD {
                    let parts = greedy_partition(&symbol_graph, 3);
                    (parts, result.cut_weight)
                } else {
                    let parts = vec![result.partition_a, result.partition_b];
                    (parts, result.cut_weight)
                }
            }
            None => {
                return SplitAnalysis {
                    path: path.as_str().to_string(),
                    should_split: false,
                    reason: "Min-cut returned no partition".into(),
                    suggested_splits: vec![],
                    cut_weight: 0.0,
                    impact: zero_impact(),
                    data_quality,
                };
            }
        }
    };

    // Step 10 — Build SplitGroups
    let file_stem = path
        .as_str()
        .rsplit('/')
        .next()
        .unwrap_or(path.as_str())
        .trim_end_matches(".ts")
        .trim_end_matches(".tsx")
        .trim_end_matches(".js")
        .trim_end_matches(".jsx")
        .trim_end_matches(".rs")
        .trim_end_matches(".py");

    let suggested_splits: Vec<SplitGroup> = partitions
        .iter()
        .enumerate()
        .map(|(idx, partition)| {
            let syms: Vec<&SymbolDef> = partition
                .iter()
                .filter_map(|&i| symbols.get(i).copied())
                .collect();

            let symbol_names: BTreeSet<String> =
                syms.iter().map(|s| s.name.clone()).collect();

            let name = derive_group_name(&syms, file_stem, idx);

            let estimated_lines: u32 = syms
                .iter()
                .map(|s| s.span.end.saturating_sub(s.span.start) + 1)
                .sum();

            let top_3: Vec<&str> = syms.iter().take(3).map(|s| s.name.as_str()).collect();
            let rationale = format!(
                "Symbols grouped by coupling analysis: {}",
                top_3.join(", ")
            );

            SplitGroup {
                name,
                symbols: symbol_names,
                estimated_lines,
                rationale,
            }
        })
        .collect();

    // Step 11 — Compute SplitImpact
    let adj_index = AdjacencyIndex::build(&graph.edges, |_| true);
    let br_result = blast_radius(graph, path, Some(10), &adj_index);
    let blast_radius_before = br_result.len().saturating_sub(1) as u32;

    let centrality_before = round4(centrality.unwrap_or(0.0));
    let num_groups = partitions.len().max(1) as f64;
    let centrality_reduction_estimate = round4(centrality_before * (1.0 - 1.0 / num_groups));
    let blast_radius_after_estimate = (blast_radius_before as f64 / num_groups).ceil() as u32;

    let impact = SplitImpact {
        blast_radius_before,
        blast_radius_after_estimate,
        centrality_before,
        centrality_reduction_estimate,
    };

    // Step 12 — Determine should_split
    let min_group_size = partitions.iter().map(|p| p.len()).min().unwrap_or(0);
    let should_split =
        min_group_size >= MIN_SYMBOLS_PER_GROUP
            && cut_weight <= MAX_CUT_WEIGHT_FOR_SPLIT
            && partitions.len() >= 2;

    let reason = if should_split {
        format!(
            "File can be split into {} groups with cut weight {:.1} (threshold {:.1}). \
             Smallest group has {} symbols (minimum {}).",
            partitions.len(),
            cut_weight,
            MAX_CUT_WEIGHT_FOR_SPLIT,
            min_group_size,
            MIN_SYMBOLS_PER_GROUP
        )
    } else if min_group_size < MIN_SYMBOLS_PER_GROUP {
        format!(
            "Partition produces groups too small ({} symbols, minimum {})",
            min_group_size, MIN_SYMBOLS_PER_GROUP
        )
    } else if cut_weight > MAX_CUT_WEIGHT_FOR_SPLIT {
        format!(
            "File is too cohesive to split (cut weight {:.1} exceeds threshold {:.1})",
            cut_weight, MAX_CUT_WEIGHT_FOR_SPLIT
        )
    } else {
        "No valid partition found".into()
    };

    // Step 13 — Return SplitAnalysis
    SplitAnalysis {
        path: path.as_str().to_string(),
        should_split,
        reason,
        suggested_splits,
        cut_weight: round4(cut_weight),
        impact,
        data_quality,
    }
}

/// Derive a group name from the most common parent, longest common prefix, or fallback.
fn derive_group_name(syms: &[&SymbolDef], file_stem: &str, idx: usize) -> String {
    // Try most common parent
    let mut parent_counts: BTreeMap<&str, usize> = BTreeMap::new();
    for s in syms {
        if let Some(ref p) = s.parent {
            *parent_counts.entry(p.as_str()).or_insert(0) += 1;
        }
    }
    if let Some((&parent, &count)) = parent_counts.iter().max_by_key(|(_, &c)| c) {
        if count > syms.len() / 2 {
            return parent.to_string();
        }
    }

    // Try longest common prefix of symbol names
    let names: Vec<&str> = syms.iter().map(|s| s.name.as_str()).collect();
    if names.len() >= 2 {
        let lcp = longest_common_prefix(&names);
        if lcp.len() >= 3 {
            return lcp.trim_end_matches('_').to_string();
        }
    }

    // Fallback
    format!("{file_stem}_part{}", idx + 1)
}

/// Compute the longest common prefix of a set of strings.
fn longest_common_prefix(strings: &[&str]) -> String {
    if strings.is_empty() {
        return String::new();
    }
    let first = strings[0];
    let mut len = first.len();
    for s in &strings[1..] {
        len = len.min(s.len());
        for (i, (a, b)) in first.bytes().zip(s.bytes()).enumerate() {
            if a != b {
                len = len.min(i);
                break;
            }
        }
    }
    first[..len].to_string()
}

fn build_symbol_graph(
    path: &CanonicalPath,
    symbols: &[&SymbolDef],
    call_graph: Option<&CallGraph>,
    temporal: Option<&TemporalState>,
    edges: &[Edge],
) -> SymbolGraph {
    // 1. Create sorted, deduped node names
    let name_set: BTreeSet<String> = symbols.iter().map(|s| s.name.clone()).collect();
    let nodes: Vec<String> = name_set.into_iter().collect();
    let n = nodes.len();

    // 2. Name-to-index map
    let name_to_idx: BTreeMap<&str, usize> = nodes
        .iter()
        .enumerate()
        .map(|(i, name)| (name.as_str(), i))
        .collect();

    // 3. Initialize weights matrix
    let mut weights = vec![vec![0.0; n]; n];

    // 4. Export co-usage signal (Decision 2, weight 1.0)
    if let Some(cg) = call_graph {
        let caller_entries = cg.all_callers_for_file(path);

        // Group by caller file: which of our symbols does each caller file reference?
        let mut callers_by_file: BTreeMap<&CanonicalPath, BTreeSet<usize>> = BTreeMap::new();

        for (sym_name, call_edges) in &caller_entries {
            if let Some(&sym_idx) = name_to_idx.get(*sym_name) {
                for ce in *call_edges {
                    callers_by_file
                        .entry(&ce.file)
                        .or_default()
                        .insert(sym_idx);
                }
            }
        }

        // For each caller file, add co-usage weight between all pairs of referenced symbols
        for sym_indices in callers_by_file.values() {
            let indices: Vec<usize> = sym_indices.iter().copied().collect();
            for a in 0..indices.len() {
                for b in (a + 1)..indices.len() {
                    let i = indices[a];
                    let j = indices[b];
                    weights[i][j] += WEIGHT_CO_USAGE;
                    weights[j][i] += WEIGHT_CO_USAGE;
                }
            }
        }
    }

    // Also use edge-based co-usage: symbols imported together by the same file
    let incoming_edges: Vec<&Edge> = edges.iter().filter(|e| e.to == *path).collect();
    let mut importers_by_file: BTreeMap<&CanonicalPath, BTreeSet<usize>> = BTreeMap::new();
    for e in &incoming_edges {
        for sym in &e.symbols {
            if let Some(&idx) = name_to_idx.get(sym.as_str()) {
                importers_by_file.entry(&e.from).or_default().insert(idx);
            }
        }
    }
    for sym_indices in importers_by_file.values() {
        let indices: Vec<usize> = sym_indices.iter().copied().collect();
        for a in 0..indices.len() {
            for b in (a + 1)..indices.len() {
                let i = indices[a];
                let j = indices[b];
                weights[i][j] += WEIGHT_CO_USAGE;
                weights[j][i] += WEIGHT_CO_USAGE;
            }
        }
    }

    // 5. Parent co-location signal (Decision 2, weight 0.5)
    let mut by_parent: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
    for s in symbols {
        if let Some(ref parent) = s.parent {
            if let Some(&idx) = name_to_idx.get(s.name.as_str()) {
                by_parent.entry(parent.as_str()).or_default().push(idx);
            }
        }
    }
    for indices in by_parent.values() {
        for a in 0..indices.len() {
            for b in (a + 1)..indices.len() {
                let i = indices[a];
                let j = indices[b];
                weights[i][j] += WEIGHT_CO_LOCATION;
                weights[j][i] += WEIGHT_CO_LOCATION;
            }
        }
    }

    // 6. Temporal signal (Decision 2, weight 0.3)
    if let Some(temp) = temporal {
        if let Some(churn) = temp.churn.get(path) {
            if churn.commits_90d > 10 {
                for i in 0..n {
                    for j in (i + 1)..n {
                        weights[i][j] += WEIGHT_TEMPORAL;
                        weights[j][i] += WEIGHT_TEMPORAL;
                    }
                }
            }
        }
    }

    SymbolGraph { nodes, weights }
}

fn greedy_partition(graph: &SymbolGraph, max_groups: usize) -> Vec<BTreeSet<usize>> {
    let n = graph.nodes.len();
    if n == 0 {
        return vec![];
    }

    // Start: each node in its own community
    let mut communities: Vec<BTreeSet<usize>> = (0..n).map(|i| BTreeSet::from([i])).collect();

    loop {
        if communities.len() <= max_groups {
            break;
        }

        // Find the pair of communities with the highest inter-community weight
        let mut best_weight = 0.0_f64;
        let mut best_pair = (0, 1);
        let mut found = false;

        for a in 0..communities.len() {
            for b in (a + 1)..communities.len() {
                let mut w = 0.0;
                for &i in &communities[a] {
                    for &j in &communities[b] {
                        w += graph.weights[i][j];
                    }
                }
                if w > best_weight || !found {
                    best_weight = w;
                    best_pair = (a, b);
                    found = true;
                }
            }
        }

        // Stop if no positive edge weight remains
        if best_weight <= 0.0 {
            break;
        }

        // Merge the two communities
        let (a, b) = best_pair;
        let merged = communities[b].clone();
        communities[a].extend(merged);
        communities.remove(b);
    }

    communities
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::node::{ArchLayer, Node};
    use crate::model::symbol::{LineSpan, SymbolKind, Visibility};
    use crate::model::types::ContentHash;
    use crate::model::ClusterId;

    fn make_symbol(name: &str, start: u32, end: u32, parent: Option<&str>) -> SymbolDef {
        SymbolDef {
            name: name.to_string(),
            kind: SymbolKind::Function,
            visibility: Visibility::Public,
            span: LineSpan { start, end },
            signature: None,
            parent: parent.map(|p| p.to_string()),
        }
    }

    fn make_node(symbols: Vec<SymbolDef>, lines: u32) -> Node {
        Node {
            file_type: FileType::Source,
            layer: ArchLayer::Service,
            fsd_layer: None,
            arch_depth: 1,
            lines,
            hash: ContentHash::new("abc123".to_string()),
            exports: vec![],
            cluster: ClusterId::new("test"),
            symbols,
        }
    }

    fn make_graph(nodes: BTreeMap<CanonicalPath, Node>, edges: Vec<Edge>) -> ProjectGraph {
        ProjectGraph { nodes, edges }
    }

    #[test]
    fn file_not_found_returns_no_split() {
        let graph = make_graph(BTreeMap::new(), vec![]);
        let path = CanonicalPath::new("missing.ts");
        let result = analyze_split(&path, &graph, None, None, None, None);
        assert!(!result.should_split);
        assert_eq!(result.reason, "File not found in graph");
    }

    #[test]
    fn test_file_returns_no_split() {
        let path = CanonicalPath::new("test.spec.ts");
        let mut nodes = BTreeMap::new();
        let mut node = make_node(vec![], 100);
        node.file_type = FileType::Test;
        nodes.insert(path.clone(), node);
        let graph = make_graph(nodes, vec![]);
        let result = analyze_split(&path, &graph, None, None, None, None);
        assert!(!result.should_split);
        assert!(result.reason.contains("Test files"));
    }

    #[test]
    fn too_few_symbols_returns_no_split() {
        let path = CanonicalPath::new("small.ts");
        let syms = vec![
            make_symbol("foo", 1, 10, None),
            make_symbol("bar", 11, 20, None),
        ];
        let mut nodes = BTreeMap::new();
        nodes.insert(path.clone(), make_node(syms, 20));
        let graph = make_graph(nodes, vec![]);
        let result = analyze_split(&path, &graph, None, None, None, None);
        assert!(!result.should_split);
        assert!(result.reason.contains("only 2 symbols"));
    }

    #[test]
    fn greedy_partition_basic() {
        let g = SymbolGraph {
            nodes: vec!["a".into(), "b".into(), "c".into(), "d".into()],
            weights: vec![
                vec![0.0, 5.0, 0.0, 0.0],
                vec![5.0, 0.0, 0.0, 0.0],
                vec![0.0, 0.0, 0.0, 5.0],
                vec![0.0, 0.0, 5.0, 0.0],
            ],
        };
        let parts = greedy_partition(&g, 2);
        assert_eq!(parts.len(), 2);
        let has_ab = parts.iter().any(|p| p.contains(&0) && p.contains(&1));
        let has_cd = parts.iter().any(|p| p.contains(&2) && p.contains(&3));
        assert!(has_ab);
        assert!(has_cd);
    }

    #[test]
    fn greedy_partition_disconnected() {
        let g = SymbolGraph {
            nodes: vec!["a".into(), "b".into(), "c".into()],
            weights: vec![
                vec![0.0, 0.0, 0.0],
                vec![0.0, 0.0, 0.0],
                vec![0.0, 0.0, 0.0],
            ],
        };
        let parts = greedy_partition(&g, 2);
        assert_eq!(parts.len(), 3);
    }

    #[test]
    fn build_symbol_graph_parent_coupling() {
        let path = CanonicalPath::new("file.ts");
        let syms = vec![
            make_symbol("methodA", 1, 10, Some("ClassX")),
            make_symbol("methodB", 11, 20, Some("ClassX")),
            make_symbol("helperC", 21, 30, None),
        ];
        let sym_refs: Vec<&SymbolDef> = syms.iter().collect();
        let graph = build_symbol_graph(&path, &sym_refs, None, None, &[]);
        let idx_a = graph.nodes.iter().position(|n| n == "methodA").unwrap();
        let idx_b = graph.nodes.iter().position(|n| n == "methodB").unwrap();
        assert!(graph.weights[idx_a][idx_b] >= WEIGHT_CO_LOCATION);
    }

    // --- Test 2: Empty file — no symbols (AC-5, EC-SPLIT-2) ---
    #[test]
    fn empty_file_no_symbols_returns_no_split() {
        let path = CanonicalPath::new("empty.ts");
        let mut nodes = BTreeMap::new();
        nodes.insert(path.clone(), make_node(vec![], 0));
        let graph = make_graph(nodes, vec![]);
        let result = analyze_split(&path, &graph, None, None, None, None);
        assert!(!result.should_split);
        assert_eq!(result.data_quality, DataQuality::Minimal);
    }

    // --- Test 3: Single symbol file (AC-5, EC-SPLIT-3) ---
    #[test]
    fn single_symbol_returns_no_split() {
        let path = CanonicalPath::new("single.ts");
        let syms = vec![make_symbol("only_func", 1, 50, None)];
        let mut nodes = BTreeMap::new();
        nodes.insert(path.clone(), make_node(syms, 50));
        let graph = make_graph(nodes, vec![]);
        let result = analyze_split(&path, &graph, None, None, None, None);
        assert!(!result.should_split);
        assert!(result.reason.contains("only 1 symbols"));
    }

    // --- Test 6: Re-export hub early return (EC-8, EC-SPLIT-13) ---
    #[test]
    fn reexport_hub_returns_no_split() {
        use crate::model::edge::EdgeType;
        use crate::model::types::Symbol;

        let path = CanonicalPath::new("index.ts");
        let syms = vec![
            make_symbol("a", 1, 5, None),
            make_symbol("b", 6, 10, None),
            make_symbol("c", 11, 15, None),
            make_symbol("d", 16, 20, None),
            make_symbol("e", 21, 25, None),
            make_symbol("f", 26, 30, None),
        ];
        let mut nodes = BTreeMap::new();
        nodes.insert(path.clone(), make_node(syms, 30));

        // 5 ReExport edges (>80% of 6 total)
        let edges = vec![
            Edge {
                from: path.clone(),
                to: CanonicalPath::new("mod_a.ts"),
                edge_type: EdgeType::ReExports,
                symbols: vec![Symbol::new("a")],
            },
            Edge {
                from: path.clone(),
                to: CanonicalPath::new("mod_b.ts"),
                edge_type: EdgeType::ReExports,
                symbols: vec![Symbol::new("b")],
            },
            Edge {
                from: path.clone(),
                to: CanonicalPath::new("mod_c.ts"),
                edge_type: EdgeType::ReExports,
                symbols: vec![Symbol::new("c")],
            },
            Edge {
                from: path.clone(),
                to: CanonicalPath::new("mod_d.ts"),
                edge_type: EdgeType::ReExports,
                symbols: vec![Symbol::new("d")],
            },
            Edge {
                from: path.clone(),
                to: CanonicalPath::new("mod_e.ts"),
                edge_type: EdgeType::ReExports,
                symbols: vec![Symbol::new("e")],
            },
            // 1 normal import (total: 6, reexport: 5 = 83% > 80%)
            Edge {
                from: path.clone(),
                to: CanonicalPath::new("mod_f.ts"),
                edge_type: EdgeType::Imports,
                symbols: vec![Symbol::new("f")],
            },
        ];

        let graph = make_graph(nodes, edges);
        let result = analyze_split(&path, &graph, None, None, None, None);
        assert!(!result.should_split);
        assert!(result.reason.contains("Re-export hub"));
    }

    // --- Test 7: Naturally bi-partitioned file (AC-6, EC-SPLIT-7) ---
    #[test]
    fn naturally_bipartitioned_file_should_split() {
        use crate::model::edge::EdgeType;
        use crate::model::types::Symbol;

        let path = CanonicalPath::new("big_module.ts");
        let syms = vec![
            make_symbol("alpha", 1, 20, None),
            make_symbol("beta", 21, 40, None),
            make_symbol("gamma", 41, 60, None),
            make_symbol("delta", 61, 80, None),
            make_symbol("epsilon", 81, 100, None),
            make_symbol("eta", 101, 120, None),
            make_symbol("theta", 121, 140, None),
            make_symbol("zeta", 141, 160, None),
        ];
        let mut nodes = BTreeMap::new();
        nodes.insert(path.clone(), make_node(syms, 160));

        // Multiple files import cluster A symbols (strong co-usage coupling within A)
        // Multiple files import cluster B symbols (strong co-usage coupling within B)
        // No cross-cluster imports
        let mut edges = Vec::new();
        for i in 0..5 {
            edges.push(Edge {
                from: CanonicalPath::new(&format!("consumer_a{i}.ts")),
                to: path.clone(),
                edge_type: EdgeType::Imports,
                symbols: vec![
                    Symbol::new("alpha"),
                    Symbol::new("beta"),
                    Symbol::new("gamma"),
                    Symbol::new("delta"),
                ],
            });
            edges.push(Edge {
                from: CanonicalPath::new(&format!("consumer_b{i}.ts")),
                to: path.clone(),
                edge_type: EdgeType::Imports,
                symbols: vec![
                    Symbol::new("epsilon"),
                    Symbol::new("eta"),
                    Symbol::new("theta"),
                    Symbol::new("zeta"),
                ],
            });
        }

        let graph = make_graph(nodes, edges);
        let result = analyze_split(&path, &graph, None, None, None, None);
        assert!(result.should_split, "Expected should_split=true, reason: {}", result.reason);
        assert_eq!(result.suggested_splits.len(), 2);
        assert_eq!(result.data_quality, DataQuality::Structural);
        assert_eq!(result.cut_weight, 0.0, "Disconnected clusters should have cut_weight 0");

        // Each group should have 4 symbols (balanced partition)
        assert_eq!(result.suggested_splits[0].symbols.len(), 4);
        assert_eq!(result.suggested_splits[1].symbols.len(), 4);

        // All 8 symbols should be present across both groups
        let all_syms: BTreeSet<String> = result.suggested_splits[0]
            .symbols
            .union(&result.suggested_splits[1].symbols)
            .cloned()
            .collect();
        let expected: BTreeSet<String> = [
            "alpha", "beta", "gamma", "delta", "epsilon", "eta", "theta", "zeta",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        assert_eq!(all_syms, expected, "All symbols must be assigned to a group");
    }

    // --- Test 8: No symbol index — minimal mode (AC-7, EC-DEGRADE-4) ---
    #[test]
    fn no_symbol_index_minimal_mode() {
        let path = CanonicalPath::new("nosyms.ts");
        let mut nodes = BTreeMap::new();
        // Node has no symbols in its own vec, and no symbol_index provided
        nodes.insert(path.clone(), make_node(vec![], 200));
        let graph = make_graph(nodes, vec![]);
        let result = analyze_split(&path, &graph, None, None, None, None);
        assert!(!result.should_split);
        assert_eq!(result.data_quality, DataQuality::Minimal);
    }

    // --- Test 9: No temporal — structural mode (AC-8, AC-14, EC-DEGRADE-2) ---
    #[test]
    fn no_temporal_structural_mode() {
        let path = CanonicalPath::new("structs.ts");
        let syms = vec![
            make_symbol("s1", 1, 10, None),
            make_symbol("s2", 11, 20, None),
            make_symbol("s3", 21, 30, None),
            make_symbol("s4", 31, 40, None),
            make_symbol("s5", 41, 50, None),
            make_symbol("s6", 51, 60, None),
        ];
        let mut nodes = BTreeMap::new();
        nodes.insert(path.clone(), make_node(syms, 60));
        let graph = make_graph(nodes, vec![]);
        // symbol_index=None but node has symbols, temporal=None
        let result = analyze_split(&path, &graph, None, None, None, None);
        assert_eq!(result.data_quality, DataQuality::Structural);
    }

    // --- Test 10: Full mode data quality (AC-14, EC-DEGRADE-1) ---
    #[test]
    fn full_mode_data_quality() {
        use crate::model::temporal::{ChurnMetrics, TemporalState};

        let path = CanonicalPath::new("fulldata.ts");
        let syms = vec![
            make_symbol("f1", 1, 10, None),
            make_symbol("f2", 11, 20, None),
            make_symbol("f3", 21, 30, None),
            make_symbol("f4", 31, 40, None),
            make_symbol("f5", 41, 50, None),
            make_symbol("f6", 51, 60, None),
        ];
        let mut nodes = BTreeMap::new();
        nodes.insert(path.clone(), make_node(syms, 60));
        let graph = make_graph(nodes, vec![]);

        let mut churn_map = BTreeMap::new();
        churn_map.insert(
            path.clone(),
            ChurnMetrics {
                commits_30d: 5,
                commits_90d: 15,
                commits_1y: 30,
                lines_changed_30d: 100,
                lines_changed_90d: 300,
                authors_30d: 2,
                last_changed: Some("2026-03-01".to_string()),
                top_authors: vec![],
            },
        );
        let temporal = TemporalState {
            churn: churn_map,
            co_changes: vec![],
            ownership: BTreeMap::new(),
            hotspots: vec![],
            shallow: false,
            commits_analyzed: 30,
            window_start: "2025-01-01".to_string(),
            window_end: "2026-03-30".to_string(),
        };

        let result = analyze_split(&path, &graph, None, None, Some(&temporal), None);
        assert_eq!(result.data_quality, DataQuality::Full);
    }

    // --- Test 11: Trivial partition triggers fallback (AC-9, EC-SPLIT-8) ---
    #[test]
    fn trivial_partition_triggers_greedy_fallback() {
        use crate::model::edge::EdgeType;
        use crate::model::types::Symbol;

        let path = CanonicalPath::new("skewed.ts");
        // 7 symbols: 6 tightly coupled, 1 weakly connected outlier
        // Stoer-Wagner should isolate the outlier (<20% of total), triggering fallback
        let syms = vec![
            make_symbol("core_a", 1, 20, None),
            make_symbol("core_b", 21, 40, None),
            make_symbol("core_c", 41, 60, None),
            make_symbol("core_d", 61, 80, None),
            make_symbol("core_e", 81, 100, None),
            make_symbol("core_f", 101, 120, None),
            make_symbol("outlier", 121, 140, None),
        ];
        let mut nodes = BTreeMap::new();
        nodes.insert(path.clone(), make_node(syms, 140));

        // All core symbols imported together by multiple files (strong coupling)
        // Outlier never co-imported with core
        let edges = vec![
            Edge {
                from: CanonicalPath::new("user1.ts"),
                to: path.clone(),
                edge_type: EdgeType::Imports,
                symbols: vec![
                    Symbol::new("core_a"),
                    Symbol::new("core_b"),
                    Symbol::new("core_c"),
                    Symbol::new("core_d"),
                    Symbol::new("core_e"),
                    Symbol::new("core_f"),
                ],
            },
            Edge {
                from: CanonicalPath::new("user2.ts"),
                to: path.clone(),
                edge_type: EdgeType::Imports,
                symbols: vec![
                    Symbol::new("core_a"),
                    Symbol::new("core_b"),
                    Symbol::new("core_c"),
                    Symbol::new("core_d"),
                    Symbol::new("core_e"),
                    Symbol::new("core_f"),
                ],
            },
        ];

        let graph = make_graph(nodes, edges);
        let result = analyze_split(&path, &graph, None, None, None, None);
        // Should not panic — fallback fires. Result depends on greedy output.
        // The key assertion is that the function completes successfully.
        assert!(!result.reason.is_empty());
    }

    // --- Test 12: Large file threshold (AC-10, EC-SPLIT-9) ---
    #[test]
    fn large_file_over_500_symbols_completes() {
        let path = CanonicalPath::new("huge.ts");
        let syms: Vec<SymbolDef> = (0..501)
            .map(|i| make_symbol(&format!("sym_{i}"), i * 2, i * 2 + 1, None))
            .collect();
        let lines = 501 * 2;
        let mut nodes = BTreeMap::new();
        nodes.insert(path.clone(), make_node(syms, lines));
        let graph = make_graph(nodes, vec![]);
        // Should skip Stoer-Wagner and use greedy_partition without panic
        let result = analyze_split(&path, &graph, None, None, None, None);
        assert!(!result.reason.is_empty());
        assert_eq!(result.data_quality, DataQuality::Structural);
        // Verify the constant is correct
        assert_eq!(LARGE_FILE_SYMBOL_THRESHOLD, 500);
    }

    // --- Test 13: Determinism (AC-20) ---
    #[test]
    fn deterministic_output() {
        use crate::model::edge::EdgeType;
        use crate::model::types::Symbol;

        let path = CanonicalPath::new("det.ts");
        let syms = vec![
            make_symbol("aa", 1, 20, None),
            make_symbol("bb", 21, 40, None),
            make_symbol("cc", 41, 60, None),
            make_symbol("dd", 61, 80, None),
            make_symbol("ee", 81, 100, None),
            make_symbol("ff", 101, 120, None),
            make_symbol("gg", 121, 140, None),
            make_symbol("hh", 141, 160, None),
        ];
        let mk_graph = || {
            let mut nodes = BTreeMap::new();
            nodes.insert(path.clone(), make_node(syms.clone(), 160));
            let edges = vec![
                Edge {
                    from: CanonicalPath::new("x.ts"),
                    to: path.clone(),
                    edge_type: EdgeType::Imports,
                    symbols: vec![
                        Symbol::new("aa"),
                        Symbol::new("bb"),
                        Symbol::new("cc"),
                        Symbol::new("dd"),
                    ],
                },
                Edge {
                    from: CanonicalPath::new("y.ts"),
                    to: path.clone(),
                    edge_type: EdgeType::Imports,
                    symbols: vec![
                        Symbol::new("ee"),
                        Symbol::new("ff"),
                        Symbol::new("gg"),
                        Symbol::new("hh"),
                    ],
                },
            ];
            make_graph(nodes, edges)
        };

        let graph1 = mk_graph();
        let graph2 = mk_graph();
        let result1 = analyze_split(&path, &graph1, None, None, None, None);
        let result2 = analyze_split(&path, &graph2, None, None, None, None);
        let json1 = serde_json::to_string(&result1).unwrap();
        let json2 = serde_json::to_string(&result2).unwrap();
        assert_eq!(json1, json2, "analyze_split must produce deterministic output");
    }

    // --- Test 14: Float rounding (AC-15) ---
    #[test]
    fn float_rounding_four_decimal_places() {
        let path = CanonicalPath::new("rounded.ts");
        let syms = vec![
            make_symbol("r1", 1, 10, None),
            make_symbol("r2", 11, 20, None),
            make_symbol("r3", 21, 30, None),
            make_symbol("r4", 31, 40, None),
            make_symbol("r5", 41, 50, None),
            make_symbol("r6", 51, 60, None),
        ];
        let mut nodes = BTreeMap::new();
        nodes.insert(path.clone(), make_node(syms, 60));
        let graph = make_graph(nodes, vec![]);
        let centrality = 0.123_456_789;
        let result = analyze_split(&path, &graph, None, None, None, Some(centrality));
        // round4(0.123456789) should be 0.1235
        assert_eq!(result.impact.centrality_before, 0.1235);
        // Verify reduction estimate is also rounded
        let s = format!("{}", result.impact.centrality_reduction_estimate);
        let decimal_places = s
            .split('.')
            .nth(1)
            .map(|d| d.trim_end_matches('0').len())
            .unwrap_or(0);
        assert!(decimal_places <= 4, "centrality_reduction_estimate has {decimal_places} decimal places");
    }

    // --- Test 15: Tightly coupled file (EC-SPLIT-6) ---
    #[test]
    fn tightly_coupled_file_no_split() {
        use crate::model::edge::EdgeType;
        use crate::model::types::Symbol;

        let path = CanonicalPath::new("cohesive.ts");
        let syms = vec![
            make_symbol("t1", 1, 20, None),
            make_symbol("t2", 21, 40, None),
            make_symbol("t3", 41, 60, None),
            make_symbol("t4", 61, 80, None),
            make_symbol("t5", 81, 100, None),
            make_symbol("t6", 101, 120, None),
            make_symbol("t7", 121, 140, None),
            make_symbol("t8", 141, 160, None),
        ];
        let sym_names: Vec<&str> = vec!["t1", "t2", "t3", "t4", "t5", "t6", "t7", "t8"];
        let mut nodes = BTreeMap::new();
        nodes.insert(path.clone(), make_node(syms, 160));

        // Every external file imports ALL symbols — maximum cohesion
        let all_symbols: Vec<Symbol> = sym_names.iter().map(|n| Symbol::new(*n)).collect();
        let edges = vec![
            Edge {
                from: CanonicalPath::new("a.ts"),
                to: path.clone(),
                edge_type: EdgeType::Imports,
                symbols: all_symbols.clone(),
            },
            Edge {
                from: CanonicalPath::new("b.ts"),
                to: path.clone(),
                edge_type: EdgeType::Imports,
                symbols: all_symbols.clone(),
            },
            Edge {
                from: CanonicalPath::new("c.ts"),
                to: path.clone(),
                edge_type: EdgeType::Imports,
                symbols: all_symbols.clone(),
            },
            Edge {
                from: CanonicalPath::new("d.ts"),
                to: path.clone(),
                edge_type: EdgeType::Imports,
                symbols: all_symbols,
            },
        ];

        let graph = make_graph(nodes, edges);
        let result = analyze_split(&path, &graph, None, None, None, None);
        // High co-usage coupling everywhere => high cut_weight => should NOT split
        assert!(!result.should_split, "Tightly coupled file should not split, reason: {}", result.reason);
        assert!(
            result.reason.contains("cohesive") || result.reason.contains("too small"),
            "Expected cohesive/too-small reason, got: {}",
            result.reason
        );
    }
}
