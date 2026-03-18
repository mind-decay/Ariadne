use std::path::PathBuf;
use std::process;
use std::time::Instant;

use clap::{Parser, Subcommand};

use ariadne_graph::diagnostic::{
    format_summary, format_warnings, DiagnosticReport, WarningFormat,
};
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
        Commands::Build { path, output } => {
            run_build(&path, output.as_deref());
        }
        Commands::Info => {
            run_info();
        }
    }
}

fn run_build(path: &PathBuf, output: Option<&std::path::Path>) {
    let start = Instant::now();

    // Composition Root (D-020)
    let pipeline = BuildPipeline::new(
        Box::new(FsWalker::new()),
        Box::new(FsReader::new()),
        ParserRegistry::with_tier1(),
        Box::new(JsonSerializer),
    );

    let config = WalkConfig::default();

    match pipeline.run_with_output(path, config, output) {
        Ok(output) => {
            let elapsed = start.elapsed();
            let report = DiagnosticReport {
                warnings: output.warnings,
                counts: output.counts,
            };

            // Print summary to stdout
            println!(
                "{}",
                format_summary(
                    &report,
                    output.file_count,
                    output.edge_count,
                    output.cluster_count,
                    elapsed,
                )
            );

            // Print warnings to stderr (Human format, non-verbose for now)
            let warning_output = format_warnings(&report, WarningFormat::Human, false);
            if !warning_output.is_empty() {
                eprintln!("{}", warning_output);
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
