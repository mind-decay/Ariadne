use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;

use crate::model::{EdgeType, ProjectGraph, StatsOutput};

/// Generate L1 cluster detail view.
pub fn generate_cluster_view(
    cluster_name: &str,
    graph: &ProjectGraph,
    stats: &StatsOutput,
) -> Result<String, std::fmt::Error> {
    let mut out = String::new();
    writeln!(out, "# Cluster: {}", cluster_name)?;
    writeln!(out)?;

    // Collect files in this cluster (BTreeSet for O(1) membership tests)
    let cluster_files: BTreeSet<&str> = graph
        .nodes
        .iter()
        .filter(|(_, node)| node.cluster.as_str() == cluster_name)
        .map(|(path, _)| path.as_str())
        .collect();

    if cluster_files.is_empty() {
        writeln!(out, "*No files in this cluster.*")?;
        return Ok(out);
    }

    // Compute in/out degree per file
    let mut in_degree: BTreeMap<&str, u32> = BTreeMap::new();
    let mut out_degree: BTreeMap<&str, u32> = BTreeMap::new();
    for edge in &graph.edges {
        if edge.edge_type.is_architectural() {
            *out_degree.entry(edge.from.as_str()).or_default() += 1;
            *in_degree.entry(edge.to.as_str()).or_default() += 1;
        }
    }

    // File table
    writeln!(out, "## Files")?;
    writeln!(out)?;
    writeln!(out, "| File | Type | Layer | In | Out | Centrality |")?;
    writeln!(out, "|------|------|------:|---:|----:|-----------:|")?;

    for &file in &cluster_files {
        if let Some(node) = graph.nodes.get(&crate::model::CanonicalPath::new(file)) {
            let bc = stats.centrality.get(file).copied().unwrap_or(0.0);
            let ind = in_degree.get(file).copied().unwrap_or(0);
            let outd = out_degree.get(file).copied().unwrap_or(0);
            writeln!(
                out,
                "| `{}` | {} | {} | {} | {} | {:.4} |",
                file,
                node.file_type.as_str(),
                node.arch_depth,
                ind,
                outd,
                bc
            )?;
        }
    }
    writeln!(out)?;

    // Internal dependencies
    let internal_edges: Vec<_> = graph
        .edges
        .iter()
        .filter(|e| {
            e.edge_type.is_architectural()
                && cluster_files.contains(&e.from.as_str())
                && cluster_files.contains(&e.to.as_str())
        })
        .collect();

    if !internal_edges.is_empty() {
        writeln!(out, "## Internal Dependencies")?;
        writeln!(out)?;
        for edge in &internal_edges {
            writeln!(
                out,
                "- `{}` → `{}` ({})",
                edge.from.as_str(),
                edge.to.as_str(),
                edge.edge_type.as_str()
            )?;
        }
        writeln!(out)?;
    }

    // External deps (outgoing from this cluster)
    let external_out: Vec<_> = graph
        .edges
        .iter()
        .filter(|e| {
            e.edge_type.is_architectural()
                && cluster_files.contains(&e.from.as_str())
                && !cluster_files.contains(&e.to.as_str())
        })
        .collect();

    if !external_out.is_empty() {
        writeln!(out, "## External Dependencies")?;
        writeln!(out)?;
        for edge in &external_out {
            writeln!(
                out,
                "- `{}` → `{}` ({})",
                edge.from.as_str(),
                edge.to.as_str(),
                edge.edge_type.as_str()
            )?;
        }
        writeln!(out)?;
    }

    // External dependents (incoming to this cluster)
    let external_in: Vec<_> = graph
        .edges
        .iter()
        .filter(|e| {
            e.edge_type.is_architectural()
                && !cluster_files.contains(&e.from.as_str())
                && cluster_files.contains(&e.to.as_str())
        })
        .collect();

    if !external_in.is_empty() {
        writeln!(out, "## External Dependents")?;
        writeln!(out)?;
        for edge in &external_in {
            writeln!(
                out,
                "- `{}` ← `{}` ({})",
                edge.to.as_str(),
                edge.from.as_str(),
                edge.edge_type.as_str()
            )?;
        }
        writeln!(out)?;
    }

    // Tests section
    let test_edges: Vec<_> = graph
        .edges
        .iter()
        .filter(|e| {
            e.edge_type == EdgeType::Tests
                && (cluster_files.contains(&e.from.as_str())
                    || cluster_files.contains(&e.to.as_str()))
        })
        .collect();

    if !test_edges.is_empty() {
        writeln!(out, "## Tests")?;
        writeln!(out)?;
        for edge in &test_edges {
            writeln!(
                out,
                "- `{}` tests `{}`",
                edge.from.as_str(),
                edge.to.as_str()
            )?;
        }
        writeln!(out)?;
    }

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::*;

    fn make_graph_with_cluster() -> (ProjectGraph, StatsOutput) {
        let mut nodes = BTreeMap::new();
        nodes.insert(
            CanonicalPath::new("src/a.ts"),
            Node {
                file_type: FileType::Source,
                layer: ArchLayer::Service,
                fsd_layer: None,
                arch_depth: 1,
                lines: 100,
                hash: ContentHash::new("aaa".to_string()),
                exports: vec![],
                cluster: ClusterId::new("src"),
                    symbols: Vec::new(),
            },
        );
        nodes.insert(
            CanonicalPath::new("src/b.ts"),
            Node {
                file_type: FileType::Source,
                layer: ArchLayer::Util,
                fsd_layer: None,
                arch_depth: 0,
                lines: 50,
                hash: ContentHash::new("bbb".to_string()),
                exports: vec![],
                cluster: ClusterId::new("src"),
                    symbols: Vec::new(),
            },
        );
        let edges = vec![Edge {
            from: CanonicalPath::new("src/a.ts"),
            to: CanonicalPath::new("src/b.ts"),
            edge_type: EdgeType::Imports,
            symbols: vec![],
        }];
        let graph = ProjectGraph { nodes, edges };

        let mut centrality = BTreeMap::new();
        centrality.insert("src/a.ts".to_string(), 0.5);
        let stats = StatsOutput {
            version: 1,
            centrality,
            sccs: vec![],
            layers: BTreeMap::new(),
            summary: StatsSummary {
                max_depth: 1,
                avg_in_degree: 0.5,
                avg_out_degree: 0.5,
                bottleneck_files: vec![],
                orphan_files: vec![],
            },
        };
        (graph, stats)
    }

    #[test]
    fn empty_cluster_message() {
        let graph = ProjectGraph {
            nodes: BTreeMap::new(),
            edges: vec![],
        };
        let stats = StatsOutput {
            version: 1,
            centrality: BTreeMap::new(),
            sccs: vec![],
            layers: BTreeMap::new(),
            summary: StatsSummary {
                max_depth: 0,
                avg_in_degree: 0.0,
                avg_out_degree: 0.0,
                bottleneck_files: vec![],
                orphan_files: vec![],
            },
        };
        let md = generate_cluster_view("nonexistent", &graph, &stats).unwrap();
        assert!(md.contains("# Cluster: nonexistent"));
        assert!(md.contains("*No files in this cluster.*"));
    }

    #[test]
    fn cluster_shows_files_table() {
        let (graph, stats) = make_graph_with_cluster();
        let md = generate_cluster_view("src", &graph, &stats).unwrap();
        assert!(md.contains("# Cluster: src"));
        assert!(md.contains("## Files"));
        assert!(md.contains("src/a.ts"));
        assert!(md.contains("src/b.ts"));
    }

    #[test]
    fn cluster_shows_internal_dependencies() {
        let (graph, stats) = make_graph_with_cluster();
        let md = generate_cluster_view("src", &graph, &stats).unwrap();
        assert!(md.contains("## Internal Dependencies"));
    }
}
