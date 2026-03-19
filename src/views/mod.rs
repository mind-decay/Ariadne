pub mod cluster;
pub mod impact;
pub mod index;

use std::fs;
use std::path::Path;

use crate::diagnostic::FatalError;
use crate::model::{ClusterMap, ProjectGraph, StatsOutput};

/// Generate L0 index + all L1 cluster views.
pub fn generate_all_views(
    graph: &ProjectGraph,
    clusters: &ClusterMap,
    stats: &StatsOutput,
    output_dir: &Path,
) -> Result<usize, FatalError> {
    let views_dir = output_dir;
    let clusters_dir = views_dir.join("clusters");

    fs::create_dir_all(&clusters_dir).map_err(|e| FatalError::OutputNotWritable {
        path: views_dir.to_path_buf(),
        reason: e.to_string(),
    })?;

    // L0: index.md
    let index_content = index::generate_index(graph, clusters, stats);
    fs::write(views_dir.join("index.md"), index_content).map_err(|e| {
        FatalError::OutputNotWritable {
            path: views_dir.join("index.md"),
            reason: e.to_string(),
        }
    })?;

    // L1: per-cluster views
    let mut cluster_count = 0;
    for cluster_id in clusters.clusters.keys() {
        let content = cluster::generate_cluster_view(cluster_id.as_str(), graph, stats);
        let filename = sanitize_filename(cluster_id.as_str());
        fs::write(clusters_dir.join(format!("{}.md", filename)), content).map_err(|e| {
            FatalError::OutputNotWritable {
                path: clusters_dir.join(format!("{}.md", filename)),
                reason: e.to_string(),
            }
        })?;
        cluster_count += 1;
    }

    Ok(cluster_count)
}

/// Sanitize cluster name for use as filename.
fn sanitize_filename(name: &str) -> String {
    name.replace(['/', '\\'], "_")
}
