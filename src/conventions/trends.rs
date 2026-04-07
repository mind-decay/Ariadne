use std::collections::BTreeMap;

use crate::conventions::types::{TemporalTrend, Trend};
use crate::model::{FileType, ProjectGraph, TemporalState};

/// Minimum data points per population to report a trend.
const MIN_SAMPLE_SIZE: usize = 5;

/// Ratio threshold: new_proportion > GROWTH_RATIO * old_proportion → Growing.
const GROWTH_RATIO: f64 = 2.0;

/// Detect temporal trends in the codebase.
///
/// Two-population comparison: files with churn in last 90 days ("new") vs files
/// without recent churn ("old"). Minimum 5 data points per population.
/// Returns empty vec if no temporal data or insufficient commits.
pub fn temporal_trends(
    temporal: Option<&TemporalState>,
    graph: &ProjectGraph,
    scope: Option<&str>,
) -> Vec<TemporalTrend> {
    let temporal = match temporal {
        Some(t) if t.commits_analyzed >= 5 => t,
        _ => return Vec::new(),
    };

    let scope_prefix = scope.map(|s| {
        if s.ends_with('/') { s.to_string() } else { format!("{s}/") }
    });

    // Partition files into "new" (active in last 90d) and "old" (no recent activity)
    let mut new_exts: BTreeMap<&str, usize> = BTreeMap::new();
    let mut old_exts: BTreeMap<&str, usize> = BTreeMap::new();
    let mut new_count = 0usize;
    let mut old_count = 0usize;

    for (path, node) in &graph.nodes {
        if node.file_type != FileType::Source && node.file_type != FileType::TypeDef {
            continue;
        }
        if let Some(ref prefix) = scope_prefix {
            if !path.as_str().starts_with(prefix.as_str()) {
                continue;
            }
        }

        let ext = match path.extension() {
            Some(e) => e,
            None => continue,
        };

        let is_new = temporal.churn.get(path)
            .is_some_and(|c| c.commits_90d > 0);

        if is_new {
            *new_exts.entry(ext).or_default() += 1;
            new_count += 1;
        } else {
            *old_exts.entry(ext).or_default() += 1;
            old_count += 1;
        }
    }

    // Need minimum sample size in both populations
    if new_count < MIN_SAMPLE_SIZE || old_count < MIN_SAMPLE_SIZE {
        return Vec::new();
    }

    let mut trends = Vec::new();

    // Detect extension migration trends
    let all_exts: Vec<&str> = new_exts.keys().chain(old_exts.keys())
        .copied()
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();

    for ext in all_exts {
        let new_n = *new_exts.get(ext).unwrap_or(&0);
        let old_n = *old_exts.get(ext).unwrap_or(&0);

        // Skip if insufficient data for this extension
        if new_n + old_n < MIN_SAMPLE_SIZE {
            continue;
        }

        let new_prop = new_n as f64 / new_count as f64;
        let old_prop = old_n as f64 / old_count as f64;

        let trend = if old_prop < f64::EPSILON {
            // Extension only appears in new files
            if new_n >= MIN_SAMPLE_SIZE {
                Some(Trend::Growing)
            } else {
                None
            }
        } else if new_prop < f64::EPSILON {
            // Extension only in old files
            if old_n >= MIN_SAMPLE_SIZE {
                Some(Trend::Declining)
            } else {
                None
            }
        } else if new_prop > GROWTH_RATIO * old_prop {
            Some(Trend::Growing)
        } else if new_prop < old_prop / GROWTH_RATIO {
            Some(Trend::Declining)
        } else {
            None // Stable — not interesting enough to report
        };

        if let Some(trend) = trend {
            trends.push(TemporalTrend {
                pattern: format!(".{ext} files are {}", match trend {
                    Trend::Growing => "growing",
                    Trend::Declining => "declining",
                    Trend::Stable => "stable",
                }),
                trend,
                evidence: format!(
                    "{new_n}/{new_count} recently changed files use .{ext}, \
                     {old_n}/{old_count} older files use .{ext}"
                ),
            });
        }
    }

    // Sort by total evidence count (more data = more confident)
    trends.sort_by(|a, b| b.evidence.len().cmp(&a.evidence.len()));
    trends
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{
        ArchLayer, CanonicalPath, ChurnMetrics, ClusterId, ContentHash, Node,
    };

    fn make_source_node(ext_hint: &str) -> Node {
        let _ = ext_hint; // Just for naming in tests
        Node {
            file_type: FileType::Source,
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

    fn make_churn(commits_90d: u32) -> ChurnMetrics {
        ChurnMetrics {
            commits_30d: 0,
            commits_90d,
            commits_1y: commits_90d,
            lines_changed_30d: 0,
            lines_changed_90d: 0,
            authors_30d: 1,
            last_changed: None,
            top_authors: vec![],
        }
    }

    fn make_temporal(churn: BTreeMap<CanonicalPath, ChurnMetrics>, commits: u32) -> TemporalState {
        TemporalState {
            churn,
            co_changes: vec![],
            ownership: BTreeMap::new(),
            hotspots: vec![],
            shallow: false,
            commits_analyzed: commits,
            window_start: "2025-01-01".to_string(),
            window_end: "2026-04-01".to_string(),
        }
    }

    #[test]
    fn no_temporal_returns_empty() {
        let graph = ProjectGraph {
            nodes: BTreeMap::new(),
            edges: vec![],
        };
        let result = temporal_trends(None, &graph, None);
        assert!(result.is_empty());
    }

    #[test]
    fn insufficient_commits_returns_empty() {
        let temporal = make_temporal(BTreeMap::new(), 3);
        let graph = ProjectGraph {
            nodes: BTreeMap::new(),
            edges: vec![],
        };
        let result = temporal_trends(Some(&temporal), &graph, None);
        assert!(result.is_empty());
    }

    #[test]
    fn insufficient_population_returns_empty() {
        // Only 3 files — below MIN_SAMPLE_SIZE (5) for both populations
        let mut nodes = BTreeMap::new();
        let mut churn = BTreeMap::new();

        for i in 0..3 {
            let path = CanonicalPath::new(format!("src/file{i}.ts"));
            nodes.insert(path.clone(), make_source_node("ts"));
            churn.insert(path, make_churn(2));
        }

        let temporal = make_temporal(churn, 10);
        let graph = ProjectGraph { nodes, edges: vec![] };
        let result = temporal_trends(Some(&temporal), &graph, None);
        assert!(result.is_empty());
    }

    #[test]
    fn extension_migration_detected() {
        let mut nodes = BTreeMap::new();
        let mut churn = BTreeMap::new();

        // 8 "new" .tsx files (recently changed)
        for i in 0..8 {
            let path = CanonicalPath::new(format!("src/new{i}.tsx"));
            nodes.insert(path.clone(), make_source_node("tsx"));
            churn.insert(path, make_churn(5));
        }

        // 10 "old" .jsx files (no recent activity)
        for i in 0..10 {
            let path = CanonicalPath::new(format!("src/old{i}.jsx"));
            nodes.insert(path.clone(), make_source_node("jsx"));
            // No churn entry → old
        }

        let temporal = make_temporal(churn, 50);
        let graph = ProjectGraph { nodes, edges: vec![] };
        let result = temporal_trends(Some(&temporal), &graph, None);

        assert!(!result.is_empty());

        // .tsx should be Growing (100% new vs 0% old)
        let tsx_trend = result.iter().find(|t| t.pattern.contains(".tsx")).unwrap();
        assert_eq!(tsx_trend.trend, Trend::Growing);

        // .jsx should be Declining (0% new vs 100% old)
        let jsx_trend = result.iter().find(|t| t.pattern.contains(".jsx")).unwrap();
        assert_eq!(jsx_trend.trend, Trend::Declining);
    }

    #[test]
    fn evidence_string_is_readable() {
        let mut nodes = BTreeMap::new();
        let mut churn = BTreeMap::new();

        for i in 0..6 {
            let path = CanonicalPath::new(format!("src/new{i}.tsx"));
            nodes.insert(path.clone(), make_source_node("tsx"));
            churn.insert(path, make_churn(3));
        }
        for i in 0..6 {
            let path = CanonicalPath::new(format!("src/old{i}.js"));
            nodes.insert(path.clone(), make_source_node("js"));
        }

        let temporal = make_temporal(churn, 20);
        let graph = ProjectGraph { nodes, edges: vec![] };
        let result = temporal_trends(Some(&temporal), &graph, None);

        for trend in &result {
            // Evidence should contain counts
            assert!(trend.evidence.contains('/'), "evidence should contain counts: {}", trend.evidence);
        }
    }

    #[test]
    fn scope_filter_works() {
        let mut nodes = BTreeMap::new();
        let mut churn = BTreeMap::new();

        // 6 new files in src/auth/
        for i in 0..6 {
            let path = CanonicalPath::new(format!("src/auth/file{i}.ts"));
            nodes.insert(path.clone(), make_source_node("ts"));
            churn.insert(path, make_churn(5));
        }
        // 6 old files in src/utils/
        for i in 0..6 {
            let path = CanonicalPath::new(format!("src/utils/file{i}.ts"));
            nodes.insert(path.clone(), make_source_node("ts"));
        }

        let temporal = make_temporal(churn, 20);
        let graph = ProjectGraph { nodes, edges: vec![] };

        // All files are .ts → no extension migration when both populations exist
        // But scoped to src/auth: all 6 are new → no old population → empty
        let result = temporal_trends(Some(&temporal), &graph, Some("src/auth"));
        assert!(result.is_empty(), "scoped to only new files → insufficient old population");
    }
}
