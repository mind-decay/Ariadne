use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use arc_swap::ArcSwap;
use rmcp::ServiceExt;

use crate::diagnostic::FatalError;
use crate::mcp::lock::{acquire_lock, release_lock};
use crate::mcp::state::load_graph_state;
use crate::mcp::tools::AriadneTools;
use crate::mcp::watch::FileWatcher;
use crate::parser::ParserRegistry;
use crate::pipeline::{BuildPipeline, FsReader, FsWalker, WalkConfig};
use crate::serial::json::JsonSerializer;

pub struct ServeConfig {
    pub project_root: PathBuf,
    pub output_dir: PathBuf,
    pub debounce_ms: u64,
    pub watch_enabled: bool,
}

/// Run the MCP server. This is the main entry point for `ariadne serve`.
pub async fn run(config: ServeConfig) -> Result<(), FatalError> {
    let lock_path = config.output_dir.join(".lock");

    // 1. Acquire lock
    acquire_lock(&lock_path)?;

    // 2. Load or build graph
    let graph_state = match load_graph_state(&config.output_dir, &JsonSerializer) {
        Ok(state) => state,
        Err(FatalError::GraphNotFound { .. }) | Err(FatalError::StatsNotFound { .. }) => {
            eprintln!(
                "[ariadne] No graph found in {}. Running initial build...",
                config.output_dir.display()
            );
            let pipeline = make_pipeline();
            pipeline.run_with_output(
                &config.project_root,
                WalkConfig::default(),
                Some(&config.output_dir),
                false,
                false,
                false,
            )?;
            load_graph_state(&config.output_dir, &JsonSerializer)?
        }
        Err(e) => return Err(e),
    };

    // 3. Setup shared state
    let state = Arc::new(ArcSwap::from_pointee(graph_state));
    let rebuilding = Arc::new(AtomicBool::new(false));

    // 4. Start file watcher (if enabled)
    let _watcher = if config.watch_enabled {
        let pipeline = Arc::new(make_pipeline());
        let registry = ParserRegistry::with_tier1();
        let known_extensions: HashSet<String> = registry
            .supported_extensions()
            .into_iter()
            .map(|s| s.to_string())
            .collect();

        match FileWatcher::start(
            config.project_root.clone(),
            config.output_dir.clone(),
            config.debounce_ms,
            state.clone(),
            rebuilding.clone(),
            pipeline,
            known_extensions,
        ) {
            Ok(w) => {
                eprintln!("[ariadne] File watcher started (debounce: {}ms)", config.debounce_ms);
                Some(w)
            }
            Err(e) => {
                eprintln!("[ariadne] Warning: file watcher failed to start: {}. Running without auto-update.", e);
                None
            }
        }
    } else {
        None
    };

    // 5. Register signal handler for graceful shutdown
    let lock_for_shutdown = lock_path.clone();
    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.ok();
        release_lock(&lock_for_shutdown).ok();
        std::process::exit(0);
    });

    // 6. Start MCP server on stdio
    let tools = AriadneTools::new(
        state.clone(),
        rebuilding.clone(),
        config.project_root.clone(),
    );

    eprintln!(
        "[ariadne] MCP server ready. {} files, {} edges. Listening on stdio.",
        state.load().graph.nodes.len(),
        state.load().graph.edges.len(),
    );

    let transport = rmcp::transport::io::stdio();
    let service = tools
        .serve(transport)
        .await
        .map_err(|e| FatalError::McpServerFailed {
            reason: format!("failed to start MCP server: {}", e),
        })?;

    // Wait for the service to complete (client disconnects)
    service.waiting().await.map_err(|e| FatalError::McpProtocolError {
        reason: format!("MCP server error: {}", e),
    })?;

    // 7. Cleanup
    release_lock(&lock_path)?;
    Ok(())
}

fn make_pipeline() -> BuildPipeline {
    BuildPipeline::new(
        Box::new(FsWalker::new()),
        Box::new(FsReader::new()),
        ParserRegistry::with_tier1(),
        Box::new(JsonSerializer),
    )
}
