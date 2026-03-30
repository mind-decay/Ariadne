use std::collections::{BTreeMap, BTreeSet};

use crate::model::{ArchLayer, CanonicalPath, Cluster, ClusterId, Node};

use super::types::{DataQuality, PlacementAlternative, PlacementSuggestion};

/// Suggest optimal file placement based on dependency relationships.
///
/// Analyzes cluster membership, architectural layers, and dependency patterns
/// to recommend where a new file should be placed in the project.
pub fn suggest_placement(
    description: &str,
    depends_on: &[CanonicalPath],
    depended_by: &[CanonicalPath],
    nodes: &BTreeMap<CanonicalPath, Node>,
    clusters: &BTreeMap<ClusterId, Cluster>,
    layer_index: &BTreeMap<u32, Vec<CanonicalPath>>,
) -> PlacementSuggestion {
    let _ = layer_index; // available for future use

    // Step 1: Deduplicate inputs (EC-PL-18)
    let depends_on_set: BTreeSet<&CanonicalPath> = depends_on.iter().collect();
    let depended_by_set: BTreeSet<&CanonicalPath> = depended_by.iter().collect();

    // Step 2: Resolve paths to nodes
    let mut resolved_deps: Vec<(&CanonicalPath, &Node)> = Vec::new();
    let mut resolved_rev: Vec<(&CanonicalPath, &Node)> = Vec::new();
    let mut unresolved_count: usize = 0;

    for path in &depends_on_set {
        if let Some(node) = nodes.get(*path) {
            resolved_deps.push((*path, node));
        } else {
            unresolved_count += 1;
        }
    }

    for path in &depended_by_set {
        if let Some(node) = nodes.get(*path) {
            resolved_rev.push((*path, node));
        } else {
            unresolved_count += 1;
        }
    }

    // Step 3: Early return for no usable data (EC-PL-1, EC-PL-9)
    if resolved_deps.is_empty() && resolved_rev.is_empty() && clusters.is_empty() {
        return PlacementSuggestion {
            suggested_path: "src/new_module".to_string(),
            cluster: String::new(),
            layer: "unknown".to_string(),
            arch_depth: 0,
            reasoning: vec![
                "No dependency context available; defaulting to root-level placement".to_string(),
            ],
            alternatives: vec![],
            data_quality: DataQuality::Minimal,
        };
    }

    let mut reasoning: Vec<String> = Vec::new();

    // Step 4: Count cluster votes (Architecture Decision 3)
    let mut cluster_votes: BTreeMap<&ClusterId, u32> = BTreeMap::new();
    let all_resolved: Vec<(&CanonicalPath, &Node)> = resolved_deps
        .iter()
        .chain(resolved_rev.iter())
        .copied()
        .collect();

    for (_, node) in &all_resolved {
        *cluster_votes.entry(&node.cluster).or_insert(0) += 1;
    }

    let total_votes: u32 = cluster_votes.values().sum();

    // Select winner: highest count, lowest ClusterId breaks ties (D3)
    let winner: Option<(&ClusterId, u32)> = cluster_votes
        .iter()
        .min_by(|(id_a, &cnt_a), (id_b, &cnt_b)| cnt_b.cmp(&cnt_a).then_with(|| id_a.cmp(id_b)))
        .map(|(id, &count)| (*id, count));

    let (winning_cluster_id, winning_count) = winner
        .map(|(id, count)| (Some(id), count))
        .unwrap_or((None, 0));

    if let Some(cid) = winning_cluster_id {
        let pct = if total_votes > 0 {
            (winning_count as f64 / total_votes as f64 * 100.0) as u32
        } else {
            0
        };
        reasoning.push(format!(
            "Cluster '{}' selected with {}/{} dependency votes ({}%)",
            cid, winning_count, total_votes, pct
        ));
        if pct < 80 {
            reasoning.push(format!(
                "Confidence is moderate ({}% < 80%); consider alternatives",
                pct
            ));
        }
    }

    // Step 5: Compute arch_depth (Architecture Decision 4)
    let dep_depths: Vec<u32> = resolved_deps.iter().map(|(_, n)| n.arch_depth).collect();
    let rev_depths: Vec<u32> = resolved_rev.iter().map(|(_, n)| n.arch_depth).collect();

    let suggested_depth = if !dep_depths.is_empty() && !rev_depths.is_empty() {
        let max_dep = dep_depths.iter().copied().max().unwrap_or(0);
        let min_rev = rev_depths.iter().copied().min().unwrap_or(0);
        let depth = max_dep + 1;
        if depth > min_rev {
            reasoning.push(format!(
                "Layer conflict: suggested depth {} exceeds minimum reverse-dep depth {}",
                depth, min_rev
            ));
        }
        depth
    } else if !dep_depths.is_empty() {
        dep_depths.iter().copied().max().unwrap_or(0) + 1
    } else if !rev_depths.is_empty() {
        rev_depths.iter().copied().min().unwrap_or(0)
    } else {
        0
    };

    // Step 6: Determine layer name
    let mut layer_votes: Vec<(ArchLayer, u32)> = Vec::new();
    for (_, node) in &all_resolved {
        if let Some(entry) = layer_votes.iter_mut().find(|(l, _)| *l == node.layer) {
            entry.1 += 1;
        } else {
            layer_votes.push((node.layer, 1));
        }
    }
    // Sort by count descending, then by ArchLayer (Ord) ascending for determinism
    layer_votes.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

    let layer_name = layer_votes
        .first()
        .map(|(l, _)| l.as_str().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    reasoning.push(format!("Inferred architectural layer: {}", layer_name));

    // Step 7: Generate suggested path (Architecture Decision 5)
    let winning_cluster = winning_cluster_id.and_then(|cid| clusters.get(cid));

    let prefix = winning_cluster
        .map(|c| {
            let paths: Vec<&CanonicalPath> = c.files.iter().collect();
            common_path_prefix(&paths)
        })
        .unwrap_or_default();

    let ext = winning_cluster
        .map(|c| {
            let paths: Vec<&CanonicalPath> = c.files.iter().collect();
            most_common_extension(&paths)
        })
        .unwrap_or_else(|| "rs".to_string());

    let sanitized = sanitize_to_filename(description);
    let base_path = if prefix.is_empty() {
        format!("{}.{}", sanitized, ext)
    } else {
        format!("{}{}.{}", prefix, sanitized, ext)
    };

    // Conflict check (EC-PL-15)
    let mut suggested_path = base_path.clone();
    let mut suffix = 2u32;
    while nodes.contains_key(&CanonicalPath::new(&suggested_path)) {
        suggested_path = if prefix.is_empty() {
            format!("{}_{}.{}", sanitized, suffix, ext)
        } else {
            format!("{}{}_{}.{}", prefix, sanitized, suffix, ext)
        };
        suffix += 1;
    }

    // Step 8: Detect circular references (Architecture Decision 6, EC-PL-15)
    let circular: BTreeSet<&CanonicalPath> = depends_on_set
        .intersection(&depended_by_set)
        .copied()
        .collect();
    for path in &circular {
        reasoning.push(format!("Warning: circular dependency with {}", path));
    }

    // Step 9: Generate alternatives (up to 3)
    let mut alternatives: Vec<PlacementAlternative> = Vec::new();
    if let Some(winner_id) = winning_cluster_id {
        let mut runner_ups: Vec<(&ClusterId, u32)> = cluster_votes
            .iter()
            .filter(|(id, _)| **id != winner_id)
            .map(|(id, &count)| (*id, count))
            .collect();
        runner_ups.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

        for (cid, count) in runner_ups.into_iter().take(3) {
            let alt_cluster = clusters.get(cid);
            let alt_prefix = alt_cluster
                .map(|c| {
                    let paths: Vec<&CanonicalPath> = c.files.iter().collect();
                    common_path_prefix(&paths)
                })
                .unwrap_or_default();
            let alt_ext = alt_cluster
                .map(|c| {
                    let paths: Vec<&CanonicalPath> = c.files.iter().collect();
                    most_common_extension(&paths)
                })
                .unwrap_or_else(|| "rs".to_string());

            let alt_path = if alt_prefix.is_empty() {
                format!("{}.{}", sanitized, alt_ext)
            } else {
                format!("{}{}.{}", alt_prefix, sanitized, alt_ext)
            };

            alternatives.push(PlacementAlternative {
                path: alt_path,
                cluster: cid.as_str().to_string(),
                risk: format!(
                    "Lower dependency overlap ({}/{} files)",
                    count, total_votes
                ),
            });
        }
    }

    // Step 10: Unresolved path count
    if unresolved_count > 0 {
        reasoning.push(format!(
            "{} dependency path(s) could not be resolved in the graph",
            unresolved_count
        ));
    }

    // Step 11: Set data_quality (Architecture Decision 7)
    let data_quality = if !nodes.is_empty()
        && !clusters.is_empty()
        && (!resolved_deps.is_empty() || !resolved_rev.is_empty())
    {
        DataQuality::Structural
    } else {
        DataQuality::Minimal
    };

    // Step 12: Return PlacementSuggestion
    PlacementSuggestion {
        suggested_path,
        cluster: winning_cluster_id
            .map(|cid| cid.as_str().to_string())
            .unwrap_or_default(),
        layer: layer_name,
        arch_depth: suggested_depth,
        reasoning,
        alternatives,
        data_quality,
    }
}

/// Sanitize a description string into a valid filename component.
/// Lowercase, replace non-alphanumeric with _, collapse multiples, trim, truncate 64.
fn sanitize_to_filename(description: &str) -> String {
    let lowered = description.to_lowercase();
    let mut result = String::with_capacity(lowered.len());
    let mut last_was_underscore = true; // treat start as underscore to trim leading

    for ch in lowered.chars() {
        if ch.is_ascii_alphanumeric() {
            result.push(ch);
            last_was_underscore = false;
        } else if !last_was_underscore {
            result.push('_');
            last_was_underscore = true;
        }
    }

    // Trim trailing underscore
    while result.ends_with('_') {
        result.pop();
    }

    // Truncate to 64 chars
    if result.len() > 64 {
        result.truncate(64);
        // Don't leave a trailing underscore after truncation
        while result.ends_with('_') {
            result.pop();
        }
    }

    if result.is_empty() {
        "new_module".to_string()
    } else {
        result
    }
}

/// Find the longest shared directory prefix among a set of paths.
/// E.g., ["src/auth/login.rs", "src/auth/session.rs"] -> "src/auth/"
fn common_path_prefix(paths: &[&CanonicalPath]) -> String {
    if paths.is_empty() {
        return String::new();
    }

    // Collect directory parts for each path
    let dirs: Vec<Vec<&str>> = paths
        .iter()
        .filter_map(|p| {
            let s = p.as_str();
            s.rfind('/').map(|i| s[..i].split('/').collect())
        })
        .collect();

    if dirs.is_empty() {
        return String::new();
    }

    let first = &dirs[0];
    let mut prefix_len = 0;

    for i in 0..first.len() {
        if dirs.iter().all(|d| d.len() > i && d[i] == first[i]) {
            prefix_len = i + 1;
        } else {
            break;
        }
    }

    if prefix_len == 0 {
        return String::new();
    }

    let mut result: String = first[..prefix_len].join("/");
    result.push('/');
    result
}

/// Find the most common file extension among a set of paths.
/// Default "rs" if none found.
fn most_common_extension(paths: &[&CanonicalPath]) -> String {
    let mut counts: BTreeMap<&str, u32> = BTreeMap::new();
    for p in paths {
        if let Some(ext) = p.extension() {
            *counts.entry(ext).or_insert(0) += 1;
        }
    }

    counts
        .into_iter()
        .min_by(|(ext_a, cnt_a), (ext_b, cnt_b)| cnt_b.cmp(cnt_a).then_with(|| ext_a.cmp(ext_b)))
        .map(|(ext, _)| ext.to_string())
        .unwrap_or_else(|| "rs".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ContentHash, FileType, Node};

    fn make_node(cluster: &str, layer: ArchLayer, depth: u32) -> Node {
        Node {
            file_type: FileType::Source,
            layer,
            fsd_layer: None,
            arch_depth: depth,
            lines: 100,
            hash: ContentHash::new("0000000000000000".to_string()),
            exports: vec![],
            cluster: ClusterId::new(cluster),
            symbols: vec![],
        }
    }

    #[test]
    fn empty_inputs_returns_minimal() {
        let result = suggest_placement(
            "test module",
            &[],
            &[],
            &BTreeMap::new(),
            &BTreeMap::new(),
            &BTreeMap::new(),
        );
        assert_eq!(result.suggested_path, "src/new_module");
        assert_eq!(result.data_quality, DataQuality::Minimal);
    }

    #[test]
    fn single_dependency_places_in_same_cluster() {
        let mut nodes = BTreeMap::new();
        nodes.insert(
            CanonicalPath::new("src/auth/login.rs"),
            make_node("auth", ArchLayer::Service, 2),
        );

        let mut clusters = BTreeMap::new();
        clusters.insert(
            ClusterId::new("auth"),
            Cluster {
                files: vec![CanonicalPath::new("src/auth/login.rs")],
                file_count: 1,
                internal_edges: 0,
                external_edges: 0,
                cohesion: 1.0,
            },
        );

        let result = suggest_placement(
            "auth helper",
            &[CanonicalPath::new("src/auth/login.rs")],
            &[],
            &nodes,
            &clusters,
            &BTreeMap::new(),
        );

        assert_eq!(result.cluster, "auth");
        assert!(result.suggested_path.starts_with("src/auth/"));
        assert!(result.suggested_path.contains("auth_helper"));
        assert_eq!(result.arch_depth, 3);
        assert_eq!(result.data_quality, DataQuality::Structural);
    }

    #[test]
    fn sanitize_to_filename_basic() {
        assert_eq!(sanitize_to_filename("Auth Helper"), "auth_helper");
        assert_eq!(sanitize_to_filename("  spaces  "), "spaces");
        assert_eq!(sanitize_to_filename("a--b!!c"), "a_b_c");
        assert_eq!(sanitize_to_filename(""), "new_module");
    }

    #[test]
    fn common_path_prefix_shared() {
        let a = CanonicalPath::new("src/auth/login.rs");
        let b = CanonicalPath::new("src/auth/session.rs");
        assert_eq!(common_path_prefix(&[&a, &b]), "src/auth/");
    }

    #[test]
    fn common_path_prefix_no_shared() {
        let a = CanonicalPath::new("src/login.rs");
        let b = CanonicalPath::new("lib/session.rs");
        assert_eq!(common_path_prefix(&[&a, &b]), "");
    }

    #[test]
    fn most_common_extension_picks_majority() {
        let a = CanonicalPath::new("a.ts");
        let b = CanonicalPath::new("b.ts");
        let c = CanonicalPath::new("c.rs");
        assert_eq!(most_common_extension(&[&a, &b, &c]), "ts");
    }

    // --- AC-3: All deps in same cluster -> cluster is selected ---
    #[test]
    fn all_deps_same_cluster_selects_that_cluster() {
        let mut nodes = BTreeMap::new();
        nodes.insert(
            CanonicalPath::new("src/auth/login.rs"),
            make_node("auth", ArchLayer::Service, 2),
        );
        nodes.insert(
            CanonicalPath::new("src/auth/session.rs"),
            make_node("auth", ArchLayer::Service, 2),
        );
        nodes.insert(
            CanonicalPath::new("src/auth/token.rs"),
            make_node("auth", ArchLayer::Service, 2),
        );

        let mut clusters = BTreeMap::new();
        clusters.insert(
            ClusterId::new("auth"),
            Cluster {
                files: vec![
                    CanonicalPath::new("src/auth/login.rs"),
                    CanonicalPath::new("src/auth/session.rs"),
                    CanonicalPath::new("src/auth/token.rs"),
                ],
                file_count: 3,
                internal_edges: 0,
                external_edges: 0,
                cohesion: 1.0,
            },
        );

        let result = suggest_placement(
            "auth middleware",
            &[
                CanonicalPath::new("src/auth/login.rs"),
                CanonicalPath::new("src/auth/session.rs"),
                CanonicalPath::new("src/auth/token.rs"),
            ],
            &[],
            &nodes,
            &clusters,
            &BTreeMap::new(),
        );

        assert_eq!(result.cluster, "auth");
        assert!(result.suggested_path.starts_with("src/auth/"));
    }

    // --- AC-4: Layer inference from dependency depths ---
    #[test]
    fn layer_inference_from_dep_depths() {
        let mut nodes = BTreeMap::new();
        nodes.insert(
            CanonicalPath::new("src/a.rs"),
            make_node("core", ArchLayer::Service, 2),
        );
        nodes.insert(
            CanonicalPath::new("src/b.rs"),
            make_node("core", ArchLayer::Service, 2),
        );
        nodes.insert(
            CanonicalPath::new("src/c.rs"),
            make_node("core", ArchLayer::Data, 1),
        );

        let mut clusters = BTreeMap::new();
        clusters.insert(
            ClusterId::new("core"),
            Cluster {
                files: vec![
                    CanonicalPath::new("src/a.rs"),
                    CanonicalPath::new("src/b.rs"),
                    CanonicalPath::new("src/c.rs"),
                ],
                file_count: 3,
                internal_edges: 0,
                external_edges: 0,
                cohesion: 1.0,
            },
        );

        let result = suggest_placement(
            "new service",
            &[
                CanonicalPath::new("src/a.rs"),
                CanonicalPath::new("src/b.rs"),
                CanonicalPath::new("src/c.rs"),
            ],
            &[],
            &nodes,
            &clusters,
            &BTreeMap::new(),
        );

        // max dep depth is 2, so arch_depth should be >= 2
        assert!(result.arch_depth >= 2);
    }

    // --- AC-5: Alternatives have risk annotations ---
    #[test]
    fn alternatives_have_risk_annotations() {
        let mut nodes = BTreeMap::new();
        for i in 0..3 {
            nodes.insert(
                CanonicalPath::new(&format!("src/auth/f{}.rs", i)),
                make_node("auth", ArchLayer::Service, 2),
            );
        }
        for i in 0..2 {
            nodes.insert(
                CanonicalPath::new(&format!("src/users/f{}.rs", i)),
                make_node("users", ArchLayer::Service, 2),
            );
        }

        let mut clusters = BTreeMap::new();
        clusters.insert(
            ClusterId::new("auth"),
            Cluster {
                files: (0..3)
                    .map(|i| CanonicalPath::new(&format!("src/auth/f{}.rs", i)))
                    .collect(),
                file_count: 3,
                internal_edges: 0,
                external_edges: 0,
                cohesion: 1.0,
            },
        );
        clusters.insert(
            ClusterId::new("users"),
            Cluster {
                files: (0..2)
                    .map(|i| CanonicalPath::new(&format!("src/users/f{}.rs", i)))
                    .collect(),
                file_count: 2,
                internal_edges: 0,
                external_edges: 0,
                cohesion: 1.0,
            },
        );

        let deps: Vec<CanonicalPath> = (0..3)
            .map(|i| CanonicalPath::new(&format!("src/auth/f{}.rs", i)))
            .chain((0..2).map(|i| CanonicalPath::new(&format!("src/users/f{}.rs", i))))
            .collect();

        let result = suggest_placement("shared module", &deps, &[], &nodes, &clusters, &BTreeMap::new());

        assert!(
            result.alternatives.iter().all(|a| !a.risk.is_empty()),
            "All alternatives must have non-empty risk annotations"
        );
    }

    // --- AC-6: Alternatives ordered by decreasing score ---
    #[test]
    fn alternatives_ordered_by_decreasing_score() {
        let mut nodes = BTreeMap::new();
        // auth: 3 deps, users: 2 deps, config: 1 dep
        for i in 0..3 {
            nodes.insert(
                CanonicalPath::new(&format!("src/auth/f{}.rs", i)),
                make_node("auth", ArchLayer::Service, 2),
            );
        }
        for i in 0..2 {
            nodes.insert(
                CanonicalPath::new(&format!("src/users/f{}.rs", i)),
                make_node("users", ArchLayer::Service, 2),
            );
        }
        nodes.insert(
            CanonicalPath::new("src/config/f0.rs"),
            make_node("config", ArchLayer::Config, 0),
        );

        let mut clusters = BTreeMap::new();
        clusters.insert(
            ClusterId::new("auth"),
            Cluster {
                files: (0..3)
                    .map(|i| CanonicalPath::new(&format!("src/auth/f{}.rs", i)))
                    .collect(),
                file_count: 3,
                internal_edges: 0,
                external_edges: 0,
                cohesion: 1.0,
            },
        );
        clusters.insert(
            ClusterId::new("users"),
            Cluster {
                files: (0..2)
                    .map(|i| CanonicalPath::new(&format!("src/users/f{}.rs", i)))
                    .collect(),
                file_count: 2,
                internal_edges: 0,
                external_edges: 0,
                cohesion: 1.0,
            },
        );
        clusters.insert(
            ClusterId::new("config"),
            Cluster {
                files: vec![CanonicalPath::new("src/config/f0.rs")],
                file_count: 1,
                internal_edges: 0,
                external_edges: 0,
                cohesion: 1.0,
            },
        );

        let deps: Vec<CanonicalPath> = (0..3)
            .map(|i| CanonicalPath::new(&format!("src/auth/f{}.rs", i)))
            .chain((0..2).map(|i| CanonicalPath::new(&format!("src/users/f{}.rs", i))))
            .chain(std::iter::once(CanonicalPath::new("src/config/f0.rs")))
            .collect();

        let result = suggest_placement("cross module", &deps, &[], &nodes, &clusters, &BTreeMap::new());

        assert_eq!(result.cluster, "auth"); // winner with 3 votes
        assert!(result.alternatives.len() >= 2);
        // "users" (2 votes) should come before "config" (1 vote) in alternatives
        let user_idx = result.alternatives.iter().position(|a| a.cluster == "users");
        let config_idx = result.alternatives.iter().position(|a| a.cluster == "config");
        assert!(
            user_idx.unwrap() < config_idx.unwrap(),
            "users (2 votes) should appear before config (1 vote)"
        );
    }

    // --- AC-7: At most 3 alternatives ---
    #[test]
    fn at_most_three_alternatives() {
        let mut nodes = BTreeMap::new();
        let cluster_names = ["alpha", "bravo", "charlie", "delta", "echo"];

        for (i, name) in cluster_names.iter().enumerate() {
            nodes.insert(
                CanonicalPath::new(&format!("src/{}/f.rs", name)),
                make_node(name, ArchLayer::Service, 2),
            );
            // Give the first cluster an extra dep so it wins
            if i == 0 {
                nodes.insert(
                    CanonicalPath::new(&format!("src/{}/g.rs", name)),
                    make_node(name, ArchLayer::Service, 2),
                );
            }
        }

        let mut clusters = BTreeMap::new();
        for name in &cluster_names {
            let mut files = vec![CanonicalPath::new(&format!("src/{}/f.rs", name))];
            if *name == "alpha" {
                files.push(CanonicalPath::new(&format!("src/{}/g.rs", name)));
            }
            clusters.insert(
                ClusterId::new(*name),
                Cluster {
                    file_count: files.len(),
                    files,
                    internal_edges: 0,
                    external_edges: 0,
                    cohesion: 1.0,
                },
            );
        }

        let mut deps: Vec<CanonicalPath> = cluster_names
            .iter()
            .map(|name| CanonicalPath::new(&format!("src/{}/f.rs", name)))
            .collect();
        deps.push(CanonicalPath::new("src/alpha/g.rs"));

        let result = suggest_placement("multi cluster", &deps, &[], &nodes, &clusters, &BTreeMap::new());

        assert!(
            result.alternatives.len() <= 3,
            "Should have at most 3 alternatives, got {}",
            result.alternatives.len()
        );
    }

    // --- AC-9 / EC-PL-4: Unknown paths silently skipped ---
    #[test]
    fn unknown_paths_silently_skipped() {
        let result = suggest_placement(
            "ghost module",
            &[CanonicalPath::new("nonexistent/file.rs")],
            &[],
            &BTreeMap::new(),
            &BTreeMap::new(),
            &BTreeMap::new(),
        );

        // Should not panic, should return valid result
        assert_eq!(result.data_quality, DataQuality::Minimal);
    }

    // --- AC-10: Deterministic output ---
    #[test]
    fn deterministic_output() {
        let mut nodes = BTreeMap::new();
        nodes.insert(
            CanonicalPath::new("src/auth/login.rs"),
            make_node("auth", ArchLayer::Service, 2),
        );
        nodes.insert(
            CanonicalPath::new("src/users/profile.rs"),
            make_node("users", ArchLayer::Service, 1),
        );

        let mut clusters = BTreeMap::new();
        clusters.insert(
            ClusterId::new("auth"),
            Cluster {
                files: vec![CanonicalPath::new("src/auth/login.rs")],
                file_count: 1,
                internal_edges: 0,
                external_edges: 0,
                cohesion: 1.0,
            },
        );
        clusters.insert(
            ClusterId::new("users"),
            Cluster {
                files: vec![CanonicalPath::new("src/users/profile.rs")],
                file_count: 1,
                internal_edges: 0,
                external_edges: 0,
                cohesion: 1.0,
            },
        );

        let deps = vec![
            CanonicalPath::new("src/auth/login.rs"),
            CanonicalPath::new("src/users/profile.rs"),
        ];

        let r1 = suggest_placement("det test", &deps, &[], &nodes, &clusters, &BTreeMap::new());
        let r2 = suggest_placement("det test", &deps, &[], &nodes, &clusters, &BTreeMap::new());

        let j1 = serde_json::to_string(&r1).unwrap();
        let j2 = serde_json::to_string(&r2).unwrap();
        assert_eq!(j1, j2, "Output must be byte-identical across runs");
    }

    // --- AC-17: DataQuality reflects available data ---
    #[test]
    fn data_quality_structural_when_populated() {
        let mut nodes = BTreeMap::new();
        nodes.insert(
            CanonicalPath::new("src/a.rs"),
            make_node("core", ArchLayer::Service, 1),
        );

        let mut clusters = BTreeMap::new();
        clusters.insert(
            ClusterId::new("core"),
            Cluster {
                files: vec![CanonicalPath::new("src/a.rs")],
                file_count: 1,
                internal_edges: 0,
                external_edges: 0,
                cohesion: 1.0,
            },
        );

        let result = suggest_placement(
            "quality test",
            &[CanonicalPath::new("src/a.rs")],
            &[],
            &nodes,
            &clusters,
            &BTreeMap::new(),
        );
        assert_eq!(result.data_quality, DataQuality::Structural);
    }

    #[test]
    fn data_quality_minimal_when_empty_clusters() {
        let result = suggest_placement(
            "quality test",
            &[],
            &[],
            &BTreeMap::new(),
            &BTreeMap::new(),
            &BTreeMap::new(),
        );
        assert_eq!(result.data_quality, DataQuality::Minimal);
    }

    // --- EC-PL-3: Equal cluster split ---
    #[test]
    fn equal_cluster_split_deterministic() {
        let mut nodes = BTreeMap::new();
        nodes.insert(
            CanonicalPath::new("src/auth/a.rs"),
            make_node("auth", ArchLayer::Service, 2),
        );
        nodes.insert(
            CanonicalPath::new("src/auth/b.rs"),
            make_node("auth", ArchLayer::Service, 2),
        );
        nodes.insert(
            CanonicalPath::new("src/users/a.rs"),
            make_node("users", ArchLayer::Service, 2),
        );
        nodes.insert(
            CanonicalPath::new("src/users/b.rs"),
            make_node("users", ArchLayer::Service, 2),
        );

        let mut clusters = BTreeMap::new();
        clusters.insert(
            ClusterId::new("auth"),
            Cluster {
                files: vec![
                    CanonicalPath::new("src/auth/a.rs"),
                    CanonicalPath::new("src/auth/b.rs"),
                ],
                file_count: 2,
                internal_edges: 0,
                external_edges: 0,
                cohesion: 1.0,
            },
        );
        clusters.insert(
            ClusterId::new("users"),
            Cluster {
                files: vec![
                    CanonicalPath::new("src/users/a.rs"),
                    CanonicalPath::new("src/users/b.rs"),
                ],
                file_count: 2,
                internal_edges: 0,
                external_edges: 0,
                cohesion: 1.0,
            },
        );

        let deps = vec![
            CanonicalPath::new("src/auth/a.rs"),
            CanonicalPath::new("src/auth/b.rs"),
            CanonicalPath::new("src/users/a.rs"),
            CanonicalPath::new("src/users/b.rs"),
        ];

        let r1 = suggest_placement("split test", &deps, &[], &nodes, &clusters, &BTreeMap::new());
        let r2 = suggest_placement("split test", &deps, &[], &nodes, &clusters, &BTreeMap::new());

        // Must be deterministic
        assert_eq!(r1.cluster, r2.cluster);

        // The non-winning cluster should appear in alternatives
        let other = if r1.cluster == "auth" { "users" } else { "auth" };
        assert!(
            r1.alternatives.iter().any(|a| a.cluster == other),
            "The other cluster should appear as an alternative"
        );
    }

    // --- EC-PL-5: Mixed found/not-found dependencies ---
    #[test]
    fn mixed_found_and_not_found_dependencies() {
        let mut nodes = BTreeMap::new();
        nodes.insert(
            CanonicalPath::new("src/auth/login.rs"),
            make_node("auth", ArchLayer::Service, 2),
        );

        let mut clusters = BTreeMap::new();
        clusters.insert(
            ClusterId::new("auth"),
            Cluster {
                files: vec![CanonicalPath::new("src/auth/login.rs")],
                file_count: 1,
                internal_edges: 0,
                external_edges: 0,
                cohesion: 1.0,
            },
        );

        let result = suggest_placement(
            "mixed deps",
            &[
                CanonicalPath::new("src/auth/login.rs"),
                CanonicalPath::new("src/missing/gone.rs"),
            ],
            &[],
            &nodes,
            &clusters,
            &BTreeMap::new(),
        );

        assert_eq!(result.cluster, "auth");
        // Should note unresolved paths
        assert!(
            result.reasoning.iter().any(|r| r.contains("could not be resolved")),
            "Should mention unresolved paths in reasoning"
        );
    }

    // --- EC-PL-7: Conflicting layer constraints ---
    #[test]
    fn conflicting_layer_constraints() {
        let mut nodes = BTreeMap::new();
        nodes.insert(
            CanonicalPath::new("src/deep/a.rs"),
            make_node("deep", ArchLayer::Api, 3),
        );
        nodes.insert(
            CanonicalPath::new("src/shallow/b.rs"),
            make_node("shallow", ArchLayer::Data, 1),
        );

        let mut clusters = BTreeMap::new();
        clusters.insert(
            ClusterId::new("deep"),
            Cluster {
                files: vec![CanonicalPath::new("src/deep/a.rs")],
                file_count: 1,
                internal_edges: 0,
                external_edges: 0,
                cohesion: 1.0,
            },
        );
        clusters.insert(
            ClusterId::new("shallow"),
            Cluster {
                files: vec![CanonicalPath::new("src/shallow/b.rs")],
                file_count: 1,
                internal_edges: 0,
                external_edges: 0,
                cohesion: 1.0,
            },
        );

        let result = suggest_placement(
            "conflict test",
            &[CanonicalPath::new("src/deep/a.rs")],
            &[CanonicalPath::new("src/shallow/b.rs")],
            &nodes,
            &clusters,
            &BTreeMap::new(),
        );

        // Should not panic
        assert!(result.arch_depth > 0);
        // Should warn about layer conflict
        assert!(
            result.reasoning.iter().any(|r| r.contains("Layer conflict") || r.contains("layer conflict")),
            "Should mention layer conflict in reasoning: {:?}",
            result.reasoning
        );
    }

    // --- EC-PL-8: depended_by only ---
    #[test]
    fn depended_by_only() {
        let mut nodes = BTreeMap::new();
        nodes.insert(
            CanonicalPath::new("src/svc/handler.rs"),
            make_node("svc", ArchLayer::Service, 3),
        );

        let mut clusters = BTreeMap::new();
        clusters.insert(
            ClusterId::new("svc"),
            Cluster {
                files: vec![CanonicalPath::new("src/svc/handler.rs")],
                file_count: 1,
                internal_edges: 0,
                external_edges: 0,
                cohesion: 1.0,
            },
        );

        let result = suggest_placement(
            "stable base",
            &[],
            &[CanonicalPath::new("src/svc/handler.rs")],
            &nodes,
            &clusters,
            &BTreeMap::new(),
        );

        // Should place in same cluster as reverse dependency
        assert_eq!(result.cluster, "svc");
        // Should be more stable (lower depth) than what depends on it
        assert!(result.arch_depth <= 3);
    }

    // --- EC-PL-11: Very long description ---
    #[test]
    fn very_long_description_no_panic() {
        let long_desc = "a".repeat(10_000);
        let result = suggest_placement(
            &long_desc,
            &[],
            &[],
            &BTreeMap::new(),
            &BTreeMap::new(),
            &BTreeMap::new(),
        );

        // Should not panic and path should be reasonable length
        assert!(result.suggested_path.len() <= 200);
    }

    // --- EC-PL-12: Special characters in description ---
    #[test]
    fn special_characters_in_description() {
        let mut nodes = BTreeMap::new();
        nodes.insert(
            CanonicalPath::new("src/auth/login.rs"),
            make_node("auth", ArchLayer::Service, 2),
        );

        let mut clusters = BTreeMap::new();
        clusters.insert(
            ClusterId::new("auth"),
            Cluster {
                files: vec![CanonicalPath::new("src/auth/login.rs")],
                file_count: 1,
                internal_edges: 0,
                external_edges: 0,
                cohesion: 1.0,
            },
        );

        let result = suggest_placement(
            "user/authentication & session-management (v2)",
            &[CanonicalPath::new("src/auth/login.rs")],
            &[],
            &nodes,
            &clusters,
            &BTreeMap::new(),
        );

        // Extract the filename part (after last /)
        let filename = result
            .suggested_path
            .rsplit('/')
            .next()
            .unwrap_or(&result.suggested_path);

        // Should not contain special chars (except . for extension and _ for separators)
        for ch in filename.chars() {
            assert!(
                ch.is_ascii_alphanumeric() || ch == '_' || ch == '.' || ch == '-',
                "Unexpected character '{}' in suggested filename: {}",
                ch,
                filename
            );
        }
    }

    // --- EC-PL-14: All deps same layer ---
    #[test]
    fn all_deps_same_layer() {
        let mut nodes = BTreeMap::new();
        nodes.insert(
            CanonicalPath::new("src/svc/a.rs"),
            make_node("svc", ArchLayer::Service, 2),
        );
        nodes.insert(
            CanonicalPath::new("src/svc/b.rs"),
            make_node("svc", ArchLayer::Service, 2),
        );

        let mut clusters = BTreeMap::new();
        clusters.insert(
            ClusterId::new("svc"),
            Cluster {
                files: vec![
                    CanonicalPath::new("src/svc/a.rs"),
                    CanonicalPath::new("src/svc/b.rs"),
                ],
                file_count: 2,
                internal_edges: 0,
                external_edges: 0,
                cohesion: 1.0,
            },
        );

        let result = suggest_placement(
            "svc util",
            &[
                CanonicalPath::new("src/svc/a.rs"),
                CanonicalPath::new("src/svc/b.rs"),
            ],
            &[],
            &nodes,
            &clusters,
            &BTreeMap::new(),
        );

        // arch_depth should be at same or higher layer (>= 2)
        assert!(
            result.arch_depth >= 2,
            "arch_depth should be >= 2 (max dep depth), got {}",
            result.arch_depth
        );
    }

    // --- EC-PL-16: Large input ---
    #[test]
    fn large_input_no_perf_degradation() {
        let mut nodes = BTreeMap::new();
        let mut all_deps = Vec::new();

        for i in 0..300 {
            let cluster_name = format!("c{}", i % 10);
            let path = CanonicalPath::new(&format!("src/{}/f{}.rs", cluster_name, i));
            nodes.insert(path.clone(), make_node(&cluster_name, ArchLayer::Service, 2));
            all_deps.push(path);
        }

        let mut clusters = BTreeMap::new();
        for i in 0..10 {
            let name = format!("c{}", i);
            let files: Vec<CanonicalPath> = (0..30)
                .map(|j| CanonicalPath::new(&format!("src/{}/f{}.rs", name, i + j * 10)))
                .collect();
            clusters.insert(
                ClusterId::new(&name),
                Cluster {
                    file_count: files.len(),
                    files,
                    internal_edges: 0,
                    external_edges: 0,
                    cohesion: 1.0,
                },
            );
        }

        // Should complete without panic
        let result = suggest_placement(
            "large test",
            &all_deps,
            &[],
            &nodes,
            &clusters,
            &BTreeMap::new(),
        );

        assert!(!result.cluster.is_empty());
        assert!(!result.suggested_path.is_empty());
    }

    // --- EC-PL-9: Empty clusters map with resolved deps ---
    #[test]
    fn empty_clusters_with_resolved_deps() {
        let mut nodes = BTreeMap::new();
        nodes.insert(
            CanonicalPath::new("src/a.rs"),
            make_node("orphan", ArchLayer::Service, 1),
        );

        let result = suggest_placement(
            "orphan test",
            &[CanonicalPath::new("src/a.rs")],
            &[],
            &nodes,
            &BTreeMap::new(), // empty clusters
            &BTreeMap::new(),
        );

        // Should not panic; data quality depends on implementation logic
        assert!(!result.suggested_path.is_empty());
    }

    // --- EC-PL-10: Empty layers map ---
    #[test]
    fn empty_layers_map() {
        let mut nodes = BTreeMap::new();
        nodes.insert(
            CanonicalPath::new("src/a.rs"),
            make_node("core", ArchLayer::Service, 1),
        );

        let mut clusters = BTreeMap::new();
        clusters.insert(
            ClusterId::new("core"),
            Cluster {
                files: vec![CanonicalPath::new("src/a.rs")],
                file_count: 1,
                internal_edges: 0,
                external_edges: 0,
                cohesion: 1.0,
            },
        );

        let result = suggest_placement(
            "no layers",
            &[CanonicalPath::new("src/a.rs")],
            &[],
            &nodes,
            &clusters,
            &BTreeMap::new(), // empty layers
        );

        // Should still produce valid output
        assert_eq!(result.cluster, "core");
        assert!(!result.layer.is_empty());
    }

    #[test]
    fn circular_dependency_warning() {
        let mut nodes = BTreeMap::new();
        nodes.insert(
            CanonicalPath::new("src/a.rs"),
            make_node("core", ArchLayer::Service, 1),
        );

        let mut clusters = BTreeMap::new();
        clusters.insert(
            ClusterId::new("core"),
            Cluster {
                files: vec![CanonicalPath::new("src/a.rs")],
                file_count: 1,
                internal_edges: 0,
                external_edges: 0,
                cohesion: 1.0,
            },
        );

        let shared = CanonicalPath::new("src/a.rs");
        let result = suggest_placement(
            "circular test",
            &[shared.clone()],
            &[shared],
            &nodes,
            &clusters,
            &BTreeMap::new(),
        );

        assert!(result
            .reasoning
            .iter()
            .any(|r| r.contains("circular dependency")));
    }

    #[test]
    fn duplicate_inputs_deduplicated() {
        let mut nodes = BTreeMap::new();
        nodes.insert(
            CanonicalPath::new("src/a.rs"),
            make_node("core", ArchLayer::Util, 0),
        );

        let mut clusters = BTreeMap::new();
        clusters.insert(
            ClusterId::new("core"),
            Cluster {
                files: vec![CanonicalPath::new("src/a.rs")],
                file_count: 1,
                internal_edges: 0,
                external_edges: 0,
                cohesion: 1.0,
            },
        );

        // Pass same path twice in depends_on
        let result = suggest_placement(
            "dedup test",
            &[
                CanonicalPath::new("src/a.rs"),
                CanonicalPath::new("src/a.rs"),
            ],
            &[],
            &nodes,
            &clusters,
            &BTreeMap::new(),
        );

        // Should only count once in votes
        assert!(result.reasoning[0].contains("1/1"));
    }
}
