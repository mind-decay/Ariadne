pub mod build;
pub mod read;
pub mod resolve;
pub mod walk;

use std::path::{Path, PathBuf};

use rayon::prelude::*;

use crate::cluster::assign_clusters;
use crate::diagnostic::{DiagnosticCollector, DiagnosticCounts, FatalError, Warning, WarningCode};
use crate::model::*;
use crate::parser::{ParserRegistry, RawExport, RawImport};
use crate::serial::{ClusterEntryOutput, ClusterOutput, GraphOutput, GraphSerializer, NodeOutput};

pub use read::{FileContent, FileReader, FileSkipReason, FsReader};
pub use walk::{FileEntry, FileWalker, FsWalker, WalkConfig};

/// Output of the parse stage.
#[derive(Clone, Debug)]
pub struct ParsedFile {
    pub path: CanonicalPath,
    pub imports: Vec<RawImport>,
    pub exports: Vec<RawExport>,
}

/// Result of a successful pipeline run.
#[derive(Debug)]
pub struct BuildOutput {
    pub graph_path: PathBuf,
    pub clusters_path: PathBuf,
    pub file_count: usize,
    pub edge_count: usize,
    pub cluster_count: usize,
    pub warnings: Vec<Warning>,
    pub counts: DiagnosticCounts,
}

/// The build pipeline — orchestrates walk → read → parse → resolve → cluster → serialize.
pub struct BuildPipeline {
    walker: Box<dyn FileWalker>,
    reader: Box<dyn FileReader>,
    registry: ParserRegistry,
    serializer: Box<dyn GraphSerializer>,
}

impl BuildPipeline {
    pub fn new(
        walker: Box<dyn FileWalker>,
        reader: Box<dyn FileReader>,
        registry: ParserRegistry,
        serializer: Box<dyn GraphSerializer>,
    ) -> Self {
        Self {
            walker,
            reader,
            registry,
            serializer,
        }
    }

    pub fn run(&self, root: &Path, config: WalkConfig) -> Result<BuildOutput, FatalError> {
        self.run_with_output(root, config, None)
    }

    pub fn run_with_output(&self, root: &Path, config: WalkConfig, output_dir: Option<&Path>) -> Result<BuildOutput, FatalError> {
        let diagnostics = DiagnosticCollector::new();
        let abs_root = std::fs::canonicalize(root).map_err(|_| FatalError::ProjectNotFound {
            path: root.to_path_buf(),
        })?;

        // Stage 1: Walk
        let entries = self.walker.walk(&abs_root, &config)?;

        // Stage 2: Read (with diagnostics)
        let mut file_contents: Vec<FileContent> = Vec::new();
        for entry in &entries {
            // Only read files with recognized parser extensions
            if self.registry.parser_for(&entry.extension).is_none() {
                continue;
            }

            match self.reader.read(entry, &abs_root, config.max_file_size) {
                Ok(content) => file_contents.push(content),
                Err(skip) => {
                    // Convert FileSkipReason to warning
                    let warning = skip_reason_to_warning(&skip);
                    diagnostics.warn(warning);
                }
            }
        }

        // E004: no parseable files
        if file_contents.is_empty() {
            return Err(FatalError::NoParseableFiles {
                path: root.to_path_buf(),
            });
        }

        // Sort for deterministic parallel processing
        file_contents.sort_by(|a, b| a.path.cmp(&b.path));

        // Stage 3: Parse (parallel via rayon on sorted list)
        let parsed_files: Vec<ParsedFile> = file_contents
            .par_iter()
            .filter_map(|fc| {
                let extension = fc.path.extension().unwrap_or("");
                let parser = self.registry.parser_for(extension)?;

                match self.registry.parse_source(&fc.bytes, parser) {
                    Ok(Some((_tree, imports, exports))) => Some(ParsedFile {
                        path: fc.path.clone(),
                        imports,
                        exports,
                    }),
                    Ok(None) => {
                        // Parse failed (>50% ERROR nodes)
                        diagnostics.warn(Warning {
                            code: WarningCode::W001ParseFailed,
                            path: fc.path.clone(),
                            message: "parse failed: too many errors".to_string(),
                            detail: None,
                        });
                        None
                    }
                    Err(msg) => {
                        diagnostics.warn(Warning {
                            code: WarningCode::W001ParseFailed,
                            path: fc.path.clone(),
                            message: msg,
                            detail: None,
                        });
                        None
                    }
                }
            })
            .collect();

        // Stage 4: Resolve + Build graph
        let mut graph =
            build::resolve_and_build(&parsed_files, &file_contents, &self.registry, &diagnostics);

        // Stage 5: Cluster
        let cluster_map = assign_clusters(&graph);

        // Apply cluster assignments to nodes
        for (cluster_id, cluster) in &cluster_map.clusters {
            for file_path in &cluster.files {
                if let Some(node) = graph.nodes.get_mut(file_path) {
                    node.cluster = cluster_id.clone();
                }
            }
        }

        // Stage 6: Convert to output model
        let graph_output = project_graph_to_output(&graph, &abs_root);
        let cluster_output = cluster_map_to_output(&cluster_map);

        // Stage 7: Serialize
        let output_dir = match output_dir {
            Some(dir) => dir.to_path_buf(),
            None => root.join(".ariadne").join("graph"),
        };
        self.serializer.write_graph(&graph_output, &output_dir)?;
        self.serializer
            .write_clusters(&cluster_output, &output_dir)?;

        // Drain diagnostics
        let report = diagnostics.drain();

        Ok(BuildOutput {
            graph_path: output_dir.join("graph.json"),
            clusters_path: output_dir.join("clusters.json"),
            file_count: graph.nodes.len(),
            edge_count: graph.edges.len(),
            cluster_count: cluster_map.clusters.len(),
            warnings: report.warnings,
            counts: report.counts,
        })
    }
}

/// Convert ProjectGraph to GraphOutput (D-022).
fn project_graph_to_output(graph: &ProjectGraph, project_root: &Path) -> GraphOutput {
    let mut nodes = std::collections::BTreeMap::new();
    for (path, node) in &graph.nodes {
        nodes.insert(
            path.as_str().to_string(),
            NodeOutput {
                file_type: node.file_type.as_str().to_string(),
                layer: node.layer.as_str().to_string(),
                arch_depth: node.arch_depth,
                lines: node.lines,
                hash: node.hash.as_str().to_string(),
                exports: node.exports.iter().map(|s| s.as_str().to_string()).collect(),
                cluster: node.cluster.as_str().to_string(),
            },
        );
    }

    let edges: Vec<(String, String, String, Vec<String>)> = graph
        .edges
        .iter()
        .map(|e| {
            (
                e.from.as_str().to_string(),
                e.to.as_str().to_string(),
                e.edge_type.as_str().to_string(),
                e.symbols.iter().map(|s| s.as_str().to_string()).collect(),
            )
        })
        .collect();

    GraphOutput {
        version: 1,
        project_root: project_root.to_string_lossy().to_string(),
        node_count: graph.nodes.len(),
        edge_count: graph.edges.len(),
        nodes,
        edges,
        generated: None,
    }
}

/// Convert ClusterMap to ClusterOutput.
fn cluster_map_to_output(cluster_map: &ClusterMap) -> ClusterOutput {
    let mut clusters = std::collections::BTreeMap::new();
    for (id, cluster) in &cluster_map.clusters {
        clusters.insert(
            id.as_str().to_string(),
            ClusterEntryOutput {
                files: cluster.files.iter().map(|p| p.as_str().to_string()).collect(),
                file_count: cluster.file_count,
                internal_edges: cluster.internal_edges,
                external_edges: cluster.external_edges,
                cohesion: cluster.cohesion,
            },
        );
    }
    ClusterOutput { clusters }
}

/// Convert a FileSkipReason into a Warning.
fn skip_reason_to_warning(reason: &FileSkipReason) -> Warning {
    match reason {
        FileSkipReason::ReadError { path, reason } => Warning {
            code: WarningCode::W002ReadFailed,
            path: CanonicalPath::new(path.to_string_lossy().to_string()),
            message: format!("cannot read: {}", reason),
            detail: None,
        },
        FileSkipReason::TooLarge { path, size } => Warning {
            code: WarningCode::W003FileTooLarge,
            path: CanonicalPath::new(path.to_string_lossy().to_string()),
            message: format!("file too large: {} bytes", size),
            detail: None,
        },
        FileSkipReason::BinaryFile { path } => Warning {
            code: WarningCode::W004BinaryFile,
            path: CanonicalPath::new(path.to_string_lossy().to_string()),
            message: "binary file".to_string(),
            detail: None,
        },
        FileSkipReason::EncodingError { path } => Warning {
            code: WarningCode::W009EncodingError,
            path: CanonicalPath::new(path.to_string_lossy().to_string()),
            message: "not valid UTF-8".to_string(),
            detail: None,
        },
    }
}
