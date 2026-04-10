pub mod build;
pub mod read;
pub mod resolve;
pub mod walk;

use std::path::{Path, PathBuf};
use std::time::Instant;

use rayon::prelude::*;

use crate::algo;
use crate::cluster::assign_clusters;
use crate::detect::{detect_fsd_project, detect_workspace, is_case_insensitive};
use crate::diagnostic::{DiagnosticCollector, DiagnosticCounts, FatalError, Warning, WarningCode};
use crate::model::*;
use crate::model::semantic::Boundary;
use crate::model::symbol::SymbolDef;
use crate::parser::{ParseOutcome, ParserRegistry, RawExport, RawImport};
use crate::parser::config::discover_config;
use crate::semantic;
use crate::serial::{
    self, ClusterEntryOutput, ClusterOutput, GraphOutput, GraphReader, GraphSerializer,
    NodeOutput, RawImportOutput,
};

pub use read::{FileContent, FileReader, FileSkipReason, FsReader};
pub use walk::{FileEntry, FileWalker, FsWalker, WalkConfig, WalkResult};

/// Output of the parse stage.
#[derive(Clone, Debug)]
pub struct ParsedFile {
    pub path: CanonicalPath,
    pub imports: Vec<RawImport>,
    pub exports: Vec<RawExport>,
    pub symbols: Vec<SymbolDef>,
    pub boundaries: Vec<Boundary>,
}

/// Result of a successful pipeline run.
#[derive(Debug)]
pub struct BuildOutput {
    pub graph_path: PathBuf,
    pub clusters_path: PathBuf,
    pub stats_path: PathBuf,
    pub file_count: usize,
    pub edge_count: usize,
    pub cluster_count: usize,
    pub warnings: Vec<Warning>,
    pub counts: DiagnosticCounts,
}

/// Options that control a build or update run.
#[derive(Clone, Debug, Default)]
pub struct BuildOptions<'a> {
    pub output_dir: Option<&'a Path>,
    pub timestamp: bool,
    pub verbose: bool,
    pub no_louvain: bool,
    pub rust_crate_name: Option<&'a str>,
    pub louvain_resolution: Option<f64>,
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

    /// Return the set of file extensions this pipeline's registry can parse.
    /// Used by the MCP server to filter file watcher events without
    /// constructing a second `ParserRegistry`.
    pub fn supported_extensions(&self) -> Vec<&str> {
        self.registry.supported_extensions()
    }

    /// Re-parse imports from source bytes for a given file extension.
    /// Used by the freshness engine for lightweight import change detection.
    /// Preserves the dependency boundary: mcp/ -> pipeline/ -> parser/.
    pub fn reparse_imports(
        &self,
        extension: &str,
        source: &[u8],
        path: &crate::model::CanonicalPath,
    ) -> Option<Vec<crate::parser::RawImport>> {
        self.registry.reparse_imports(extension, source, path)
    }

    /// Apply symbol limits (W020 overflow guard) and sort for determinism.
    /// Symbols are already extracted during parse_source (D-077: no re-parsing).
    fn apply_symbol_limits(
        &self,
        mut symbols: Vec<SymbolDef>,
        fc: &FileContent,
        diagnostics: &DiagnosticCollector,
    ) -> Vec<SymbolDef> {
        // W020: overflow guard — truncate to 1000
        if symbols.len() > 1000 {
            diagnostics.warn(Warning {
                code: WarningCode::W020SymbolOverflow,
                path: fc.path.clone(),
                message: format!(
                    "symbol count {} exceeds limit 1000, truncating",
                    symbols.len()
                ),
                detail: None,
            });
            symbols.truncate(1000);
        }

        // Sort for determinism
        symbols.sort();
        symbols
    }

    pub fn run(&self, root: &Path, config: WalkConfig) -> Result<BuildOutput, FatalError> {
        self.run_with_options(root, config, &BuildOptions::default())
    }

    pub fn run_with_options(
        &self,
        root: &Path,
        config: WalkConfig,
        opts: &BuildOptions,
    ) -> Result<BuildOutput, FatalError> {
        let output_dir = opts.output_dir;
        let timestamp = opts.timestamp;
        let verbose = opts.verbose;
        let no_louvain = opts.no_louvain;
        let rust_crate_name = opts.rust_crate_name;
        let louvain_resolution = opts.louvain_resolution;
        let diagnostics = DiagnosticCollector::new();
        let abs_root = std::fs::canonicalize(root).map_err(|_| FatalError::ProjectNotFound {
            path: root.to_path_buf(),
        })?;

        let total_start = Instant::now();

        // Stage 1: Walk
        let walk_start = Instant::now();
        let walk_result = self.walker.walk(&abs_root, &config)?;
        let entries = walk_result.entries;
        // Forward walk-level warnings to DiagnosticCollector (S1/S2 fix)
        for w in walk_result.warnings {
            diagnostics.warn(w);
        }
        if verbose {
            eprintln!(
                "[walk]      {:>6}ms  {} files found",
                walk_start.elapsed().as_millis(),
                entries.len()
            );
        }

        // Config discovery (D-120): build FileSet from walked entries, discover configs,
        // construct config-aware registry for resolvers that support it.
        let walked_file_set: FileSet = entries
            .iter()
            .map(|e| {
                let rel = e.path.strip_prefix(&abs_root).unwrap_or(&e.path);
                CanonicalPath::new(rel.to_string_lossy())
            })
            .collect();
        let project_config = discover_config(&abs_root, &walked_file_set, &diagnostics);
        let registry = ParserRegistry::with_project_config(&project_config);

        // Stage 2: Read (with diagnostics)
        let read_start = Instant::now();
        let mut file_contents: Vec<FileContent> = Vec::new();
        let mut read_skipped: usize = 0;
        for entry in &entries {
            // Only read files with recognized parser extensions
            if registry.parser_for(&entry.extension).is_none() {
                continue;
            }

            match self.reader.read(entry, &abs_root, config.max_file_size) {
                Ok(content) => file_contents.push(content),
                Err(skip) => {
                    read_skipped += 1;
                    // Convert FileSkipReason to warning
                    let warning = skip_reason_to_warning(&skip);
                    diagnostics.warn(warning);
                }
            }
        }
        if verbose {
            eprintln!(
                "[read+hash] {:>6}ms  {} files read ({} skipped)",
                read_start.elapsed().as_millis(),
                file_contents.len(),
                read_skipped
            );
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
        let parse_start = Instant::now();
        let file_count_before_parse = file_contents.len();
        let parsed_files: Vec<ParsedFile> = file_contents
            .par_iter()
            .filter_map(|fc| {
                let extension = fc.path.extension().unwrap_or("");
                let parser = registry.parser_for(extension)?;

                match registry.parse_source(&fc.bytes, parser, extension, &fc.path) {
                    Ok(ParseOutcome::Ok(imports, exports, symbols, boundaries)) => {
                        let symbols = self.apply_symbol_limits(symbols, fc, &diagnostics);
                        Some(ParsedFile {
                            path: fc.path.clone(),
                            imports,
                            exports,
                            symbols,
                            boundaries,
                        })
                    }
                    Ok(ParseOutcome::Partial(imports, exports, symbols, boundaries)) => {
                        // Partial parse — extract what we can, emit W007
                        diagnostics.warn(Warning {
                            code: WarningCode::W007PartialParse,
                            path: fc.path.clone(),
                            message: "partial parse: some syntax errors".to_string(),
                            detail: None,
                        });
                        let symbols = self.apply_symbol_limits(symbols, fc, &diagnostics);
                        Some(ParsedFile {
                            path: fc.path.clone(),
                            imports,
                            exports,
                            symbols,
                            boundaries,
                        })
                    }
                    Ok(ParseOutcome::Failed) => {
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
        if verbose {
            let parse_warnings = file_count_before_parse - parsed_files.len();
            eprintln!(
                "[parse]     {:>6}ms  {} files parsed ({} warnings)",
                parse_start.elapsed().as_millis(),
                parsed_files.len(),
                parse_warnings
            );
        }

        // Boundary aggregation: collect boundaries from parsed files for semantic analysis
        let file_boundaries: std::collections::BTreeMap<CanonicalPath, Vec<Boundary>> =
            parsed_files
                .iter()
                .filter(|pf| !pf.boundaries.is_empty())
                .map(|pf| (pf.path.clone(), pf.boundaries.clone()))
                .collect();

        // Stage 4: Resolve + Build graph
        let resolve_start = Instant::now();
        // Detect workspace for workspace-aware import resolution
        let workspace_info = detect_workspace(&abs_root, &diagnostics);
        let workspace_relative = workspace_info.as_ref().map(|ws| ws.relativize(&abs_root));
        // Detect case sensitivity once per build (F3 fix)
        let case_insensitive = is_case_insensitive(&abs_root);
        // Detect FSD project structure for layer classification
        let all_paths: Vec<CanonicalPath> = file_contents.iter().map(|fc| fc.path.clone()).collect();
        let is_fsd = detect_fsd_project(&all_paths);
        let resolve_opts = build::ResolveOptions {
            workspace: workspace_relative.as_ref(),
            case_insensitive,
            is_fsd,
            rust_crate_name,
        };
        let mut graph = build::resolve_and_build(
            &parsed_files,
            &file_contents,
            &registry,
            &diagnostics,
            &resolve_opts,
        );
        if verbose {
            eprintln!(
                "[resolve]   {:>6}ms  {} edges created",
                resolve_start.elapsed().as_millis(),
                graph.edges.len()
            );
        }

        // Stage 5: Cluster
        let cluster_start = Instant::now();
        let mut cluster_map = assign_clusters(&graph);

        // Apply cluster assignments to nodes
        for (cluster_id, cluster) in &cluster_map.clusters {
            for file_path in &cluster.files {
                if let Some(node) = graph.nodes.get_mut(file_path) {
                    node.cluster = cluster_id.clone();
                }
            }
        }
        if verbose {
            eprintln!(
                "[cluster]   {:>6}ms  {} clusters",
                cluster_start.elapsed().as_millis(),
                cluster_map.clusters.len()
            );
        }

        // Stage 5b: Louvain clustering (refines directory-based clusters)
        if !no_louvain {
            let louvain_start = Instant::now();
            let dir_cluster_count = cluster_map.clusters.len();
            let gamma = louvain_resolution.unwrap_or(1.0);
            let (refined, converged) =
                algo::louvain::louvain_clustering_with_resolution(&graph, &cluster_map, gamma);
            if !converged {
                diagnostics.warn(Warning {
                    code: WarningCode::W012AlgorithmFailed,
                    path: CanonicalPath::new("<louvain>"),
                    message: "Louvain clustering did not converge within iteration limit"
                        .to_string(),
                    detail: None,
                });
            }

            // Guard: if Louvain reduced clusters below 50% of directory-based count,
            // the codebase is too tightly connected for Louvain to be useful.
            // Keep directory-based clusters which preserve navigable structure.
            let louvain_count = refined.clusters.len();
            if louvain_count * 2 >= dir_cluster_count {
                cluster_map = refined;
            } else if verbose {
                eprintln!(
                    "[louvain]   Louvain reduced {} → {} clusters (below 50%), keeping directory-based",
                    dir_cluster_count, louvain_count,
                );
            }

            // Re-apply cluster assignments to nodes after Louvain
            for (cluster_id, cluster) in &cluster_map.clusters {
                for file_path in &cluster.files {
                    if let Some(node) = graph.nodes.get_mut(file_path) {
                        node.cluster = cluster_id.clone();
                    }
                }
            }

            if verbose {
                eprintln!(
                    "[louvain]   {:>6}ms  {} clusters (was {} directory-based)",
                    louvain_start.elapsed().as_millis(),
                    cluster_map.clusters.len(),
                    dir_cluster_count,
                );
            }
        }

        // Stage 6: Run algorithms (before serialization so arch_depth is correct in graph.json)
        let algo_start = Instant::now();
        let index = algo::AdjacencyIndex::build(&graph.edges, algo::is_architectural);
        let sccs = algo::scc::find_sccs(&graph, &index);
        let layers = algo::topo_sort::topological_layers(&graph, &sccs, &index);

        // Apply arch_depth from topological layers to graph nodes
        for (path, &layer) in &layers {
            if let Some(node) = graph.nodes.get_mut(path) {
                node.arch_depth = layer;
            }
        }

        let centrality = algo::centrality::betweenness_centrality(&graph, &index);
        let stats = algo::stats::compute_stats(&graph, &centrality, &sccs, &layers);
        if verbose {
            eprintln!(
                "[algorithms]{:>6}ms  {} SCCs, {} layers, {} centrality scores",
                algo_start.elapsed().as_millis(),
                sccs.len(),
                layers.values().copied().max().unwrap_or(0) + 1,
                centrality.len(),
            );
        }

        // Stage 7: Convert to output model
        // Use the original CLI path (not abs_root) for portability — D-006, D-015
        let mut graph_output = project_graph_to_output(&graph, root);
        if timestamp {
            graph_output.generated = Some(format_utc_timestamp());
        }
        let cluster_output = cluster_map_to_output(&cluster_map);

        // Stage 8: Serialize
        let ser_start = Instant::now();
        let output_dir = match output_dir {
            Some(dir) => dir.to_path_buf(),
            None => root.join(".ariadne").join("graph"),
        };
        self.serializer.write_graph(&graph_output, &output_dir)?;
        self.serializer
            .write_clusters(&cluster_output, &output_dir)?;
        self.serializer.write_stats(&stats, &output_dir)?;
        // Serialize raw imports for freshness engine (D-054)
        let raw_imports_output: std::collections::BTreeMap<String, Vec<RawImportOutput>> =
            parsed_files
                .iter()
                .map(|pf| {
                    let key = pf.path.as_str().to_string();
                    let imports = pf
                        .imports
                        .iter()
                        .map(|ri| RawImportOutput {
                            path: ri.path.clone(),
                            symbols: ri.symbols.clone(),
                            is_type_only: ri.is_type_only,
                        })
                        .collect();
                    (key, imports)
                })
                .collect();
        self.serializer
            .write_raw_imports(&raw_imports_output, &output_dir)?;
        // Serialize boundaries if any were found (D-103, D-104)
        if !file_boundaries.is_empty() {
            let semantic_state = semantic::analyze(file_boundaries);
            let boundary_output = serial::semantic_state_to_boundary_output(&semantic_state);
            self.serializer
                .write_boundaries(&boundary_output, &output_dir)?;
        }
        if verbose {
            let graph_size = std::fs::metadata(output_dir.join("graph.json"))
                .map(|m| m.len())
                .unwrap_or(0);
            let cluster_size = std::fs::metadata(output_dir.join("clusters.json"))
                .map(|m| m.len())
                .unwrap_or(0);
            let stats_size = std::fs::metadata(output_dir.join("stats.json"))
                .map(|m| m.len())
                .unwrap_or(0);
            eprintln!(
                "[serialize] {:>6}ms  graph.json ({}) + clusters.json ({}) + stats.json ({})",
                ser_start.elapsed().as_millis(),
                format_size(graph_size),
                format_size(cluster_size),
                format_size(stats_size)
            );
        }

        if verbose {
            eprintln!("[total]     {:>6}ms", total_start.elapsed().as_millis());
        }

        // Drain diagnostics
        let report = diagnostics.drain();

        Ok(BuildOutput {
            graph_path: output_dir.join("graph.json"),
            clusters_path: output_dir.join("clusters.json"),
            stats_path: output_dir.join("stats.json"),
            file_count: graph.nodes.len(),
            edge_count: graph.edges.len(),
            cluster_count: cluster_map.clusters.len(),
            warnings: report.warnings,
            counts: report.counts,
        })
    }

    /// Incremental update via delta computation (D9).
    /// Loads existing graph, detects changes via content hash comparison.
    /// Falls back to full build on errors or >5% changes.
    /// When below threshold with actual changes, does a full rebuild
    /// (algorithms are fast; correctness over optimization).
    /// Incremental re-parse of only changed files is deferred to Phase 3.
    /// Views are NOT regenerated (per spec D9).
    pub fn update(
        &self,
        root: &Path,
        config: WalkConfig,
        reader: &dyn GraphReader,
        opts: &BuildOptions,
    ) -> Result<BuildOutput, FatalError> {
        let verbose = opts.verbose;
        let out_dir = match opts.output_dir {
            Some(dir) => dir.to_path_buf(),
            None => root.join(".ariadne").join("graph"),
        };

        // Step 1: Load existing graph
        let old_graph_output = match reader.read_graph(&out_dir) {
            Ok(g) => g,
            Err(FatalError::GraphNotFound { .. }) => {
                if verbose {
                    eprintln!("[delta]     no prior graph — falling back to full build");
                }
                return self
                    .run_with_options(root, config, opts);
            }
            Err(FatalError::GraphCorrupted { ref reason, .. }) => {
                let reason_str = reason.clone();
                if verbose {
                    eprintln!(
                        "[delta]     corrupted graph: {} — falling back to full build",
                        reason_str
                    );
                }
                let mut result =
                    self.run_with_options(root, config, opts)?;
                let w011 = Warning {
                    code: WarningCode::W011GraphCorrupted,
                    path: CanonicalPath::new(out_dir.display().to_string()),
                    message: format!("graph corrupted: {}, rebuilding", reason_str),
                    detail: None,
                };
                result.warnings.push(w011);
                result.counts.graph_load_warnings += 1;
                return Ok(result);
            }
            Err(e) => return Err(e),
        };

        // W010: version mismatch check
        if old_graph_output.version != 1 {
            if verbose {
                eprintln!(
                    "[delta]     graph version {} != 1 — falling back to full build",
                    old_graph_output.version
                );
            }
            let mut result =
                self.run_with_options(root, config, opts)?;
            let w010 = Warning {
                code: WarningCode::W010GraphVersionMismatch,
                path: CanonicalPath::new(out_dir.display().to_string()),
                message: "graph version mismatch, rebuilding".to_string(),
                detail: None,
            };
            result.warnings.push(w010);
            result.counts.graph_load_warnings += 1;
            return Ok(result);
        }

        let old_graph: ProjectGraph = match old_graph_output.try_into() {
            Ok(g) => g,
            Err(reason) => {
                if verbose {
                    eprintln!(
                        "[delta]     graph conversion failed: {} — falling back to full build",
                        reason
                    );
                }
                let mut result =
                    self.run_with_options(root, config, opts)?;
                let w011 = Warning {
                    code: WarningCode::W011GraphCorrupted,
                    path: CanonicalPath::new(out_dir.display().to_string()),
                    message: format!("graph corrupted: {}, rebuilding", reason),
                    detail: None,
                };
                result.warnings.push(w011);
                result.counts.graph_load_warnings += 1;
                return Ok(result);
            }
        };

        // Step 2: Walk + read current files to get hashes
        let abs_root = std::fs::canonicalize(root).map_err(|_| FatalError::ProjectNotFound {
            path: root.to_path_buf(),
        })?;

        let walk_result = self.walker.walk(&abs_root, &config)?;
        let mut file_contents: Vec<FileContent> = Vec::new();
        for entry in &walk_result.entries {
            if self.registry.parser_for(&entry.extension).is_none() {
                continue;
            }
            if let Ok(content) = self.reader.read(entry, &abs_root, config.max_file_size) {
                file_contents.push(content);
            }
        }
        file_contents.sort_by(|a, b| a.path.cmp(&b.path));

        // Step 3: Compute delta
        let delta_start = Instant::now();
        let current_hashes: Vec<(CanonicalPath, ContentHash)> = file_contents
            .iter()
            .map(|fc| (fc.path.clone(), fc.hash.clone()))
            .collect();

        let delta = algo::delta::compute_delta(&old_graph.nodes, &current_hashes);

        if verbose {
            let mode =
                if delta.changed.is_empty() && delta.added.is_empty() && delta.removed.is_empty() {
                    "no changes"
                } else if delta.requires_full_recompute {
                    "full recompute — >5% threshold"
                } else {
                    "incremental"
                };
            eprintln!(
                "[delta]     {:>6}ms  {} changed, {} added, {} removed ({})",
                delta_start.elapsed().as_millis(),
                delta.changed.len(),
                delta.added.len(),
                delta.removed.len(),
                mode,
            );
        }

        // Short-circuit: no changes at all
        if delta.changed.is_empty() && delta.added.is_empty() && delta.removed.is_empty() {
            // Load cluster count from existing clusters.json
            let cluster_count = reader
                .read_clusters(&out_dir)
                .map(|c| c.clusters.len())
                .unwrap_or(0);
            return Ok(BuildOutput {
                graph_path: out_dir.join("graph.json"),
                clusters_path: out_dir.join("clusters.json"),
                stats_path: out_dir.join("stats.json"),
                file_count: old_graph.nodes.len(),
                edge_count: old_graph.edges.len(),
                cluster_count,
                warnings: vec![],
                counts: DiagnosticCounts::default(),
            });
        }

        // Any changes detected — do a full rebuild for correctness.
        // The delta detection itself is the optimization: we skip the rebuild
        // entirely when nothing changed.
        self.run_with_options(root, config, opts)
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
                exports: node
                    .exports
                    .iter()
                    .map(|s| s.as_str().to_string())
                    .collect(),
                cluster: node.cluster.as_str().to_string(),
                fsd_layer: node.fsd_layer.map(|l| l.as_str().to_string()),
                symbols: node.symbols.clone(),
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
                files: cluster
                    .files
                    .iter()
                    .map(|p| p.as_str().to_string())
                    .collect(),
                file_count: cluster.file_count,
                internal_edges: cluster.internal_edges,
                external_edges: cluster.external_edges,
                cohesion: cluster.cohesion,
            },
        );
    }
    ClusterOutput { clusters }
}

/// Format current UTC time as ISO 8601 with seconds precision.
fn format_utc_timestamp() -> String {
    let now = time::OffsetDateTime::now_utc();
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        now.year(),
        now.month() as u8,
        now.day(),
        now.hour(),
        now.minute(),
        now.second(),
    )
}

/// Format byte size in human-readable form (e.g., "2.1MB", "24KB", "512B").
fn format_size(bytes: u64) -> String {
    if bytes >= 1_048_576 {
        format!("{:.1}MB", bytes as f64 / 1_048_576.0)
    } else if bytes >= 1024 {
        format!("{}KB", bytes / 1024)
    } else {
        format!("{}B", bytes)
    }
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
