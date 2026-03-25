use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use arc_swap::ArcSwap;
use rmcp::ServiceExt;
use tokio_util::sync::CancellationToken;

use crate::diagnostic::FatalError;
use crate::mcp::lock::LockGuard;
use crate::mcp::state::{load_graph_state, GraphState};
use crate::mcp::tools::AriadneTools;
use crate::mcp::watch::FileWatcher;
use crate::pipeline::{BuildOptions, BuildPipeline, WalkConfig};
use crate::serial::json::JsonSerializer;

pub struct ServeConfig {
    pub project_root: PathBuf,
    pub output_dir: PathBuf,
    pub debounce_ms: u64,
    pub watch_enabled: bool,
    pub pipeline: Arc<BuildPipeline>,
}

/// Run the MCP server. This is the main entry point for `ariadne serve`.
pub async fn run(config: ServeConfig) -> Result<(), FatalError> {
    let lock_path = config.output_dir.join(".lock");

    // 1. Acquire lock (RAII — released on drop)
    let _lock = LockGuard::acquire(&lock_path)?;

    // 2. Detect Rust crate name once for the entire session
    let rust_crate_name = crate::detect::detect_rust_crate_name(&config.project_root);

    // 3. Load or build graph
    let graph_state = match load_graph_state(&config.output_dir, &JsonSerializer) {
        Ok(state) => state,
        Err(FatalError::GraphNotFound { .. }) | Err(FatalError::StatsNotFound { .. }) => {
            eprintln!(
                "[ariadne] No graph found in {}. Running initial build...",
                config.output_dir.display()
            );
            config.pipeline.run_with_options(
                &config.project_root,
                WalkConfig::default(),
                &BuildOptions {
                    output_dir: Some(&config.output_dir),
                    rust_crate_name: rust_crate_name.as_deref(),
                    ..BuildOptions::default()
                },
            )?;
            load_graph_state(&config.output_dir, &JsonSerializer)?
        }
        Err(e) => return Err(e),
    };

    // 3. Setup shared state
    let state = Arc::new(ArcSwap::from_pointee(graph_state));
    let rebuilding = Arc::new(AtomicBool::new(false));

    // 5. Start file watcher or poll fallback
    let _watcher = if config.watch_enabled {
        let pipeline = config.pipeline.clone();
        let known_extensions: HashSet<String> = pipeline
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
            pipeline.clone(),
            known_extensions.clone(),
            rust_crate_name.clone(),
        ) {
            Ok(w) => {
                eprintln!(
                    "[ariadne] File watcher started (debounce: {}ms)",
                    config.debounce_ms
                );
                // Spawn liveness monitor — warns if watcher thread dies (REQ-P3-07)
                let _monitor = FileWatcher::spawn_liveness_monitor(
                    w.heartbeat().clone(),
                    Duration::from_secs(300), // 5 minute stale threshold
                );
                Some(w)
            }
            Err(e) => {
                // W014: fs watcher failed — fall back to polling
                eprintln!(
                    "[ariadne] Warning (W014): file watcher failed: {}. Falling back to 30s polling.",
                    e
                );
                start_poll_fallback(
                    config.project_root.clone(),
                    config.output_dir.clone(),
                    state.clone(),
                    rebuilding.clone(),
                    pipeline,
                    known_extensions,
                    rust_crate_name.clone(),
                );
                None
            }
        }
    } else {
        None
    };

    // 6. Setup cancellation for graceful shutdown
    let cancel = CancellationToken::new();
    let cancel_signal = cancel.clone();
    tokio::spawn(async move {
        #[cfg(unix)]
        {
            use tokio::signal::unix::{signal, SignalKind};
            let mut sigterm =
                signal(SignalKind::terminate()).expect("failed to register SIGTERM handler");
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {
                    eprintln!("[ariadne] Received SIGINT, shutting down...");
                }
                _ = sigterm.recv() => {
                    eprintln!("[ariadne] Received SIGTERM, shutting down...");
                }
            }
        }
        #[cfg(not(unix))]
        {
            tokio::signal::ctrl_c().await.ok();
            eprintln!("[ariadne] Shutting down...");
        }
        cancel_signal.cancel();
    });

    // 7. Start MCP server on stdio
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

    // Wait for service completion or cancellation
    tokio::select! {
        result = service.waiting() => {
            result.map_err(|e| FatalError::McpProtocolError {
                reason: format!("MCP server error: {}", e),
            })?;
        }
        _ = cancel.cancelled() => {
            // Graceful shutdown — destructors will run
        }
    }

    // 8. Cleanup — lock released via LockGuard drop
    Ok(())
}

/// Start a polling fallback that checks for file changes every 30 seconds.
/// Used when the fs watcher fails to start (W014).
fn start_poll_fallback(
    project_root: PathBuf,
    output_dir: PathBuf,
    state: Arc<ArcSwap<GraphState>>,
    rebuilding: Arc<AtomicBool>,
    pipeline: Arc<BuildPipeline>,
    _known_extensions: HashSet<String>,
    rust_crate_name: Option<String>,
) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(30));
        interval.tick().await; // First tick is immediate — skip it
        loop {
            interval.tick().await;
            if rebuilding
                .compare_exchange(false, true, Ordering::SeqCst, Ordering::Relaxed)
                .is_err()
            {
                continue; // Already rebuilding
            }

            // Use incremental update with delta detection — only rebuilds
            // when file content hashes have actually changed
            let config = WalkConfig::default();
            let reader = JsonSerializer;
            match pipeline.update(
                &project_root,
                config,
                &reader,
                &BuildOptions {
                    output_dir: Some(&output_dir),
                    rust_crate_name: rust_crate_name.as_deref(),
                    ..BuildOptions::default()
                },
            ) {
                Ok(_) => match load_graph_state(&output_dir, &reader) {
                    Ok(new_state) => {
                        state.store(Arc::new(new_state));
                    }
                    Err(e) => {
                        eprintln!("[ariadne] Poll rebuild: failed to reload state: {}", e);
                    }
                },
                Err(e) => {
                    eprintln!("[ariadne] Poll rebuild failed: {}", e);
                }
            }

            rebuilding.store(false, Ordering::SeqCst);
        }
    });
}
