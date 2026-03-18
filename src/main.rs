use std::path::PathBuf;
use std::process;
use std::time::Instant;

use clap::{Parser, Subcommand};

use ariadne_graph::algo;
use ariadne_graph::diagnostic::{
    format_summary, format_warnings, DiagnosticReport, FatalError, WarningFormat,
};
use ariadne_graph::model::{CanonicalPath, ClusterMap, ProjectGraph};
use ariadne_graph::pipeline::{BuildPipeline, FsReader, FsWalker, WalkConfig};
use ariadne_graph::parser::ParserRegistry;
use ariadne_graph::serial::json::JsonSerializer;
use ariadne_graph::serial::{GraphReader, StatsOutput};

#[derive(Parser)]
#[command(name = "ariadne", version, about = "Structural dependency graph engine")]
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
        } => run_build(
            &path,
            output.as_deref(),
            verbose,
            &warnings,
            strict,
            timestamp,
            max_file_size,
            max_files,
        ),
        Commands::Info => {
            run_info();
            Ok(())
        }
        Commands::Query { cmd } => run_query(cmd),
        Commands::Views { cmd } => run_views(cmd),
    };

    if let Err(e) = result {
        eprintln!("{}", e);
        process::exit(1);
    }
}

fn run_build(
    path: &PathBuf,
    output: Option<&std::path::Path>,
    verbose: bool,
    warnings: &str,
    strict: bool,
    timestamp: bool,
    max_file_size: u64,
    max_files: usize,
) -> Result<(), FatalError> {
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

    let build_output = pipeline.run_with_output(path, config, output, timestamp, verbose)?;
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
    let graph: ProjectGraph = output.try_into().map_err(|e: String| FatalError::GraphCorrupted {
        path: dir.join("graph.json"),
        reason: e,
    })?;
    Ok(graph)
}

fn load_stats(reader: &dyn GraphReader, dir: &std::path::Path) -> Result<StatsOutput, FatalError> {
    reader.read_stats(dir)?.ok_or_else(|| FatalError::StatsNotFound {
        path: dir.to_path_buf(),
    })
}

fn load_clusters(reader: &dyn GraphReader, dir: &std::path::Path) -> Result<ClusterMap, FatalError> {
    let output = reader.read_clusters(dir)?;
    let clusters: ClusterMap = output.try_into().map_err(|e: String| FatalError::GraphCorrupted {
        path: dir.join("clusters.json"),
        reason: e,
    })?;
    Ok(clusters)
}

// --- Query commands ---

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
            let result = algo::blast_radius::blast_radius(&graph, &path, depth);

            if format == "json" {
                let json_result: std::collections::BTreeMap<String, u32> = result
                    .iter()
                    .map(|(k, &v)| (k.as_str().to_string(), v))
                    .collect();
                println!("{}", serde_json::to_string_pretty(&json_result).unwrap());
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
                println!("{}", serde_json::to_string_pretty(&json).unwrap());
            } else {
                let view =
                    ariadne_graph::views::impact::generate_subgraph_view(&result, &graph);
                print!("{}", view);
            }
        }
        QueryCommands::Stats { format, graph_dir } => {
            let stats = load_stats(&reader, &graph_dir)?;

            if format == "json" {
                println!("{}", serde_json::to_string_pretty(&stats).unwrap());
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
            let filtered: std::collections::BTreeMap<&String, &f64> = stats
                .centrality
                .iter()
                .filter(|(_, &v)| v >= min)
                .collect();

            if format == "json" {
                println!("{}", serde_json::to_string_pretty(&filtered).unwrap());
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
                    println!("{}", serde_json::to_string_pretty(entry).unwrap());
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

            let node = graph.nodes.get(&cp).ok_or_else(|| FatalError::GraphNotFound {
                path: graph_dir.clone(),
            })?;

            if format == "json" {
                let file_output = ariadne_graph::serial::FileQueryOutput {
                    path: path.clone(),
                    node: ariadne_graph::serial::NodeOutput {
                        file_type: node.file_type.as_str().to_string(),
                        layer: node.layer.as_str().to_string(),
                        arch_depth: node.arch_depth,
                        lines: node.lines,
                        hash: node.hash.as_str().to_string(),
                        exports: node.exports.iter().map(|s| s.as_str().to_string()).collect(),
                        cluster: node.cluster.as_str().to_string(),
                    },
                    incoming_edges: graph.edges.iter()
                        .filter(|e| e.to == cp)
                        .map(|e| (
                            e.from.as_str().to_string(),
                            e.to.as_str().to_string(),
                            e.edge_type.as_str().to_string(),
                            e.symbols.iter().map(|s| s.as_str().to_string()).collect(),
                        ))
                        .collect(),
                    outgoing_edges: graph.edges.iter()
                        .filter(|e| e.from == cp)
                        .map(|e| (
                            e.from.as_str().to_string(),
                            e.to.as_str().to_string(),
                            e.edge_type.as_str().to_string(),
                            e.symbols.iter().map(|s| s.as_str().to_string()).collect(),
                        ))
                        .collect(),
                    centrality: stats.centrality.get(&path).copied(),
                    cluster: node.cluster.as_str().to_string(),
                };
                println!("{}", serde_json::to_string_pretty(&file_output).unwrap());
            } else {
                println!("# File: `{}`\n", path);
                println!("- **Type:** {}", node.file_type.as_str());
                println!("- **Layer:** {} (depth {})", node.layer.as_str(), node.arch_depth);
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
                        println!("- `{}` ({}, {:?})", e.from.as_str(), e.edge_type.as_str(),
                            e.symbols.iter().map(|s| s.as_str()).collect::<Vec<_>>());
                    }
                    println!();
                }

                let outgoing: Vec<_> = graph.edges.iter().filter(|e| e.from == cp).collect();
                if !outgoing.is_empty() {
                    println!("## Outgoing Edges\n");
                    for e in &outgoing {
                        println!("- `{}` ({}, {:?})", e.to.as_str(), e.edge_type.as_str(),
                            e.symbols.iter().map(|s| s.as_str()).collect::<Vec<_>>());
                    }
                    println!();
                }
            }
        }
        QueryCommands::Cycles { format, graph_dir } => {
            let stats = load_stats(&reader, &graph_dir)?;

            if format == "json" {
                println!("{}", serde_json::to_string_pretty(&stats.sccs).unwrap());
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
                println!("{}", serde_json::to_string_pretty(&stats.layers).unwrap());
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
