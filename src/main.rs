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
        /// Enable verbose output (per-stage timing, W006 import warnings)
        #[arg(long)]
        verbose: bool,
        /// Warning output format: "human" or "json"
        #[arg(long, default_value = "human")]
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
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Build {
            path,
            output,
            verbose,
            warnings,
            strict,
            timestamp,
            max_file_size,
            max_files,
        } => {
            run_build(
                &path,
                output.as_deref(),
                verbose,
                &warnings,
                strict,
                timestamp,
                max_file_size,
                max_files,
            );
        }
        Commands::Info => {
            run_info();
        }
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
) {
    let start = Instant::now();

    // Parse warning format
    let warning_format = match warnings {
        "json" => WarningFormat::Json,
        _ => WarningFormat::Human,
    };

    // Composition Root (D-020)
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

    match pipeline.run_with_output(path, config, output, timestamp, verbose) {
        Ok(build_output) => {
            let elapsed = start.elapsed();
            let report = DiagnosticReport {
                warnings: build_output.warnings,
                counts: build_output.counts,
            };

            // Print summary to stdout
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

            // Print warnings to stderr
            let warning_output = format_warnings(&report, warning_format, verbose);
            if !warning_output.is_empty() {
                eprintln!("{}", warning_output);
            }

            // --strict: exit 1 if any warnings occurred
            if strict && !report.warnings.is_empty() {
                process::exit(1);
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
