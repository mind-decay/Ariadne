pub mod churn;
pub mod coupling;
pub mod git;
pub mod hotspot;
pub mod ownership;

use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;

use crate::algo;
use crate::diagnostic::DiagnosticCollector;
use crate::model::{CanonicalPath, ProjectGraph, TemporalState};

/// Run full temporal analysis on a git repository.
/// Returns None if git is unavailable or analysis fails.
pub fn analyze(
    project_root: &Path,
    graph: &ProjectGraph,
    collector: &DiagnosticCollector,
) -> Option<TemporalState> {
    // 1. Parse git log — returns None if git unavailable or not a repo
    let (commits, rename_map) = git::parse_git_log(project_root, collector)?;

    if commits.is_empty() {
        return Some(TemporalState {
            churn: BTreeMap::new(),
            co_changes: Vec::new(),
            ownership: BTreeMap::new(),
            hotspots: Vec::new(),
            shallow: false,
            commits_analyzed: 0,
            window_start: String::new(),
            window_end: String::new(),
        });
    }

    // 2. Determine analysis window
    // Git log outputs newest first, so first commit has the latest date
    let window_end = commits
        .first()
        .map(|c| extract_date(&c.date))
        .unwrap_or_default();
    let window_start = commits
        .last()
        .map(|c| extract_date(&c.date))
        .unwrap_or_default();

    let commits_analyzed = commits.len() as u32;

    // 3. Compute churn metrics
    let churn = churn::compute_churn(&commits, &rename_map, &window_end);

    // 4. Build graph edges list for structural link detection
    let graph_edges: Vec<(CanonicalPath, CanonicalPath)> = graph
        .edges
        .iter()
        .map(|e| (e.from.clone(), e.to.clone()))
        .collect();

    // 5. Compute co-change coupling
    let co_changes = coupling::compute_coupling(&commits, &rename_map, &graph_edges);

    // 6. Compute ownership
    let ownership = ownership::compute_ownership(&commits, &rename_map);

    // 7. Build file_lines map from ProjectGraph (Node.lines for each file)
    let file_lines: BTreeMap<CanonicalPath, u32> = graph
        .nodes
        .iter()
        .map(|(path, node)| (path.clone(), node.lines))
        .collect();

    // 8. Build blast_radius map from ProjectGraph (count dependents per file)
    let adj_index = algo::AdjacencyIndex::build(&graph.edges, algo::is_architectural);
    let blast_radius: BTreeMap<CanonicalPath, usize> = graph
        .nodes
        .keys()
        .filter_map(|path| {
            let radius = algo::blast_radius::blast_radius(graph, path, None, &adj_index);
            // blast_radius includes the file itself at distance 0, so subtract 1
            let dependents = if radius.len() > 1 {
                radius.len() - 1
            } else {
                0
            };
            if dependents > 0 {
                Some((path.clone(), dependents))
            } else {
                None
            }
        })
        .collect();

    // 9. Compute hotspots
    let hotspots = hotspot::compute_hotspots(&churn, &file_lines, &blast_radius);

    // 10. Determine shallow flag — check directly via git
    let shallow = is_shallow_repository(project_root);

    // 11. Assemble and return
    Some(TemporalState {
        churn,
        co_changes,
        ownership,
        hotspots,
        shallow,
        commits_analyzed,
        window_start,
        window_end,
    })
}

/// Extract the date prefix (YYYY-MM-DD) from an ISO 8601 datetime string.
fn extract_date(date: &str) -> String {
    date.get(..10).unwrap_or(date).to_string()
}

/// Check if the repository at the given path is a shallow clone.
fn is_shallow_repository(project_root: &Path) -> bool {
    let output = Command::new("git")
        .args([
            "-C",
            &project_root.to_string_lossy(),
            "rev-parse",
            "--is-shallow-repository",
        ])
        .output();

    match output {
        Ok(o) => String::from_utf8_lossy(&o.stdout).trim() == "true",
        Err(_) => false,
    }
}
