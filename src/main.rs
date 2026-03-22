use std::path::PathBuf;
use std::process;
use std::time::Instant;

use clap::{Parser, Subcommand};

use ariadne_graph::algo;
use ariadne_graph::diagnostic::{
    format_summary, format_warnings, DiagnosticReport, FatalError, WarningFormat,
};
use ariadne_graph::model::StatsOutput;
use ariadne_graph::model::{CanonicalPath, ClusterMap, ProjectGraph};
use ariadne_graph::parser::ParserRegistry;
use ariadne_graph::pipeline::{BuildPipeline, FsReader, FsWalker, WalkConfig};
use ariadne_graph::serial::json::JsonSerializer;
use ariadne_graph::serial::GraphReader;

#[derive(Parser)]
#[command(
    name = "ariadne",
    version,
    about = "Structural dependency graph engine"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Parse project and build dependency graph
    Build {
        /// Path to the project root
        path: PathBuf,
        /// Output directory (default: .ariadne/graph/)
        #[arg(long, short)]
        output: Option<PathBuf>,
        /// Enable verbose output (per-stage timing, W006 import warnings)
        #[arg(long)]
        verbose: bool,
        /// Warning output format: "human" or "json"
        #[arg(long, default_value = "human", value_parser = ["human", "json"])]
        warnings: String,
        /// Exit with code 1 if any warnings occurred
        #[arg(long)]
        strict: bool,
        /// Include generation timestamp in output
        #[arg(long)]
        timestamp: bool,
        /// Maximum file size in bytes (default: 1048576 = 1MB)
        #[arg(long, default_value_t = 1_048_576)]
        max_file_size: u64,
        /// Maximum number of files to process (default: 50000)
        #[arg(long, default_value_t = 50_000)]
        max_files: usize,
        /// Disable Louvain clustering (use directory-based clusters only)
        #[arg(long)]
        no_louvain: bool,
    },
    /// Show version and supported languages
    Info,
    /// Query the dependency graph
    Query {
        #[command(subcommand)]
        cmd: QueryCommands,
    },
    /// Generate markdown views
    Views {
        #[command(subcommand)]
        cmd: ViewsCommands,
    },
    /// Start MCP server for instant graph queries
    #[cfg(feature = "serve")]
    Serve {
        /// Project root to serve
        #[arg(long, default_value = ".")]
        project: PathBuf,
        /// Output directory (default: <project>/.ariadne/graph/)
        #[arg(long, short)]
        output: Option<PathBuf>,
        /// Debounce milliseconds for file watcher
        #[arg(long, default_value_t = 2000)]
        debounce: u64,
        /// Disable file system watcher
        #[arg(long)]
        no_watch: bool,
    },
    /// Incremental update via delta computation
    Update {
        /// Path to the project root
        path: PathBuf,
        /// Output directory (default: .ariadne/graph/)
        #[arg(long, short)]
        output: Option<PathBuf>,
        /// Enable verbose output
        #[arg(long)]
        verbose: bool,
        /// Warning output format: "human" or "json"
        #[arg(long, default_value = "human", value_parser = ["human", "json"])]
        warnings: String,
        /// Exit with code 1 if any warnings occurred
        #[arg(long)]
        strict: bool,
        /// Include generation timestamp in output
        #[arg(long)]
        timestamp: bool,
        /// Maximum file size in bytes (default: 1048576 = 1MB)
        #[arg(long, default_value_t = 1_048_576)]
        max_file_size: u64,
        /// Maximum number of files to process (default: 50000)
        #[arg(long, default_value_t = 50_000)]
        max_files: usize,
        /// Disable Louvain clustering
        #[arg(long)]
        no_louvain: bool,
    },
}

#[derive(Subcommand)]
enum QueryCommands {
    /// Show blast radius for a file
    BlastRadius {
        /// File path to analyze
        file: String,
        /// Maximum depth (default: unbounded)
        #[arg(long)]
        depth: Option<u32>,
        /// Output format
        #[arg(long, default_value = "md", value_parser = ["md", "json"])]
        format: String,
        /// Graph directory
        #[arg(long, default_value = ".ariadne/graph/")]
        graph_dir: PathBuf,
    },
    /// Extract subgraph around files
    Subgraph {
        /// Center file(s)
        files: Vec<String>,
        /// BFS depth
        #[arg(long, default_value_t = 2)]
        depth: u32,
        /// Output format
        #[arg(long, default_value = "md", value_parser = ["md", "json"])]
        format: String,
        /// Graph directory
        #[arg(long, default_value = ".ariadne/graph/")]
        graph_dir: PathBuf,
    },
    /// Show project statistics
    Stats {
        /// Output format
        #[arg(long, default_value = "md", value_parser = ["md", "json"])]
        format: String,
        /// Graph directory
        #[arg(long, default_value = ".ariadne/graph/")]
        graph_dir: PathBuf,
    },
    /// Show betweenness centrality scores
    Centrality {
        /// Minimum centrality threshold
        #[arg(long, default_value_t = 0.0)]
        min: f64,
        /// Output format
        #[arg(long, default_value = "md", value_parser = ["md", "json"])]
        format: String,
        /// Graph directory
        #[arg(long, default_value = ".ariadne/graph/")]
        graph_dir: PathBuf,
    },
    /// Show cluster details
    Cluster {
        /// Cluster name
        name: String,
        /// Output format
        #[arg(long, default_value = "md", value_parser = ["md", "json"])]
        format: String,
        /// Graph directory
        #[arg(long, default_value = ".ariadne/graph/")]
        graph_dir: PathBuf,
    },
    /// Show file details
    File {
        /// File path
        path: String,
        /// Output format
        #[arg(long, default_value = "md", value_parser = ["md", "json"])]
        format: String,
        /// Graph directory
        #[arg(long, default_value = ".ariadne/graph/")]
        graph_dir: PathBuf,
    },
    /// Show circular dependencies
    Cycles {
        /// Output format
        #[arg(long, default_value = "md", value_parser = ["md", "json"])]
        format: String,
        /// Graph directory
        #[arg(long, default_value = ".ariadne/graph/")]
        graph_dir: PathBuf,
    },
    /// Show topological layers
    Layers {
        /// Output format
        #[arg(long, default_value = "md", value_parser = ["md", "json"])]
        format: String,
        /// Graph directory
        #[arg(long, default_value = ".ariadne/graph/")]
        graph_dir: PathBuf,
    },
    /// Show Martin metrics per cluster
    Metrics {
        /// Output format
        #[arg(long, default_value = "md", value_parser = ["md", "json"])]
        format: String,
        /// Graph directory
        #[arg(long, default_value = ".ariadne/graph/")]
        graph_dir: PathBuf,
    },
    /// Detect architectural smells
    Smells {
        /// Minimum severity: "high", "medium", or "low"
        #[arg(long)]
        min_severity: Option<String>,
        /// Output format
        #[arg(long, default_value = "md", value_parser = ["md", "json"])]
        format: String,
        /// Graph directory
        #[arg(long, default_value = ".ariadne/graph/")]
        graph_dir: PathBuf,
    },
    /// Show file importance ranking (centrality + PageRank)
    Importance {
        /// Number of top files to show
        #[arg(long, default_value_t = 20)]
        top: usize,
        /// Output format
        #[arg(long, default_value = "md", value_parser = ["md", "json"])]
        format: String,
        /// Graph directory
        #[arg(long, default_value = ".ariadne/graph/")]
        graph_dir: PathBuf,
    },
    /// Spectral analysis: algebraic connectivity, monolith score, Fiedler bisection
    Spectral {
        /// Output format
        #[arg(long, default_value = "md", value_parser = ["md", "json"])]
        format: String,
        /// Graph directory
        #[arg(long, default_value = ".ariadne/graph/")]
        graph_dir: PathBuf,
    },
    /// Show compressed graph at project/cluster/file level
    Compressed {
        /// Compression level: 0 (project), 1 (cluster), 2 (file)
        #[arg(long)]
        level: u32,
        /// Focus: cluster name (level 1) or file path (level 2)
        #[arg(long)]
        focus: Option<String>,
        /// BFS depth for level 2 (default: 2)
        #[arg(long, default_value_t = 2)]
        depth: u32,
        /// Output format
        #[arg(long, default_value = "md", value_parser = ["md", "json"])]
        format: String,
        /// Graph directory
        #[arg(long, default_value = ".ariadne/graph/")]
        graph_dir: PathBuf,
    },
}

#[derive(Subcommand)]
enum ViewsCommands {
    /// Generate L0 index + L1 cluster views
    Generate {
        /// Output directory (default: .ariadne/views/)
        #[arg(long, default_value = ".ariadne/views/")]
        output: PathBuf,
        /// Graph directory
        #[arg(long, default_value = ".ariadne/graph/")]
        graph_dir: PathBuf,
    },
}

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Build {
            path,
            output,
            verbose,
            warnings,
            strict,
            timestamp,
            max_file_size,
            max_files,
            no_louvain,
        } => run_build(
            &path,
            output.as_deref(),
            verbose,
            &warnings,
            strict,
            timestamp,
            max_file_size,
            max_files,
            no_louvain,
        ),
        Commands::Info => {
            run_info();
            Ok(())
        }
        Commands::Query { cmd } => run_query(cmd),
        Commands::Views { cmd } => run_views(cmd),
        #[cfg(feature = "serve")]
        Commands::Serve {
            project,
            output,
            debounce,
            no_watch,
        } => {
            let abs_project = std::fs::canonicalize(&project).unwrap_or(project);
            let output_dir = output.unwrap_or_else(|| abs_project.join(".ariadne").join("graph"));
            let pipeline = std::sync::Arc::new(BuildPipeline::new(
                Box::new(FsWalker::new()),
                Box::new(FsReader::new()),
                ParserRegistry::with_tier1(),
                Box::new(JsonSerializer),
            ));
            let config = ariadne_graph::mcp::server::ServeConfig {
                project_root: abs_project,
                output_dir,
                debounce_ms: debounce,
                watch_enabled: !no_watch,
                pipeline,
            };
            let rt = tokio::runtime::Runtime::new().map_err(|e| {
                ariadne_graph::diagnostic::FatalError::McpServerFailed {
                    reason: format!("failed to create tokio runtime: {}", e),
                }
            });
            match rt {
                Ok(rt) => rt.block_on(ariadne_graph::mcp::server::run(config)),
                Err(e) => Err(e),
            }
        }
        Commands::Update {
            path,
            output,
            verbose,
            warnings,
            strict,
            timestamp,
            max_file_size,
            max_files,
            no_louvain,
        } => run_update(
            &path,
            output.as_deref(),
            verbose,
            &warnings,
            strict,
            timestamp,
            max_file_size,
            max_files,
            no_louvain,
        ),
    };

    if let Err(e) = result {
        eprintln!("{}", e);
        process::exit(1);
    }
}

/// Check if MCP server is running and block CLI write operations.
#[cfg(feature = "serve")]
fn check_server_lock(output_dir: &std::path::Path) -> Result<(), FatalError> {
    let lock_path = output_dir.join(".lock");
    if let Ok(ariadne_graph::mcp::lock::LockStatus::HeldByOther { pid }) =
        ariadne_graph::mcp::lock::check_lock(&lock_path)
    {
        return Err(FatalError::LockFileHeld { pid, lock_path });
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn run_build(
    path: &std::path::Path,
    output: Option<&std::path::Path>,
    verbose: bool,
    warnings: &str,
    strict: bool,
    timestamp: bool,
    max_file_size: u64,
    max_files: usize,
    no_louvain: bool,
) -> Result<(), FatalError> {
    // Check if MCP server is running
    #[cfg(feature = "serve")]
    {
        let output_dir = output
            .map(|d| d.to_path_buf())
            .unwrap_or_else(|| path.join(".ariadne").join("graph"));
        check_server_lock(&output_dir)?;
    }

    let start = Instant::now();

    let warning_format = match warnings {
        "json" => WarningFormat::Json,
        _ => WarningFormat::Human,
    };

    let pipeline = BuildPipeline::new(
        Box::new(FsWalker::new()),
        Box::new(FsReader::new()),
        ParserRegistry::with_tier1(),
        Box::new(JsonSerializer),
    );

    let config = WalkConfig {
        max_files,
        max_file_size,
        ..WalkConfig::default()
    };

    let build_output =
        pipeline.run_with_output(path, config, output, timestamp, verbose, no_louvain)?;
    let elapsed = start.elapsed();
    let report = DiagnosticReport {
        warnings: build_output.warnings,
        counts: build_output.counts,
    };

    println!(
        "{}",
        format_summary(
            &report,
            build_output.file_count,
            build_output.edge_count,
            build_output.cluster_count,
            elapsed,
        )
    );

    let warning_output = format_warnings(&report, warning_format, verbose);
    if !warning_output.is_empty() {
        eprintln!("{}", warning_output);
    }

    if strict && !report.warnings.is_empty() {
        process::exit(1);
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn run_update(
    path: &std::path::Path,
    output: Option<&std::path::Path>,
    verbose: bool,
    warnings: &str,
    strict: bool,
    timestamp: bool,
    max_file_size: u64,
    max_files: usize,
    no_louvain: bool,
) -> Result<(), FatalError> {
    // Check if MCP server is running
    #[cfg(feature = "serve")]
    {
        let output_dir = output
            .map(|d| d.to_path_buf())
            .unwrap_or_else(|| path.join(".ariadne").join("graph"));
        check_server_lock(&output_dir)?;
    }

    let start = Instant::now();

    let warning_format = match warnings {
        "json" => WarningFormat::Json,
        _ => WarningFormat::Human,
    };

    let pipeline = BuildPipeline::new(
        Box::new(FsWalker::new()),
        Box::new(FsReader::new()),
        ParserRegistry::with_tier1(),
        Box::new(JsonSerializer),
    );

    let reader = JsonSerializer;

    let config = WalkConfig {
        max_files,
        max_file_size,
        ..WalkConfig::default()
    };

    let build_output = pipeline.update(
        path, config, &reader, output, timestamp, verbose, no_louvain,
    )?;
    let elapsed = start.elapsed();
    let report = DiagnosticReport {
        warnings: build_output.warnings,
        counts: build_output.counts,
    };

    println!(
        "{}",
        format_summary(
            &report,
            build_output.file_count,
            build_output.edge_count,
            build_output.cluster_count,
            elapsed,
        )
    );

    let warning_output = format_warnings(&report, warning_format, verbose);
    if !warning_output.is_empty() {
        eprintln!("{}", warning_output);
    }

    if strict && !report.warnings.is_empty() {
        process::exit(1);
    }

    Ok(())
}

fn run_info() {
    let registry = ParserRegistry::with_tier1();
    println!("ariadne {}", env!("CARGO_PKG_VERSION"));
    println!();
    println!("Supported languages:");
    for lang in registry.language_names() {
        println!("  - {}", lang);
    }
    println!();
    println!("Supported extensions:");
    for ext in registry.supported_extensions() {
        println!("  .{}", ext);
    }
}

// --- Graph loading helpers ---

fn load_graph(reader: &dyn GraphReader, dir: &std::path::Path) -> Result<ProjectGraph, FatalError> {
    let output = reader.read_graph(dir)?;
    let graph: ProjectGraph =
        output
            .try_into()
            .map_err(|e: String| FatalError::GraphCorrupted {
                path: dir.join("graph.json"),
                reason: e,
            })?;
    Ok(graph)
}

fn load_stats(reader: &dyn GraphReader, dir: &std::path::Path) -> Result<StatsOutput, FatalError> {
    reader
        .read_stats(dir)?
        .ok_or_else(|| FatalError::StatsNotFound {
            path: dir.to_path_buf(),
        })
}

fn load_clusters(
    reader: &dyn GraphReader,
    dir: &std::path::Path,
) -> Result<ClusterMap, FatalError> {
    let output = reader.read_clusters(dir)?;
    let clusters: ClusterMap =
        output
            .try_into()
            .map_err(|e: String| FatalError::GraphCorrupted {
                path: dir.join("clusters.json"),
                reason: e,
            })?;
    Ok(clusters)
}

// --- Query commands ---

/// Serialize to pretty JSON, mapping errors to FatalError.
fn json_pretty<T: serde::Serialize>(value: &T) -> Result<String, FatalError> {
    serde_json::to_string_pretty(value).map_err(|e| FatalError::OutputNotWritable {
        path: std::path::PathBuf::from("<stdout>"),
        reason: format!("JSON serialization failed: {}", e),
    })
}

fn run_query(cmd: QueryCommands) -> Result<(), FatalError> {
    let reader = JsonSerializer;

    match cmd {
        QueryCommands::BlastRadius {
            file,
            depth,
            format,
            graph_dir,
        } => {
            let graph = load_graph(&reader, &graph_dir)?;
            let path = CanonicalPath::new(file);
            let index = algo::AdjacencyIndex::build(&graph.edges, algo::is_architectural);
            let result = algo::blast_radius::blast_radius(&graph, &path, depth, &index);

            if format == "json" {
                let json_result: std::collections::BTreeMap<String, u32> = result
                    .iter()
                    .map(|(k, &v)| (k.as_str().to_string(), v))
                    .collect();
                println!("{}", json_pretty(&json_result)?);
            } else {
                let view = ariadne_graph::views::impact::generate_blast_radius_view(
                    path.as_str(),
                    &result,
                    &graph,
                );
                print!("{}", view);
            }
        }
        QueryCommands::Subgraph {
            files,
            depth,
            format,
            graph_dir,
        } => {
            let graph = load_graph(&reader, &graph_dir)?;
            let paths: Vec<CanonicalPath> = files.into_iter().map(CanonicalPath::new).collect();
            let result = algo::subgraph::extract_subgraph(&graph, &paths, depth);

            if format == "json" {
                // Serialize SubgraphResult to JSON
                let json = serialize_subgraph_result(&result);
                println!("{}", json_pretty(&json)?);
            } else {
                let view = ariadne_graph::views::impact::generate_subgraph_view(&result);
                print!("{}", view);
            }
        }
        QueryCommands::Stats { format, graph_dir } => {
            let stats = load_stats(&reader, &graph_dir)?;

            if format == "json" {
                println!("{}", json_pretty(&stats)?);
            } else {
                print_stats_md(&stats);
            }
        }
        QueryCommands::Centrality {
            min,
            format,
            graph_dir,
        } => {
            let stats = load_stats(&reader, &graph_dir)?;
            let filtered: std::collections::BTreeMap<&String, &f64> =
                stats.centrality.iter().filter(|(_, &v)| v >= min).collect();

            if format == "json" {
                println!("{}", json_pretty(&filtered)?);
            } else {
                println!("# Betweenness Centrality (min: {:.4})\n", min);
                println!("| File | Centrality |");
                println!("|------|----------:|");
                let mut sorted: Vec<_> = filtered.into_iter().collect();
                sorted.sort_by(|a, b| {
                    b.1.partial_cmp(a.1)
                        .unwrap_or(std::cmp::Ordering::Equal)
                        .then_with(|| a.0.cmp(b.0))
                });
                for (path, &bc) in sorted {
                    println!("| `{}` | {:.4} |", path, bc);
                }
            }
        }
        QueryCommands::Cluster {
            name,
            format,
            graph_dir,
        } => {
            let reader_ref = &reader;
            if format == "json" {
                let cluster_output = reader_ref.read_clusters(&graph_dir)?;
                if let Some(entry) = cluster_output.clusters.get(&name) {
                    println!("{}", json_pretty(entry)?);
                } else {
                    eprintln!("Cluster '{}' not found", name);
                    process::exit(1);
                }
            } else {
                let graph = load_graph(&reader, &graph_dir)?;
                let stats = load_stats(&reader, &graph_dir)?;
                let view =
                    ariadne_graph::views::cluster::generate_cluster_view(&name, &graph, &stats);
                print!("{}", view);
            }
        }
        QueryCommands::File {
            path,
            format,
            graph_dir,
        } => {
            let graph = load_graph(&reader, &graph_dir)?;
            let stats = load_stats(&reader, &graph_dir)?;
            let cp = CanonicalPath::new(&path);

            let node = graph
                .nodes
                .get(&cp)
                .ok_or_else(|| FatalError::FileNotInGraph { path: path.clone() })?;

            if format == "json" {
                let file_output = ariadne_graph::serial::FileQueryOutput {
                    path: path.clone(),
                    node: ariadne_graph::serial::NodeOutput {
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
                    },
                    incoming_edges: graph
                        .edges
                        .iter()
                        .filter(|e| e.to == cp)
                        .map(|e| {
                            (
                                e.from.as_str().to_string(),
                                e.to.as_str().to_string(),
                                e.edge_type.as_str().to_string(),
                                e.symbols.iter().map(|s| s.as_str().to_string()).collect(),
                            )
                        })
                        .collect(),
                    outgoing_edges: graph
                        .edges
                        .iter()
                        .filter(|e| e.from == cp)
                        .map(|e| {
                            (
                                e.from.as_str().to_string(),
                                e.to.as_str().to_string(),
                                e.edge_type.as_str().to_string(),
                                e.symbols.iter().map(|s| s.as_str().to_string()).collect(),
                            )
                        })
                        .collect(),
                    centrality: stats.centrality.get(&path).copied(),
                    cluster: node.cluster.as_str().to_string(),
                };
                println!("{}", json_pretty(&file_output)?);
            } else {
                println!("# File: `{}`\n", path);
                println!("- **Type:** {}", node.file_type.as_str());
                println!(
                    "- **Layer:** {} (depth {})",
                    node.layer.as_str(),
                    node.arch_depth
                );
                println!("- **Cluster:** {}", node.cluster.as_str());
                println!("- **Lines:** {}", node.lines);
                if let Some(&bc) = stats.centrality.get(&path) {
                    println!("- **Centrality:** {:.4}", bc);
                }
                println!();

                let incoming: Vec<_> = graph.edges.iter().filter(|e| e.to == cp).collect();
                if !incoming.is_empty() {
                    println!("## Incoming Edges\n");
                    for e in &incoming {
                        println!(
                            "- `{}` ({}, {:?})",
                            e.from.as_str(),
                            e.edge_type.as_str(),
                            e.symbols.iter().map(|s| s.as_str()).collect::<Vec<_>>()
                        );
                    }
                    println!();
                }

                let outgoing: Vec<_> = graph.edges.iter().filter(|e| e.from == cp).collect();
                if !outgoing.is_empty() {
                    println!("## Outgoing Edges\n");
                    for e in &outgoing {
                        println!(
                            "- `{}` ({}, {:?})",
                            e.to.as_str(),
                            e.edge_type.as_str(),
                            e.symbols.iter().map(|s| s.as_str()).collect::<Vec<_>>()
                        );
                    }
                    println!();
                }
            }
        }
        QueryCommands::Cycles { format, graph_dir } => {
            let stats = load_stats(&reader, &graph_dir)?;

            if format == "json" {
                println!("{}", json_pretty(&stats.sccs)?);
            } else {
                println!("# Circular Dependencies\n");
                if stats.sccs.is_empty() {
                    println!("No circular dependencies found.");
                } else {
                    for (i, scc) in stats.sccs.iter().enumerate() {
                        println!("{}. **{} files:** {}", i + 1, scc.len(), scc.join(" → "));
                    }
                }
            }
        }
        QueryCommands::Layers { format, graph_dir } => {
            let stats = load_stats(&reader, &graph_dir)?;

            if format == "json" {
                println!("{}", json_pretty(&stats.layers)?);
            } else {
                println!("# Topological Layers\n");
                println!("Max depth: {}\n", stats.summary.max_depth);
                for (layer, files) in &stats.layers {
                    println!("## Layer {}\n", layer);
                    for file in files {
                        println!("- `{}`", file);
                    }
                    println!();
                }
            }
        }
        QueryCommands::Metrics { format, graph_dir } => {
            let graph = load_graph(&reader, &graph_dir)?;
            let clusters = load_clusters(&reader, &graph_dir)?;
            let metrics =
                ariadne_graph::analysis::metrics::compute_martin_metrics(&graph, &clusters);

            if format == "json" {
                println!("{}", json_pretty(&metrics)?);
            } else {
                println!("# Martin Metrics\n");
                println!("| Cluster | I | A | D | Zone |");
                println!("|---------|-----|-----|-----|------|");
                for (id, m) in &metrics {
                    println!(
                        "| {} | {:.4} | {:.4} | {:.4} | {:?} |",
                        id.as_str(),
                        m.instability,
                        m.abstractness,
                        m.distance,
                        m.zone
                    );
                }
            }
        }
        QueryCommands::Importance {
            top,
            format,
            graph_dir,
        } => {
            let graph = load_graph(&reader, &graph_dir)?;
            let stats = load_stats(&reader, &graph_dir)?;

            let pr = algo::pagerank::pagerank(&graph, 0.85, 100, 1e-6);
            let combined = algo::pagerank::combined_importance(&stats.centrality, &pr);

            let mut ranked: Vec<_> = combined.iter().collect();
            ranked.sort_by(|a, b| {
                b.1.partial_cmp(a.1)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| a.0.cmp(b.0))
            });
            ranked.truncate(top);

            if format == "json" {
                let result: Vec<serde_json::Value> = ranked
                    .iter()
                    .map(|(path, &score)| {
                        serde_json::json!({
                            "path": path.as_str(),
                            "combined_score": score,
                            "centrality": stats.centrality.get(path.as_str()).copied().unwrap_or(0.0),
                            "pagerank": pr.get(*path).copied().unwrap_or(0.0),
                        })
                    })
                    .collect();
                println!("{}", json_pretty(&result)?);
            } else {
                println!("# File Importance (top {})\n", top);
                println!("| File | Combined | Centrality | PageRank |");
                println!("|------|---------|-----------|---------|");
                for (path, &score) in &ranked {
                    let c = stats.centrality.get(path.as_str()).copied().unwrap_or(0.0);
                    let p = pr.get(*path).copied().unwrap_or(0.0);
                    println!(
                        "| `{}` | {:.4} | {:.4} | {:.4} |",
                        path.as_str(),
                        score,
                        c,
                        p
                    );
                }
            }
        }
        QueryCommands::Spectral { format, graph_dir } => {
            let graph = load_graph(&reader, &graph_dir)?;
            let result = algo::spectral::spectral_analysis(&graph, 200, 1e-6);

            if format == "json" {
                println!("{}", json_pretty(&result)?);
            } else {
                println!("# Spectral Analysis\n");
                println!(
                    "- **Algebraic connectivity (λ₂):** {:.4}",
                    result.algebraic_connectivity
                );
                println!("- **Monolith score:** {:.4}\n", result.monolith_score);
                for part in &result.natural_partitions {
                    println!("## Partition {}\n", part.partition_id);
                    for file in &part.files {
                        println!("- `{}`", file.as_str());
                    }
                    println!();
                }
            }
        }
        QueryCommands::Compressed {
            level,
            focus,
            depth,
            format,
            graph_dir,
        } => {
            let graph = load_graph(&reader, &graph_dir)?;
            let stats = load_stats(&reader, &graph_dir)?;
            let clusters = load_clusters(&reader, &graph_dir)?;

            let result = match level {
                0 => Ok(algo::compress::compress_l0(&graph, &clusters, &stats)),
                1 => {
                    let focus_name =
                        focus
                            .as_deref()
                            .ok_or_else(|| FatalError::InvalidArgument {
                                reason: "Level 1 requires --focus <cluster_name>".to_string(),
                            })?;
                    let cluster_id = ariadne_graph::model::ClusterId::new(focus_name);
                    algo::compress::compress_l1(&graph, &clusters, &stats, &cluster_id)
                        .map_err(|e| FatalError::InvalidArgument { reason: e })
                }
                2 => {
                    let focus_path =
                        focus
                            .as_deref()
                            .ok_or_else(|| FatalError::InvalidArgument {
                                reason: "Level 2 requires --focus <file_path>".to_string(),
                            })?;
                    let cp = CanonicalPath::new(focus_path);
                    algo::compress::compress_l2(&graph, &clusters, &stats, &cp, depth)
                        .map_err(|e| FatalError::InvalidArgument { reason: e })
                }
                _ => Err(FatalError::InvalidArgument {
                    reason: "Level must be 0, 1, or 2".to_string(),
                }),
            }?;

            if format == "json" {
                println!("{}", json_pretty(&result)?);
            } else {
                println!("# Compressed Graph (Level {})\n", level);
                if let Some(ref f) = result.focus {
                    println!("Focus: `{}`\n", f);
                }
                println!(
                    "Nodes: {}, Edges: {}, Tokens: ~{}\n",
                    result.nodes.len(),
                    result.edges.len(),
                    result.token_estimate
                );
                println!("## Nodes\n");
                for node in &result.nodes {
                    match node.node_type {
                        ariadne_graph::model::CompressedNodeType::Cluster => {
                            println!(
                                "- **{}** ({} files, cohesion: {:.2})",
                                node.name,
                                node.file_count.unwrap_or(0),
                                node.cohesion.unwrap_or(0.0)
                            );
                            if !node.key_files.is_empty() {
                                println!("  Key files: {}", node.key_files.join(", "));
                            }
                        }
                        ariadne_graph::model::CompressedNodeType::File => {
                            println!(
                                "- `{}` ({}, {})",
                                node.name,
                                node.file_type.as_deref().unwrap_or("?"),
                                node.layer.as_deref().unwrap_or("?")
                            );
                        }
                    }
                }
            }
        }
        QueryCommands::Smells {
            min_severity,
            format,
            graph_dir,
        } => {
            let graph = load_graph(&reader, &graph_dir)?;
            let stats = load_stats(&reader, &graph_dir)?;
            let clusters = load_clusters(&reader, &graph_dir)?;
            let metrics =
                ariadne_graph::analysis::metrics::compute_martin_metrics(&graph, &clusters);
            let smells =
                ariadne_graph::analysis::smells::detect_smells(&graph, &stats, &clusters, &metrics);

            let filtered: Vec<_> = if let Some(ref min_sev) = min_severity {
                let min = ariadne_graph::model::SmellSeverity::from_str_loose(min_sev);
                smells
                    .into_iter()
                    .filter(|s| s.severity.level() >= min.level())
                    .collect()
            } else {
                smells
            };

            if format == "json" {
                println!("{}", json_pretty(&filtered)?);
            } else {
                println!("# Architectural Smells\n");
                if filtered.is_empty() {
                    println!("No architectural smells detected.");
                } else {
                    for smell in &filtered {
                        println!(
                            "- **{:?}** ({:?}): {}",
                            smell.smell_type, smell.severity, smell.explanation
                        );
                        println!(
                            "  Files: {}",
                            smell
                                .files
                                .iter()
                                .map(|f| format!("`{}`", f.as_str()))
                                .collect::<Vec<_>>()
                                .join(", ")
                        );
                    }
                }
            }
        }
    }

    Ok(())
}

fn run_views(cmd: ViewsCommands) -> Result<(), FatalError> {
    let reader = JsonSerializer;

    match cmd {
        ViewsCommands::Generate { output, graph_dir } => {
            let graph = load_graph(&reader, &graph_dir)?;
            let clusters = load_clusters(&reader, &graph_dir)?;
            let stats = load_stats(&reader, &graph_dir)?;

            let count =
                ariadne_graph::views::generate_all_views(&graph, &clusters, &stats, &output)?;
            println!("Generated {} cluster views + index", count);
        }
    }

    Ok(())
}

fn print_stats_md(stats: &StatsOutput) {
    println!("# Project Statistics\n");
    println!("- **Max depth:** {}", stats.summary.max_depth);
    println!("- **Avg in-degree:** {:.4}", stats.summary.avg_in_degree);
    println!("- **Avg out-degree:** {:.4}", stats.summary.avg_out_degree);
    println!();

    if !stats.summary.bottleneck_files.is_empty() {
        println!("## Bottleneck Files (centrality > 0.7)\n");
        for file in &stats.summary.bottleneck_files {
            let bc = stats.centrality.get(file).copied().unwrap_or(0.0);
            println!("- `{}` ({:.4})", file, bc);
        }
        println!();
    }

    if !stats.sccs.is_empty() {
        println!("## Circular Dependencies\n");
        for (i, scc) in stats.sccs.iter().enumerate() {
            println!("{}. {} files: {}", i + 1, scc.len(), scc.join(" → "));
        }
        println!();
    }

    if !stats.summary.orphan_files.is_empty() {
        println!("## Orphan Files\n");
        for file in &stats.summary.orphan_files {
            println!("- `{}`", file);
        }
        println!();
    }
}

/// Serialize SubgraphResult for JSON output.
fn serialize_subgraph_result(result: &ariadne_graph::model::SubgraphResult) -> serde_json::Value {
    use serde_json::json;

    let nodes: std::collections::BTreeMap<String, serde_json::Value> = result
        .nodes
        .iter()
        .map(|(path, node)| {
            (
                path.as_str().to_string(),
                json!({
                    "type": node.file_type.as_str(),
                    "layer": node.layer.as_str(),
                    "arch_depth": node.arch_depth,
                    "lines": node.lines,
                    "hash": node.hash.as_str(),
                    "exports": node.exports.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
                    "cluster": node.cluster.as_str(),
                }),
            )
        })
        .collect();

    let edges: Vec<serde_json::Value> = result
        .edges
        .iter()
        .map(|e| {
            json!([
                e.from.as_str(),
                e.to.as_str(),
                e.edge_type.as_str(),
                e.symbols.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
            ])
        })
        .collect();

    json!({
        "nodes": nodes,
        "edges": edges,
        "center_files": result.center_files.iter().map(|p| p.as_str()).collect::<Vec<_>>(),
        "depth": result.depth,
    })
}
