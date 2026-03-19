use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use arc_swap::ArcSwap;
use notify_debouncer_full::{new_debouncer, DebounceEventResult, Debouncer, RecommendedCache};
use notify::RecursiveMode;

use crate::diagnostic::FatalError;
use crate::mcp::state::{GraphState, load_graph_state};
use crate::pipeline::{BuildPipeline, WalkConfig};
use crate::serial::json::JsonSerializer;

/// File watcher configuration and state.
pub struct FileWatcher {
    _debouncer: Debouncer<notify::RecommendedWatcher, RecommendedCache>,
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
    /// Start watching the project root for file changes.
    /// On change, triggers a full rebuild and swaps the graph state atomically.
    pub fn start(
        project_root: PathBuf,
        output_dir: PathBuf,
        debounce_ms: u64,
        state: Arc<ArcSwap<GraphState>>,
        rebuilding: Arc<AtomicBool>,
        pipeline: Arc<BuildPipeline>,
        known_extensions: HashSet<String>,
    ) -> Result<Self, FatalError> {
        let watch_root = project_root.clone();
        let debouncer = {
            let project_root = project_root;
            let output_dir = output_dir;
            let state = state.clone();
            let rebuilding = rebuilding.clone();
            let pipeline = pipeline.clone();
            let known_extensions = known_extensions.clone();

            let mut debouncer = new_debouncer(
                Duration::from_millis(debounce_ms),
                None,
                move |result: DebounceEventResult| {
                    match result {
                        Ok(events) => {
                            // Check if any event involves a file we care about
                            // Excludes .ariadne/ output directory to prevent recursive rebuilds
                            let relevant = events.iter().any(|e| {
                                e.event.paths.iter().any(|p| {
                                    should_trigger_rebuild(p, &known_extensions, &output_dir)
                                })
                            });

                            if !relevant {
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

                            let config = WalkConfig::default();
                            match pipeline.run_with_output(
                                &project_root,
                                config,
                                Some(&output_dir),
                                false,
                                false,
                                false,
                            ) {
                                Ok(_) => {
                                    // Reload state from disk
                                    let reader = JsonSerializer;
                                    match load_graph_state(&output_dir, &reader) {
                                        Ok(new_state) => {
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
        assert!(should_trigger_rebuild(Path::new("src/foo.ts"), &exts(), out));
        assert!(should_trigger_rebuild(Path::new("src/bar.rs"), &exts(), out));
        assert!(!should_trigger_rebuild(Path::new("README.md"), &exts(), out));
        assert!(!should_trigger_rebuild(Path::new("image.png"), &exts(), out));
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
