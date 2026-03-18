use std::path::PathBuf;
use std::process;
use std::time::Instant;

use clap::{Parser, Subcommand};

use ariadne_graph::pipeline::{BuildPipeline, FsReader, FsWalker, WalkConfig};
use ariadne_graph::parser::ParserRegistry;
use ariadne_graph::serial::json::JsonSerializer;

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
    },
    /// Show version and supported languages
    Info,
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Build { path, output: _ } => {
            run_build(&path);
        }
        Commands::Info => {
            run_info();
        }
    }
}

fn run_build(path: &PathBuf) {
    let start = Instant::now();

    // Composition Root (D-020)
    let pipeline = BuildPipeline::new(
        Box::new(FsWalker::new()),
        Box::new(FsReader::new()),
        ParserRegistry::with_tier1(),
        Box::new(JsonSerializer),
    );

    let config = WalkConfig::default();

    match pipeline.run(path, config) {
        Ok(output) => {
            let elapsed = start.elapsed();
            println!(
                "Built graph: {} files, {} edges, {} clusters in {:.1}s",
                output.file_count,
                output.edge_count,
                output.cluster_count,
                elapsed.as_secs_f64()
            );

            let skipped = output
                .warnings
                .iter()
                .filter(|w| {
                    matches!(
                        w.code,
                        ariadne_graph::diagnostic::WarningCode::W001ParseFailed
                            | ariadne_graph::diagnostic::WarningCode::W002ReadFailed
                            | ariadne_graph::diagnostic::WarningCode::W003FileTooLarge
                            | ariadne_graph::diagnostic::WarningCode::W004BinaryFile
                            | ariadne_graph::diagnostic::WarningCode::W009EncodingError
                    )
                })
                .count();

            if skipped > 0 {
                eprintln!("  {} files skipped", skipped);
            }

            // Print warnings to stderr
            for w in &output.warnings {
                eprintln!("warn[{}]: {} {}", w.code.code(), w.path, w.message);
            }
        }
        Err(e) => {
            eprintln!("{}", e);
            process::exit(1);
        }
    }
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
