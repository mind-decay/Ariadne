use std::collections::BTreeMap;

use crate::algo::is_architectural;
use crate::model::{CanonicalPath, FileType, ProjectGraph, StatsOutput, StatsSummary};

/// Compute StatsOutput from algorithm results.
pub fn compute_stats(
    graph: &ProjectGraph,
    centrality: &BTreeMap<CanonicalPath, f64>,
    sccs: &[Vec<CanonicalPath>],
    layers: &BTreeMap<CanonicalPath, u32>,
) -> StatsOutput {
    // Centrality: CanonicalPath → String keys
    let centrality_output: BTreeMap<String, f64> = centrality
        .iter()
        .map(|(k, &v)| (k.as_str().to_string(), v))
        .collect();

    // SCCs: convert to Vec<Vec<String>>, inner sorted, outer sorted by first
    let mut sccs_output: Vec<Vec<String>> = sccs
        .iter()
        .map(|scc| {
            let mut v: Vec<String> = scc.iter().map(|p| p.as_str().to_string()).collect();
            v.sort();
            v
        })
        .collect();
    sccs_output.sort_by(|a, b| a[0].cmp(&b[0]));

    // Layers: invert file→layer to layer→files.
    // Zero-pad keys for correct numeric ordering in BTreeMap (N12 fix).
    let mut layers_output: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (file, &layer) in layers {
        layers_output
            .entry(format!("{:05}", layer))
            .or_default()
            .push(file.as_str().to_string());
    }
    for files in layers_output.values_mut() {
        files.sort();
    }

    // Summary
    let max_depth = layers.values().copied().max().unwrap_or(0);

    // Degree metrics: count architectural edges per node
    let node_count = graph.nodes.len();
    let mut in_degree: BTreeMap<&CanonicalPath, u32> = BTreeMap::new();
    let mut out_degree: BTreeMap<&CanonicalPath, u32> = BTreeMap::new();
    for node in graph.nodes.keys() {
        in_degree.insert(node, 0);
        out_degree.insert(node, 0);
    }
    for edge in &graph.edges {
        if is_architectural(edge) {
            if let Some(count) = out_degree.get_mut(&edge.from) {
                *count += 1;
            }
            if let Some(count) = in_degree.get_mut(&edge.to) {
                *count += 1;
            }
        }
    }

    let avg_in_degree = if node_count > 0 {
        round4(in_degree.values().sum::<u32>() as f64 / node_count as f64)
    } else {
        0.0
    };
    let avg_out_degree = if node_count > 0 {
        round4(out_degree.values().sum::<u32>() as f64 / node_count as f64)
    } else {
        0.0
    };

    // Bottleneck files: centrality > 0.7, sorted by centrality desc then path
    let mut bottleneck_files: Vec<(String, f64)> = centrality
        .iter()
        .filter(|(_, &v)| v > 0.7)
        .map(|(k, &v)| (k.as_str().to_string(), v))
        .collect();
    bottleneck_files.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.0.cmp(&b.0))
    });
    let bottleneck_files: Vec<String> = bottleneck_files.into_iter().map(|(k, _)| k).collect();

    // Orphan files: source/test with zero in-degree AND zero out-degree (architectural)
    let mut orphan_files: Vec<String> = graph
        .nodes
        .iter()
        .filter(|(path, node)| {
            matches!(node.file_type, FileType::Source | FileType::Test)
                && in_degree.get(path).copied().unwrap_or(0) == 0
                && out_degree.get(path).copied().unwrap_or(0) == 0
        })
        .map(|(path, _)| path.as_str().to_string())
        .collect();
    orphan_files.sort();

    StatsOutput {
        version: 1,
        centrality: centrality_output,
        sccs: sccs_output,
        layers: layers_output,
        summary: StatsSummary {
            max_depth,
            avg_in_degree,
            avg_out_degree,
            bottleneck_files,
            orphan_files,
        },
    }
}

use super::round4;
