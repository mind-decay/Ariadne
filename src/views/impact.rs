use std::collections::BTreeMap;
use std::fmt::Write;

use crate::model::{CanonicalPath, ProjectGraph, SubgraphResult};

/// Generate L2 blast radius report as markdown.
pub fn generate_blast_radius_view(
    file: &str,
    blast_result: &BTreeMap<CanonicalPath, u32>,
    graph: &ProjectGraph,
) -> String {
    let mut out = String::new();
    writeln!(out, "# Blast Radius: `{}`", file).unwrap();
    writeln!(out).unwrap();
    writeln!(out, "**Affected files:** {}", blast_result.len()).unwrap();
    writeln!(out).unwrap();

    // Group by distance
    let mut by_distance: BTreeMap<u32, Vec<&CanonicalPath>> = BTreeMap::new();
    for (path, &dist) in blast_result {
        by_distance.entry(dist).or_default().push(path);
    }

    for (distance, files) in &by_distance {
        if *distance == 0 {
            writeln!(out, "## Source (distance 0)").unwrap();
        } else {
            writeln!(out, "## Distance {}", distance).unwrap();
        }
        writeln!(out).unwrap();
        for f in files {
            let node_info = graph
                .nodes
                .get(*f)
                .map(|n| format!(" ({}, {})", n.file_type.as_str(), n.layer.as_str()))
                .unwrap_or_default();
            writeln!(out, "- `{}`{}", f.as_str(), node_info).unwrap();
        }
        writeln!(out).unwrap();
    }

    out
}

/// Generate L2 subgraph view as markdown.
pub fn generate_subgraph_view(subgraph: &SubgraphResult) -> String {
    let mut out = String::new();
    let centers: Vec<&str> = subgraph.center_files.iter().map(|p| p.as_str()).collect();
    writeln!(out, "# Subgraph: {}", centers.join(", ")).unwrap();
    writeln!(out).unwrap();
    writeln!(
        out,
        "**Depth:** {} | **Nodes:** {} | **Edges:** {}",
        subgraph.depth,
        subgraph.nodes.len(),
        subgraph.edges.len()
    )
    .unwrap();
    writeln!(out).unwrap();

    // Node table
    writeln!(out, "## Files").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "| File | Type | Layer | Cluster |").unwrap();
    writeln!(out, "|------|------|------:|---------|").unwrap();
    for (path, node) in &subgraph.nodes {
        writeln!(
            out,
            "| `{}` | {} | {} | {} |",
            path.as_str(),
            node.file_type.as_str(),
            node.arch_depth,
            node.cluster.as_str()
        )
        .unwrap();
    }
    writeln!(out).unwrap();

    // Edges
    if !subgraph.edges.is_empty() {
        writeln!(out, "## Edges").unwrap();
        writeln!(out).unwrap();
        for edge in &subgraph.edges {
            writeln!(
                out,
                "- `{}` → `{}` ({})",
                edge.from.as_str(),
                edge.to.as_str(),
                edge.edge_type.as_str()
            )
            .unwrap();
        }
        writeln!(out).unwrap();
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::*;

    #[test]
    fn blast_radius_empty() {
        let graph = ProjectGraph {
            nodes: BTreeMap::new(),
            edges: vec![],
        };
        let blast = BTreeMap::new();
        let md = generate_blast_radius_view("src/x.ts", &blast, &graph);
        assert!(md.contains("# Blast Radius: `src/x.ts`"));
        assert!(md.contains("**Affected files:** 0"));
    }

    #[test]
    fn blast_radius_groups_by_distance() {
        let mut nodes = BTreeMap::new();
        nodes.insert(
            CanonicalPath::new("src/a.ts"),
            Node {
                file_type: FileType::Source,
                layer: ArchLayer::Service,
                fsd_layer: None,
                arch_depth: 0,
                lines: 10,
                hash: ContentHash::new("a".to_string()),
                exports: vec![],
                cluster: ClusterId::new("src"),
            },
        );
        nodes.insert(
            CanonicalPath::new("src/b.ts"),
            Node {
                file_type: FileType::Source,
                layer: ArchLayer::Util,
                fsd_layer: None,
                arch_depth: 0,
                lines: 10,
                hash: ContentHash::new("b".to_string()),
                exports: vec![],
                cluster: ClusterId::new("src"),
            },
        );
        let graph = ProjectGraph {
            nodes,
            edges: vec![],
        };
        let mut blast = BTreeMap::new();
        blast.insert(CanonicalPath::new("src/a.ts"), 0);
        blast.insert(CanonicalPath::new("src/b.ts"), 1);
        let md = generate_blast_radius_view("src/a.ts", &blast, &graph);
        assert!(md.contains("## Source (distance 0)"));
        assert!(md.contains("## Distance 1"));
        assert!(md.contains("src/b.ts"));
    }

    #[test]
    fn blast_radius_special_chars() {
        let graph = ProjectGraph {
            nodes: BTreeMap::new(),
            edges: vec![],
        };
        let mut blast = BTreeMap::new();
        blast.insert(CanonicalPath::new("src/special&file.ts"), 0);
        let md = generate_blast_radius_view("src/special&file.ts", &blast, &graph);
        assert!(md.contains("special&file.ts"));
    }

    #[test]
    fn subgraph_view_empty() {
        let subgraph = SubgraphResult {
            nodes: BTreeMap::new(),
            edges: vec![],
            center_files: vec![CanonicalPath::new("src/x.ts")],
            depth: 2,
        };
        let md = generate_subgraph_view(&subgraph);
        assert!(md.contains("# Subgraph: src/x.ts"));
        assert!(md.contains("**Depth:** 2"));
        assert!(md.contains("**Nodes:** 0"));
    }

    #[test]
    fn subgraph_view_with_nodes_and_edges() {
        let mut nodes = BTreeMap::new();
        nodes.insert(
            CanonicalPath::new("src/a.ts"),
            Node {
                file_type: FileType::Source,
                layer: ArchLayer::Service,
                fsd_layer: None,
                arch_depth: 1,
                lines: 100,
                hash: ContentHash::new("a".to_string()),
                exports: vec![],
                cluster: ClusterId::new("src"),
            },
        );
        let edges = vec![Edge {
            from: CanonicalPath::new("src/a.ts"),
            to: CanonicalPath::new("src/b.ts"),
            edge_type: EdgeType::Imports,
            symbols: vec![],
        }];
        let subgraph = SubgraphResult {
            nodes,
            edges,
            center_files: vec![CanonicalPath::new("src/a.ts")],
            depth: 1,
        };
        let md = generate_subgraph_view(&subgraph);
        assert!(md.contains("## Files"));
        assert!(md.contains("src/a.ts"));
        assert!(md.contains("## Edges"));
    }
}
