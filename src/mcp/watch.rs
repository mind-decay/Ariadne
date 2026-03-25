use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use arc_swap::ArcSwap;
use notify::RecursiveMode;
use notify_debouncer_full::{new_debouncer, DebounceEventResult, Debouncer, RecommendedCache};

use crate::analysis::diff::compute_structural_diff;
use crate::diagnostic::FatalError;
use crate::mcp::state::{load_graph_state, GraphState};
use crate::pipeline::{BuildOptions, BuildPipeline, WalkConfig};
use crate::serial::json::JsonSerializer;

/// File watcher configuration and state.
pub struct FileWatcher {
    _debouncer: Debouncer<notify::RecommendedWatcher, RecommendedCache>,
    /// Shared heartbeat: the watcher callback updates this on every event batch.
    /// A monitor task checks it periodically to detect watcher death.
    heartbeat: Arc<AtomicHeartbeat>,
}

/// Atomic heartbeat tracker for watcher liveness detection.
pub(crate) struct AtomicHeartbeat {
    /// Epoch millis of last successful watcher callback invocation.
    last_beat: std::sync::atomic::AtomicU64,
}

impl AtomicHeartbeat {
    fn new() -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        Self {
            last_beat: std::sync::atomic::AtomicU64::new(now),
        }
    }

    fn beat(&self) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        self.last_beat.store(now, Ordering::Relaxed);
    }

    fn elapsed_ms(&self) -> u64 {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        now.saturating_sub(self.last_beat.load(Ordering::Relaxed))
    }
}

/// Check if a file change should trigger a rebuild.
/// Rejects paths under `.ariadne/` output directory and filters by known extensions.
pub fn should_trigger_rebuild(
    path: &Path,
    known_extensions: &HashSet<String>,
    output_dir: &Path,
) -> bool {
    // Exclude changes under the output directory to prevent recursive rebuilds
    if path.starts_with(output_dir) {
        return false;
    }
    // Also exclude .ariadne/ by component match (covers non-canonical paths)
    for component in path.components() {
        if component.as_os_str() == ".ariadne" {
            return false;
        }
    }
    path.extension()
        .and_then(|e| e.to_str())
        .map(|ext| known_extensions.contains(ext))
        .unwrap_or(false)
}

impl FileWatcher {
    /// Returns a reference to the heartbeat tracker for liveness monitoring.
    pub(crate) fn heartbeat(&self) -> &Arc<AtomicHeartbeat> {
        &self.heartbeat
    }

    /// Start watching the project root for file changes.
    /// On change, triggers a full rebuild and swaps the graph state atomically.
    ///
    /// Includes a heartbeat monitor: if no watcher callback fires for 5 minutes,
    /// the watcher is considered dead and a warning is logged. The caller
    /// (server.rs) already has poll fallback as the recovery path.
    #[allow(clippy::too_many_arguments)]
    pub fn start(
        project_root: PathBuf,
        output_dir: PathBuf,
        debounce_ms: u64,
        state: Arc<ArcSwap<GraphState>>,
        rebuilding: Arc<AtomicBool>,
        pipeline: Arc<BuildPipeline>,
        known_extensions: HashSet<String>,
        rust_crate_name: Option<String>,
    ) -> Result<Self, FatalError> {
        let watch_root = project_root.clone();
        let heartbeat = Arc::new(AtomicHeartbeat::new());
        let debouncer = {
            let state = state.clone();
            let rebuilding = rebuilding.clone();
            let pipeline = pipeline.clone();
            let known_extensions = known_extensions.clone();
            let rust_crate_name = std::sync::Mutex::new(rust_crate_name);
            let heartbeat = heartbeat.clone();

            let mut debouncer = new_debouncer(
                Duration::from_millis(debounce_ms),
                None,
                move |result: DebounceEventResult| {
                    // Record heartbeat on every callback invocation (success or error)
                    heartbeat.beat();

                    match result {
                        Ok(events) => {
                            // Check if any event involves a file we care about
                            // Excludes .ariadne/ output directory to prevent recursive rebuilds
                            let relevant = events.iter().any(|e| {
                                e.event.paths.iter().any(|p| {
                                    should_trigger_rebuild(p, &known_extensions, &output_dir)
                                })
                            });

                            // Re-detect crate name if Cargo.toml changed
                            let cargo_toml_changed = events.iter().any(|e| {
                                e.event.paths.iter().any(|p| {
                                    p.file_name()
                                        .map(|n| n == "Cargo.toml")
                                        .unwrap_or(false)
                                })
                            });
                            if cargo_toml_changed {
                                let new_name =
                                    crate::detect::detect_rust_crate_name(&project_root);
                                if let Ok(mut guard) = rust_crate_name.lock() {
                                    *guard = new_name;
                                }
                            }

                            if !relevant && !cargo_toml_changed {
                                return;
                            }

                            // Trigger rebuild
                            if rebuilding
                                .compare_exchange(false, true, Ordering::SeqCst, Ordering::Relaxed)
                                .is_err()
                            {
                                // Already rebuilding, skip
                                return;
                            }

                            eprintln!("[ariadne] File changes detected, rebuilding...");

                            let crate_name = rust_crate_name
                                .lock()
                                .ok()
                                .and_then(|g| g.clone());
                            let config = WalkConfig::default();
                            match pipeline.run_with_options(
                                &project_root,
                                config,
                                &BuildOptions {
                                    output_dir: Some(&output_dir),
                                    rust_crate_name: crate_name.as_deref(),
                                    ..BuildOptions::default()
                                },
                            ) {
                                Ok(_) => {
                                    // Reload state from disk
                                    let reader = JsonSerializer;
                                    match load_graph_state(&output_dir, &reader, Some(&project_root)) {
                                        Ok(mut new_state) => {
                                            // Compute structural diff before state swap
                                            let old_state = state.load();
                                            let diff = compute_structural_diff(
                                                &old_state.graph,
                                                &old_state.stats,
                                                &old_state.clusters,
                                                &old_state.cluster_metrics,
                                                &new_state.graph,
                                                &new_state.stats,
                                                &new_state.clusters,
                                                &new_state.cluster_metrics,
                                            );
                                            new_state.last_diff = Some(diff);
                                            state.store(Arc::new(new_state));
                                            eprintln!("[ariadne] Rebuild complete, state updated.");
                                        }
                                        Err(e) => {
                                            eprintln!("[ariadne] Failed to reload state after rebuild: {}", e);
                                        }
                                    }
                                }
                                Err(e) => {
                                    eprintln!("[ariadne] Rebuild failed: {}", e);
                                }
                            }

                            rebuilding.store(false, Ordering::SeqCst);
                        }
                        Err(errors) => {
                            for e in errors {
                                eprintln!("[ariadne] Watch error: {}", e);
                            }
                        }
                    }
                },
            )
            .map_err(|e| FatalError::McpServerFailed {
                reason: format!("failed to create file watcher: {}", e),
            })?;

            debouncer
                .watch(&watch_root, RecursiveMode::Recursive)
                .map_err(|e| FatalError::McpServerFailed {
                    reason: format!("failed to watch directory: {}", e),
                })?;

            debouncer
        };

        Ok(Self {
            _debouncer: debouncer,
            heartbeat,
        })
    }

    /// Start a background monitor that logs a warning if the watcher appears dead.
    ///
    /// "Dead" = no heartbeat for `stale_threshold`. This is P3 priority:
    /// detection + warning only. The server already has poll fallback via
    /// `start_poll_fallback` in server.rs which handles the case where the
    /// watcher fails to start. For runtime death, we log a warning so
    /// operators are aware.
    pub(crate) fn spawn_liveness_monitor(
        heartbeat: Arc<AtomicHeartbeat>,
        stale_threshold: Duration,
    ) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let check_interval = stale_threshold.max(Duration::from_secs(60));
            let mut interval = tokio::time::interval(check_interval);
            interval.tick().await; // skip immediate tick
            let mut warned = false;
            loop {
                interval.tick().await;
                let elapsed = heartbeat.elapsed_ms();
                if elapsed > stale_threshold.as_millis() as u64 {
                    if !warned {
                        eprintln!(
                            "[ariadne] Warning (W015): file watcher may have died \
                             (no heartbeat for {}s). File changes may not trigger rebuilds.",
                            elapsed / 1000
                        );
                        warned = true;
                    }
                } else {
                    warned = false;
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn exts() -> HashSet<String> {
        ["ts", "js", "rs", "go", "py"]
            .iter()
            .map(|s| s.to_string())
            .collect()
    }

    #[test]
    fn test_should_trigger_rebuild() {
        let out = Path::new("/project/.ariadne/graph");
        assert!(should_trigger_rebuild(
            Path::new("src/foo.ts"),
            &exts(),
            out
        ));
        assert!(should_trigger_rebuild(
            Path::new("src/bar.rs"),
            &exts(),
            out
        ));
        assert!(!should_trigger_rebuild(
            Path::new("README.md"),
            &exts(),
            out
        ));
        assert!(!should_trigger_rebuild(
            Path::new("image.png"),
            &exts(),
            out
        ));
    }

    #[test]
    fn test_excludes_ariadne_output_dir() {
        let out = Path::new("/project/.ariadne/graph");
        assert!(!should_trigger_rebuild(
            Path::new("/project/.ariadne/graph/graph.json"),
            &exts(),
            out,
        ));
        assert!(!should_trigger_rebuild(
            Path::new("/project/.ariadne/graph/raw_imports.json"),
            &exts(),
            out,
        ));
    }

    #[test]
    fn test_excludes_ariadne_component() {
        let out = Path::new("/project/.ariadne/graph");
        assert!(!should_trigger_rebuild(
            Path::new("project/.ariadne/views/index.md"),
            &exts(),
            out,
        ));
    }
}
