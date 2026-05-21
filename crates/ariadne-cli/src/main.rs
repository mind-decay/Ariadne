//! `ariadne` CLI entrypoint — clap subcommand dispatch over the whole stack.
//!
//! `ariadne-cli` is the application's composition root: the one crate that
//! wires every adapter together, so it alone depends on the driving adapters
//! `ariadne-mcp` and `ariadne-watcher` [src: docs/adr/0007-cli-composition-root.md].
//!
//! `anyhow` is permitted here per the folder-layout rule (binary entrypoint);
//! each subcommand returns `anyhow::Result` and `main` renders the error.

mod commands;
mod config;
mod domain;
mod errors;

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};

/// Local-first code-intelligence for Claude.
#[derive(Debug, Parser)]
#[command(name = "ariadne", version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}

/// The eight `ariadne` subcommands [src: tier-10 `exit_criteria` #1;
/// tier-16 adds `setup`].
#[derive(Debug, Subcommand)]
enum Cmd {
    /// Scaffold `.ariadne/` and write a default `config.toml`.
    Init {
        /// Project root.
        #[arg(default_value = ".")]
        root: PathBuf,
    },
    /// One-shot project onboarding: scaffold `.ariadne/` config, merge the
    /// `ariadne` entry into `.mcp.json`, and refresh the Ariadne
    /// discoverability block in `CLAUDE.md`. Does not run an index.
    Setup {
        /// Project root.
        #[arg(default_value = ".")]
        root: PathBuf,
    },
    /// Cold-index the repository into `.ariadne/index.redb`.
    Index {
        /// Project root.
        #[arg(default_value = ".")]
        root: PathBuf,
        /// Discard any existing index before re-indexing.
        #[arg(long)]
        fresh: bool,
        /// Also run the external SCIP indexers. Off by default — they
        /// perform full language builds and are not part of the measured
        /// cold-index wall-clock [src: docs/adr/0009-parallel-cold-index.md].
        #[arg(long)]
        scip: bool,
    },
    /// Watch the repository and log invalidations until Ctrl-C.
    Watch {
        /// Project root.
        #[arg(default_value = ".")]
        root: PathBuf,
    },
    /// Host the MCP stdio server (alias for `ariadne-mcp serve`).
    Serve {
        /// Project root.
        #[arg(default_value = ".")]
        root: PathBuf,
        /// Also run the file watcher in this process.
        #[arg(long)]
        watch: bool,
    },
    /// Call one MCP tool in-process and print its JSON result.
    Query {
        /// Tool name, e.g. `blast_radius`.
        tool: String,
        /// JSON arguments object.
        #[arg(default_value = "{}")]
        args_json: String,
        /// Project root.
        #[arg(long, default_value = ".")]
        root: PathBuf,
    },
    /// Print index counts and the indexer availability matrix.
    Status {
        /// Project root.
        #[arg(default_value = ".")]
        root: PathBuf,
    },
    /// Print salsa per-table memory against the 256 MiB budget.
    Mem {
        /// Project root.
        #[arg(default_value = ".")]
        root: PathBuf,
    },
}

fn main() -> ExitCode {
    init_tracing();
    match run(Cli::parse().command) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(e) => {
            eprintln!("ariadne: {e:#}");
            ExitCode::FAILURE
        }
    }
}

/// Dispatch one parsed subcommand. The `bool` is the success flag — only
/// `mem` returns `false` (a table over budget) without an error.
fn run(cmd: Cmd) -> anyhow::Result<bool> {
    match cmd {
        Cmd::Init { root } => commands::init::run(&root).map(|()| true),
        Cmd::Setup { root } => commands::setup::run(&root).map(|()| true),
        Cmd::Index { root, fresh, scip } => commands::index::run(&root, fresh, scip).map(|()| true),
        Cmd::Watch { root } => commands::watch::run(&root).map(|()| true),
        Cmd::Serve { root, watch } => commands::serve::run(&root, watch).map(|()| true),
        Cmd::Query {
            tool,
            args_json,
            root,
        } => commands::query::run(&root, &tool, &args_json).map(|()| true),
        Cmd::Status { root } => commands::status::run(&root).map(|()| true),
        Cmd::Mem { root } => Ok(commands::mem::run(&root)),
    }
}

/// Install a stderr tracing subscriber so adapter logs never pollute the
/// stdout JSON / MCP stream. Honours `RUST_LOG`, defaulting to `warn`.
fn init_tracing() {
    use tracing_subscriber::EnvFilter;
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn"));
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .try_init();
}
