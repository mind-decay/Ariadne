use std::collections::BTreeMap;

use super::{ClusterOutput, GraphOutput};
use crate::model::*;

/// Convert GraphOutput (deserialized from graph.json) back to ProjectGraph.
impl TryFrom<GraphOutput> for ProjectGraph {
    type Error = String;

    fn try_from(output: GraphOutput) -> Result<Self, Self::Error> {
        if output.version != 1 {
            return Err(format!(
                "unsupported graph version: {} (expected 1)",
                output.version
            ));
        }

        let mut nodes = BTreeMap::new();
        for (path_str, node_output) in output.nodes {
            let path = CanonicalPath::new(path_str);
            let file_type = parse_file_type(&node_output.file_type)?;
            let layer = parse_arch_layer(&node_output.layer)?;
            nodes.insert(
                path,
                Node {
                    file_type,
                    layer,
                    arch_depth: node_output.arch_depth,
                    lines: node_output.lines,
                    hash: ContentHash::new(node_output.hash),
                    exports: node_output.exports.into_iter().map(Symbol::new).collect(),
                    cluster: ClusterId::new(node_output.cluster),
                },
            );
        }

        let mut edges = Vec::with_capacity(output.edges.len());
        for (from, to, edge_type_str, symbols) in output.edges {
            let edge_type = parse_edge_type(&edge_type_str)?;
            edges.push(Edge {
                from: CanonicalPath::new(from),
                to: CanonicalPath::new(to),
                edge_type,
                symbols: symbols.into_iter().map(Symbol::new).collect(),
            });
        }

        Ok(ProjectGraph { nodes, edges })
    }
}

/// Convert ClusterOutput back to ClusterMap.
impl TryFrom<ClusterOutput> for ClusterMap {
    type Error = String;

    fn try_from(output: ClusterOutput) -> Result<Self, Self::Error> {
        let mut clusters = BTreeMap::new();
        for (id_str, entry) in output.clusters {
            let id = ClusterId::new(id_str);
            clusters.insert(
                id,
                Cluster {
                    files: entry.files.into_iter().map(CanonicalPath::new).collect(),
                    file_count: entry.file_count,
                    internal_edges: entry.internal_edges,
                    external_edges: entry.external_edges,
                    cohesion: entry.cohesion,
                },
            );
        }
        Ok(ClusterMap { clusters })
    }
}

fn parse_file_type(s: &str) -> Result<FileType, String> {
    match s {
        "source" => Ok(FileType::Source),
        "test" => Ok(FileType::Test),
        "config" => Ok(FileType::Config),
        "style" => Ok(FileType::Style),
        "asset" => Ok(FileType::Asset),
        "type_def" => Ok(FileType::TypeDef),
        other => Err(format!("unknown file type: {}", other)),
    }
}

fn parse_arch_layer(s: &str) -> Result<ArchLayer, String> {
    match s {
        "api" => Ok(ArchLayer::Api),
        "service" => Ok(ArchLayer::Service),
        "data" => Ok(ArchLayer::Data),
        "util" => Ok(ArchLayer::Util),
        "component" => Ok(ArchLayer::Component),
        "hook" => Ok(ArchLayer::Hook),
        "config" => Ok(ArchLayer::Config),
        "unknown" => Ok(ArchLayer::Unknown),
        other => Err(format!("unknown arch layer: {}", other)),
    }
}

fn parse_edge_type(s: &str) -> Result<EdgeType, String> {
    match s {
        "imports" => Ok(EdgeType::Imports),
        "tests" => Ok(EdgeType::Tests),
        "re_exports" => Ok(EdgeType::ReExports),
        "type_imports" => Ok(EdgeType::TypeImports),
        other => Err(format!("unknown edge type: {}", other)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::serial::{ClusterEntryOutput, NodeOutput};

    #[test]
    fn round_trip_graph_output() {
        let graph_output = GraphOutput {
            version: 1,
            project_root: ".".to_string(),
            node_count: 2,
            edge_count: 1,
            nodes: BTreeMap::from([
                (
                    "src/a.ts".to_string(),
                    NodeOutput {
                        file_type: "source".to_string(),
                        layer: "util".to_string(),
                        arch_depth: 0,
                        lines: 10,
                        hash: "abc123".to_string(),
                        exports: vec!["foo".to_string()],
                        cluster: "src".to_string(),
                    },
                ),
                (
                    "src/b.ts".to_string(),
                    NodeOutput {
                        file_type: "source".to_string(),
                        layer: "service".to_string(),
                        arch_depth: 1,
                        lines: 20,
                        hash: "def456".to_string(),
                        exports: vec![],
                        cluster: "src".to_string(),
                    },
                ),
            ]),
            edges: vec![(
                "src/b.ts".to_string(),
                "src/a.ts".to_string(),
                "imports".to_string(),
                vec!["foo".to_string()],
            )],
            generated: None,
        };

        let graph: ProjectGraph = graph_output.try_into().unwrap();
        assert_eq!(graph.nodes.len(), 2);
        assert_eq!(graph.edges.len(), 1);
        assert_eq!(graph.edges[0].edge_type, EdgeType::Imports);
    }

    #[test]
    fn version_mismatch_rejected() {
        let output = GraphOutput {
            version: 99,
            project_root: ".".to_string(),
            node_count: 0,
            edge_count: 0,
            nodes: BTreeMap::new(),
            edges: vec![],
            generated: None,
        };
        let result: Result<ProjectGraph, _> = output.try_into();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("unsupported graph version"));
    }

    #[test]
    fn round_trip_cluster_output() {
        let cluster_output = ClusterOutput {
            clusters: BTreeMap::from([(
                "src/auth".to_string(),
                ClusterEntryOutput {
                    files: vec!["src/auth/login.ts".to_string()],
                    file_count: 1,
                    internal_edges: 0,
                    external_edges: 1,
                    cohesion: 0.0,
                },
            )]),
        };

        let cluster_map: ClusterMap = cluster_output.try_into().unwrap();
        assert_eq!(cluster_map.clusters.len(), 1);
    }
}
