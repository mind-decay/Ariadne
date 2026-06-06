//! `ariadne` CLI entrypoint — clap subcommand dispatch over the whole stack.
//!
//! `ariadne-cli` is the application's composition root: the one crate that
//! wires every adapter together, so it alone depends on the driving adapters
//! `ariadne-mcp` and `ariadne-watcher` [src: docs/adr/0007-cli-composition-root.md].
//!
//! `anyhow` is permitted here per the folder-layout rule (binary entrypoint);
//! each subcommand returns `anyhow::Result` and `main` renders the error.

mod adapters;
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

/// The `ariadne` subcommands [src: tier-10 `exit_criteria` #1; tier-16 adds
/// `setup`; post-v1 tier-06 adds `daemon`].
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
        /// Skip the external SCIP indexers. SCIP runs by DEFAULT, out of band:
        /// the fast tree-sitter index commits first, then a separate SCIP pass
        /// re-commits the precise edges, so cold-index wall-clock is unchanged
        /// (R9). Pass this to index on the tree-sitter resolver only
        /// [src: docs/adr/0026-default-on-out-of-band-scip.md].
        #[arg(long = "no-scip")]
        no_scip: bool,
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
    /// Route one tool query to the warm daemon (cold in-process fallback) and
    /// print its JSON result.
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
    /// Print a compact, agent-shaped project digest (revision + counts, top
    /// coupled modules, a question→tool cheat-sheet) for session bootstrap.
    Digest {
        /// Project root.
        #[arg(default_value = ".")]
        root: PathBuf,
    },
    /// Write the project architecture overview to a Markdown file plus a
    /// sidecar SVG diagram (configurable paths).
    Doc {
        /// Project root (locates the index).
        #[arg(default_value = ".")]
        root: PathBuf,
        /// Markdown output path; relative paths resolve against `root`.
        #[arg(long, default_value = "docs/codebase-overview.md")]
        out: PathBuf,
        /// SVG sidecar output path; relative paths resolve against `root`.
        #[arg(long, default_value = "docs/codebase-overview.svg")]
        svg: PathBuf,
        /// Extra substring excludes layered atop the default `Source`-only
        /// doc scope; repeatable.
        #[arg(long)]
        exclude: Vec<String>,
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
    /// Manage the background daemon (RD5): start, stop, or query it.
    Daemon {
        #[command(subcommand)]
        action: DaemonAction,
    },
}

/// `ariadne daemon` lifecycle actions [src:
/// .claude/plans/post-v1-roadmap/tier-06-daemon-skeleton.md step 7].
#[derive(Debug, Subcommand)]
enum DaemonAction {
    /// Start the background daemon and wait until it is ready.
    Start {
        /// Project root.
        #[arg(default_value = ".")]
        root: PathBuf,
    },
    /// Stop the running daemon.
    Stop {
        /// Project root.
        #[arg(default_value = ".")]
        root: PathBuf,
    },
    /// Report whether the daemon is running.
    Status {
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
        Cmd::Index {
            root,
            fresh,
            no_scip,
        } => commands::index::run(&root, fresh, !no_scip).map(|()| true),
        Cmd::Watch { root } => commands::watch::run(&root).map(|()| true),
        Cmd::Serve { root, watch } => commands::serve::run(&root, watch).map(|()| true),
        Cmd::Query {
            tool,
            args_json,
            root,
        } => commands::query::run(&root, &tool, &args_json).map(|()| true),
        Cmd::Digest { root } => {
            commands::digest::run(&root);
            Ok(true)
        }
        Cmd::Doc {
            root,
            out,
            svg,
            exclude,
        } => commands::doc::run(&root, &out, &svg, &exclude).map(|()| true),
        Cmd::Status { root } => commands::status::run(&root).map(|()| true),
        Cmd::Mem { root } => Ok(commands::mem::run(&root)),
        Cmd::Daemon { action } => match action {
            DaemonAction::Start { root } => commands::daemon::start(&root).map(|()| true),
            DaemonAction::Stop { root } => commands::daemon::stop(&root).map(|()| true),
            DaemonAction::Status { root } => commands::daemon::status(&root).map(|()| true),
        },
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
