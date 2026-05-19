//! Reconciliation walker (tier-06 step 6 / R7).
//!
//! `FSEvents` on `macOS` can coalesce or drop events under sustained
//! write load [src: <https://github.com/notify-rs/notify> platform
//! notes]. The reconciler periodically walks the workspace with the same
//! `ignore` matcher the notify thread uses, blake3-hashes every visited
//! file in 64KB chunks (so files >16MB never load whole), and emits
//! `Invalidation::HashDrift` for any path whose hash differs from the
//! sink's last-known value.

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use ariadne_core::{ContentHash, Invalidation, ReconciliationReport, WatcherSink};
use ignore::WalkBuilder;
use tracing::debug;

use crate::adapters::ignore::{ARIADNE_IGNORE_FILENAME, Ignore};

/// Streaming hash buffer. 64KB matches blake3's recommended chunk size
/// for incremental input [src: <https://github.com/BLAKE3-team/BLAKE3>].
/// Allocated on the heap so each pass holds a single boxed buffer rather
/// than a large stack frame (clippy `large_stack_arrays`).
const HASH_CHUNK_BYTES: usize = 64 * 1024;

/// Periodic full-walk hash reconciler. Stateful: the last-known hash
/// per path is cached so subsequent passes only emit drift events.
#[derive(Debug)]
pub struct Reconciler {
    root: PathBuf,
    ignore: Arc<Ignore>,
    known: HashMap<PathBuf, ContentHash>,
}

impl Reconciler {
    /// Build a fresh reconciler. The first call to [`Self::run_pass`]
    /// will emit a `HashDrift` from the all-zero hash to the on-disk
    /// hash for every visited file so downstream sinks bootstrap their
    /// state.
    #[must_use]
    pub fn new(root: PathBuf, ignore: Arc<Ignore>) -> Self {
        Self {
            root,
            ignore,
            known: HashMap::new(),
        }
    }

    /// Walk the tree once, pushing any drifts through `sink`. The report
    /// summarizes counts the caller can log without holding the sink.
    pub fn run_pass(&mut self, sink: &mut dyn WatcherSink) -> ReconciliationReport {
        let mut report = ReconciliationReport::default();
        let walker = WalkBuilder::new(&self.root)
            .git_global(true)
            .hidden(false)
            .add_custom_ignore_filename(ARIADNE_IGNORE_FILENAME)
            .build();

        for entry in walker {
            let entry = match entry {
                Ok(e) => e,
                Err(e) => {
                    report.errors.push(e.to_string());
                    continue;
                }
            };
            let path = entry.path();
            let is_dir = entry.file_type().is_some_and(|t| t.is_dir());
            if is_dir {
                continue;
            }
            if self.ignore.is_ignored(path, false) {
                continue;
            }
            // Skip symlinks pointed outside the workspace — `ignore` does
            // not follow by default, but the file_type check is cheap.
            if entry.file_type().is_some_and(|t| t.is_symlink()) {
                continue;
            }
            report.files_checked += 1;
            let hash = match hash_file(path) {
                Ok(h) => h,
                Err(e) => {
                    report.errors.push(format!("{}: {e}", path.display()));
                    continue;
                }
            };
            let prev = self.known.get(path).copied();
            if prev != Some(hash) {
                let old_hash = prev.unwrap_or([0u8; 32]);
                sink.apply_invalidation(&Invalidation::HashDrift {
                    path: path.to_path_buf(),
                    old_hash,
                    new_hash: hash,
                });
                report.drifts_emitted += 1;
                self.known.insert(path.to_path_buf(), hash);
                debug!(target: "ariadne_watcher", "drift: {}", path.display());
            }
        }
        report
    }

    /// Hash store size — exposed for tests + memory probes.
    #[must_use]
    pub fn tracked_paths(&self) -> usize {
        self.known.len()
    }
}

fn hash_file(path: &Path) -> std::io::Result<ContentHash> {
    let f = File::open(path)?;
    let mut reader = BufReader::with_capacity(HASH_CHUNK_BYTES, f);
    let mut hasher = blake3::Hasher::new();
    let mut buf = vec![0u8; HASH_CHUNK_BYTES].into_boxed_slice();
    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(*hasher.finalize().as_bytes())
}
