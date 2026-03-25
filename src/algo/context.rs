use std::collections::{BTreeSet, VecDeque};

use crate::algo::{round4, AdjacencyIndex};
use crate::model::{CanonicalPath, ClusterMap, Node, ProjectGraph, StatsOutput};

/// Task type that influences relevance scoring weights.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaskType {
    AddField,
    Refactor,
    FixBug,
    AddFeature,
    Understand,
}

impl TaskType {
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "add_field" | "addfield" => Some(Self::AddField),
            "refactor" => Some(Self::Refactor),
            "fix_bug" | "fixbug" | "fix" => Some(Self::FixBug),
            "add_feature" | "addfeature" | "feature" => Some(Self::AddFeature),
            "understand" => Some(Self::Understand),
            _ => None,
        }
    }
}

/// Context tier, ordered by priority (Target is highest).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ContextTier {
    Target,
    Direct,
    Transitive,
    Cluster,
    Interface,
}

impl ContextTier {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Target => "target",
            Self::Direct => "direct",
            Self::Transitive => "transitive",
            Self::Cluster => "cluster",
            Self::Interface => "interface",
        }
    }
}

/// A candidate file for context inclusion.
#[derive(Clone, Debug)]
pub struct ContextCandidate {
    pub path: CanonicalPath,
    pub relevance: f64,
    pub tokens: u32,
    pub tier: ContextTier,
}

/// Result of context assembly.
#[derive(Clone, Debug)]
pub struct ContextResult {
    pub selected: Vec<ContextCandidate>,
    pub total_tokens: u32,
    pub budget_used: u32,
    pub warnings: Vec<String>,
}

/// Estimate token count for a node: lines * 8.
pub fn estimate_tokens(node: &Node) -> u32 {
    node.lines * 8
}

/// Score relevance and determine tier for a path relative to center files.
/// Returns (relevance_score, tier).
pub fn score_relevance(
    path: &CanonicalPath,
    center: &BTreeSet<CanonicalPath>,
    graph: &ProjectGraph,
    index: &AdjacencyIndex,
    stats: &StatsOutput,
    task: TaskType,
) -> (f64, ContextTier) {
    // Target files always get max relevance
    if center.contains(path) {
        return (round4(1.0), ContextTier::Target);
    }

    // BFS distance from any center file (using forward + reverse edges)
    let distance = bfs_distance_from_centers(path, center, index);

    // Base relevance from distance (closer = higher)
    let dist_score = match distance {
        Some(1) => 0.8,
        Some(2) => 0.5,
        Some(d) if d <= 4 => 0.3 / (d as f64 - 1.0),
        Some(_) => 0.05,
        None => 0.02,
    };

    // Centrality bonus
    let centrality = stats
        .centrality
        .get(path.as_str())
        .copied()
        .unwrap_or(0.0);
    let centrality_bonus = centrality * 0.2;

    // Determine tier
    let tier = match distance {
        Some(1) => ContextTier::Direct,
        Some(2) | Some(3) => ContextTier::Transitive,
        _ => {
            // Check if same cluster as any center file
            let same_cluster = center.iter().any(|c| {
                let c_cluster = graph.nodes.get(c).map(|n| &n.cluster);
                let p_cluster = graph.nodes.get(path).map(|n| &n.cluster);
                c_cluster.is_some() && c_cluster == p_cluster
            });
            if same_cluster {
                ContextTier::Cluster
            } else {
                ContextTier::Interface
            }
        }
    };

    // Apply task-specific weight multipliers
    let task_weight = compute_task_weight(path, graph, task);
    let raw = (dist_score + centrality_bonus) * task_weight;
    (round4(raw.min(1.0)), tier)
}

/// Compute task-specific weight multiplier for a file.
fn compute_task_weight(path: &CanonicalPath, graph: &ProjectGraph, task: TaskType) -> f64 {
    let node = match graph.nodes.get(path) {
        Some(n) => n,
        None => return 1.0,
    };

    let is_test = node.file_type == crate::model::FileType::Test;
    let is_interface = node.file_type == crate::model::FileType::TypeDef;

    match task {
        TaskType::FixBug => {
            if is_test {
                1.5
            } else {
                1.0
            }
        }
        TaskType::Refactor => {
            if is_test {
                1.3
            } else {
                1.0
            }
        }
        TaskType::AddField => {
            if is_interface {
                1.5
            } else {
                1.0
            }
        }
        TaskType::Understand => {
            // Boost high-centrality files
            1.3
        }
        TaskType::AddFeature => 1.0,
    }
}

/// BFS distance from any center file to target, using both forward and reverse edges.
fn bfs_distance_from_centers(
    target: &CanonicalPath,
    centers: &BTreeSet<CanonicalPath>,
    index: &AdjacencyIndex,
) -> Option<u32> {
    let mut visited = BTreeSet::new();
    let mut queue = VecDeque::new();

    for c in centers {
        visited.insert(c);
        queue.push_back((c, 0u32));
    }

    while let Some((current, depth)) = queue.pop_front() {
        if current == target {
            return Some(depth);
        }

        let next_depth = depth + 1;
        // Forward neighbors
        if let Some(fwd) = index.forward.get(current) {
            for neighbor in fwd {
                if visited.insert(neighbor) {
                    queue.push_back((neighbor, next_depth));
                }
            }
        }
        // Reverse neighbors
        if let Some(rev) = index.reverse.get(current) {
            for neighbor in rev {
                if visited.insert(neighbor) {
                    queue.push_back((neighbor, next_depth));
                }
            }
        }
    }

    None
}

/// Generate context candidates via BFS from center files.
pub fn generate_candidates(
    center: &BTreeSet<CanonicalPath>,
    graph: &ProjectGraph,
    index: &AdjacencyIndex,
    stats: &StatsOutput,
    depth: u32,
    task: TaskType,
) -> Vec<ContextCandidate> {
    let mut visited = BTreeSet::new();
    let mut queue = VecDeque::new();

    // Seed with center files
    for c in center {
        if graph.nodes.contains_key(c) {
            visited.insert(c.clone());
            queue.push_back((c.clone(), 0u32));
        }
    }

    let mut candidates = Vec::new();

    while let Some((current, d)) = queue.pop_front() {
        let node = match graph.nodes.get(&current) {
            Some(n) => n,
            None => continue,
        };

        let (relevance, tier) = score_relevance(&current, center, graph, index, stats, task);
        let tokens = estimate_tokens(node);

        candidates.push(ContextCandidate {
            path: current.clone(),
            relevance,
            tokens,
            tier,
        });

        if d < depth {
            let next_depth = d + 1;
            // Forward neighbors
            if let Some(fwd) = index.forward.get(&current) {
                for neighbor in fwd {
                    if !visited.contains(*neighbor) {
                        visited.insert((*neighbor).clone());
                        queue.push_back(((*neighbor).clone(), next_depth));
                    }
                }
            }
            // Reverse neighbors
            if let Some(rev) = index.reverse.get(&current) {
                for neighbor in rev {
                    if !visited.contains(*neighbor) {
                        visited.insert((*neighbor).clone());
                        queue.push_back(((*neighbor).clone(), next_depth));
                    }
                }
            }
        }
    }

    // Sort deterministically by path
    candidates.sort_by(|a, b| a.path.cmp(&b.path));
    candidates
}

/// Select candidates within a token budget.
/// Sort by tier (ascending = higher priority first), then by relevance/tokens ratio (descending).
/// Targets are always included.
pub fn select_within_budget(
    mut candidates: Vec<ContextCandidate>,
    budget: u32,
) -> ContextResult {
    // Sort: tier ascending (Target first), then relevance/tokens ratio descending
    candidates.sort_by(|a, b| {
        a.tier.cmp(&b.tier).then_with(|| {
            let ratio_a = if a.tokens > 0 {
                a.relevance / a.tokens as f64
            } else {
                f64::MAX
            };
            let ratio_b = if b.tokens > 0 {
                b.relevance / b.tokens as f64
            } else {
                f64::MAX
            };
            ratio_b
                .partial_cmp(&ratio_a)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
    });

    let mut selected = Vec::new();
    let mut budget_used = 0u32;
    let mut warnings = Vec::new();
    let total_tokens: u32 = candidates.iter().map(|c| c.tokens).sum();

    // Check if targets alone exceed budget
    let target_tokens: u32 = candidates
        .iter()
        .filter(|c| c.tier == ContextTier::Target)
        .map(|c| c.tokens)
        .sum();
    if target_tokens > budget {
        warnings.push(format!(
            "Target files alone require {} tokens, exceeding budget of {}",
            target_tokens, budget
        ));
    }

    for candidate in candidates {
        if candidate.tier == ContextTier::Target {
            // Always include targets
            budget_used = budget_used.saturating_add(candidate.tokens);
            selected.push(candidate);
        } else if budget_used.saturating_add(candidate.tokens) <= budget {
            budget_used = budget_used.saturating_add(candidate.tokens);
            selected.push(candidate);
        }
    }

    // Sort selected by path for deterministic output
    selected.sort_by(|a, b| a.path.cmp(&b.path));

    ContextResult {
        selected,
        total_tokens,
        budget_used,
        warnings,
    }
}

/// Assemble context: generate candidates and select within budget.
#[allow(clippy::too_many_arguments)] // Intentional: parameters are orthogonal domain inputs
pub fn assemble_context(
    files: &[CanonicalPath],
    graph: &ProjectGraph,
    index: &AdjacencyIndex,
    stats: &StatsOutput,
    _clusters: &ClusterMap,
    task: TaskType,
    budget_tokens: u32,
    depth: u32,
) -> ContextResult {
    let center: BTreeSet<CanonicalPath> = files.iter().cloned().collect();
    let candidates = generate_candidates(&center, graph, index, stats, depth, task);
    select_within_budget(candidates, budget_tokens)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::algo::is_architectural;
    use crate::model::*;

    fn make_graph(node_names: &[&str], edges: &[(&str, &str)]) -> ProjectGraph {
        let mut nodes = BTreeMap::new();
        for name in node_names {
            nodes.insert(
                CanonicalPath::new(*name),
                Node {
                    file_type: FileType::Source,
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
        let edges = edges
            .iter()
            .map(|(from, to)| Edge {
                from: CanonicalPath::new(*from),
                to: CanonicalPath::new(*to),
                edge_type: EdgeType::Imports,
                symbols: vec![],
            })
            .collect();
        ProjectGraph { nodes, edges }
    }

    fn make_stats(centrality: &[(&str, f64)]) -> StatsOutput {
        StatsOutput {
            version: 1,
            centrality: centrality
                .iter()
                .map(|(k, v)| (k.to_string(), *v))
                .collect(),
            sccs: vec![],
            layers: BTreeMap::new(),
            summary: StatsSummary {
                max_depth: 0,
                avg_in_degree: 0.0,
                avg_out_degree: 0.0,
                bottleneck_files: vec![],
                orphan_files: vec![],
            },
        }
    }

    #[test]
    fn estimate_tokens_basic() {
        let node = Node {
            file_type: FileType::Source,
            layer: ArchLayer::Unknown,
            fsd_layer: None,
            arch_depth: 0,
            lines: 100,
            hash: ContentHash::new("0".to_string()),
            exports: vec![],
            cluster: ClusterId::new("default"),
            symbols: Vec::new(),
        };
        assert_eq!(estimate_tokens(&node), 800);
    }

    #[test]
    fn estimate_tokens_zero_lines() {
        let node = Node {
            file_type: FileType::Source,
            layer: ArchLayer::Unknown,
            fsd_layer: None,
            arch_depth: 0,
            lines: 0,
            hash: ContentHash::new("0".to_string()),
            exports: vec![],
            cluster: ClusterId::new("default"),
            symbols: Vec::new(),
        };
        assert_eq!(estimate_tokens(&node), 0);
    }

    #[test]
    fn score_relevance_target_is_max() {
        let graph = make_graph(&["a", "b"], &[("a", "b")]);
        let index = AdjacencyIndex::build(&graph.edges, is_architectural);
        let stats = make_stats(&[]);
        let center: BTreeSet<_> = [CanonicalPath::new("a")].into_iter().collect();

        let (score, tier) =
            score_relevance(&CanonicalPath::new("a"), &center, &graph, &index, &stats, TaskType::AddFeature);
        assert_eq!(score, 1.0);
        assert_eq!(tier, ContextTier::Target);
    }

    #[test]
    fn score_relevance_direct_higher_than_transitive() {
        // a -> b -> c
        let graph = make_graph(&["a", "b", "c"], &[("a", "b"), ("b", "c")]);
        let index = AdjacencyIndex::build(&graph.edges, is_architectural);
        let stats = make_stats(&[]);
        let center: BTreeSet<_> = [CanonicalPath::new("a")].into_iter().collect();

        let (score_b, tier_b) =
            score_relevance(&CanonicalPath::new("b"), &center, &graph, &index, &stats, TaskType::AddFeature);
        let (score_c, tier_c) =
            score_relevance(&CanonicalPath::new("c"), &center, &graph, &index, &stats, TaskType::AddFeature);

        assert_eq!(tier_b, ContextTier::Direct);
        assert_eq!(tier_c, ContextTier::Transitive);
        assert!(score_b > score_c);
    }

    #[test]
    fn budget_selection_respects_limit() {
        let candidates = vec![
            ContextCandidate {
                path: CanonicalPath::new("a"),
                relevance: 1.0,
                tokens: 100,
                tier: ContextTier::Target,
            },
            ContextCandidate {
                path: CanonicalPath::new("b"),
                relevance: 0.8,
                tokens: 100,
                tier: ContextTier::Direct,
            },
            ContextCandidate {
                path: CanonicalPath::new("c"),
                relevance: 0.5,
                tokens: 100,
                tier: ContextTier::Transitive,
            },
        ];

        let result = select_within_budget(candidates, 200);
        assert_eq!(result.selected.len(), 2);
        assert_eq!(result.budget_used, 200);
        // Target always included
        assert!(result.selected.iter().any(|c| c.path.as_str() == "a"));
    }

    #[test]
    fn budget_warns_when_targets_exceed() {
        let candidates = vec![
            ContextCandidate {
                path: CanonicalPath::new("a"),
                relevance: 1.0,
                tokens: 500,
                tier: ContextTier::Target,
            },
        ];

        let result = select_within_budget(candidates, 100);
        assert_eq!(result.selected.len(), 1); // Target still included
        assert!(!result.warnings.is_empty());
    }

    #[test]
    fn assemble_context_end_to_end() {
        // a -> b -> c
        let graph = make_graph(&["a", "b", "c"], &[("a", "b"), ("b", "c")]);
        let index = AdjacencyIndex::build(&graph.edges, is_architectural);
        let stats = make_stats(&[("b", 0.5)]);
        let clusters = ClusterMap {
            clusters: BTreeMap::new(),
        };

        let result = assemble_context(
            &[CanonicalPath::new("a")],
            &graph,
            &index,
            &stats,
            &clusters,
            TaskType::Understand,
            10000,
            3,
        );

        // Should include a (target), b (direct), c (transitive)
        assert_eq!(result.selected.len(), 3);
        assert!(result.selected.iter().any(|c| c.tier == ContextTier::Target));
    }

    #[test]
    fn task_weight_fixbug_boosts_tests() {
        let mut graph = make_graph(&["src/lib.rs", "tests/lib_test.rs"], &[("tests/lib_test.rs", "src/lib.rs")]);
        // Mark test file
        graph
            .nodes
            .get_mut(&CanonicalPath::new("tests/lib_test.rs"))
            .unwrap()
            .file_type = FileType::Test;

        let index = AdjacencyIndex::build(&graph.edges, is_architectural);
        let stats = make_stats(&[]);
        let center: BTreeSet<_> = [CanonicalPath::new("src/lib.rs")].into_iter().collect();

        let (score_fix, _) = score_relevance(
            &CanonicalPath::new("tests/lib_test.rs"),
            &center,
            &graph,
            &index,
            &stats,
            TaskType::FixBug,
        );
        let (score_feat, _) = score_relevance(
            &CanonicalPath::new("tests/lib_test.rs"),
            &center,
            &graph,
            &index,
            &stats,
            TaskType::AddFeature,
        );

        assert!(score_fix > score_feat);
    }

    #[test]
    fn generate_candidates_bfs_expansion() {
        // a -> b -> c -> d; depth=2 should include a, b, c but not d
        let graph = make_graph(&["a", "b", "c", "d"], &[("a", "b"), ("b", "c"), ("c", "d")]);
        let index = AdjacencyIndex::build(&graph.edges, is_architectural);
        let stats = make_stats(&[]);
        let center: BTreeSet<_> = [CanonicalPath::new("a")].into_iter().collect();

        let candidates = generate_candidates(&center, &graph, &index, &stats, 2, TaskType::AddFeature);
        let paths: Vec<&str> = candidates.iter().map(|c| c.path.as_str()).collect();

        assert!(paths.contains(&"a"));
        assert!(paths.contains(&"b"));
        assert!(paths.contains(&"c"));
        assert!(!paths.contains(&"d"));
        // Verify target tier
        let target = candidates.iter().find(|c| c.path.as_str() == "a").unwrap();
        assert_eq!(target.tier, ContextTier::Target);
        // Verify sorted by path (deterministic)
        let sorted: Vec<&str> = {
            let mut s = paths.clone();
            s.sort();
            s
        };
        assert_eq!(paths, sorted);
    }

    #[test]
    fn task_type_from_str() {
        assert_eq!(TaskType::parse("fix_bug"), Some(TaskType::FixBug));
        assert_eq!(TaskType::parse("refactor"), Some(TaskType::Refactor));
        assert_eq!(TaskType::parse("add_field"), Some(TaskType::AddField));
        assert_eq!(TaskType::parse("understand"), Some(TaskType::Understand));
        assert_eq!(TaskType::parse("feature"), Some(TaskType::AddFeature));
        assert_eq!(TaskType::parse("unknown"), None);
    }
}
