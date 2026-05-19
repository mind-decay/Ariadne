//! notify-rs + notify-debouncer-full driving adapter (tier-06 steps 2,4,5).
//!
//! Wraps `notify_debouncer_full::new_debouncer` with a 100ms quiet period
//! (the canonical setting used by rust-analyzer / zed / watchexec and
//! matched to typical editor save bursts: a save often fires
//! `Modify+Create+Modify+Modify` within ~10ms which the debouncer
//! coalesces) [src: <https://github.com/notify-rs/notify/tree/main/notify-debouncer-full>,
//! <https://docs.rs/notify-debouncer-full/0.7.0>].
//!
//! **`macOS` `FSEvents` caveat.** notify's `macOS` backend rides `FSEvents`,
//! which may coalesce or drop events under sustained write load. The
//! watcher mitigates by unioning the event stream with a periodic
//! gitignore-aware reconciliation walk that emits
//! `Invalidation::HashDrift` for files whose content hash drifted without
//! a corresponding notify event (R7, see
//! [`crate::adapters::reconcile`]) [src:
//! <https://github.com/notify-rs/notify> platform notes].

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use ariadne_core::{Invalidation, WatcherSink};
use notify::{
    RecursiveMode,
    event::{CreateKind, EventKind, ModifyKind, RemoveKind, RenameMode},
};
use notify_debouncer_full::{DebounceEventResult, DebouncedEvent, new_debouncer};
use tracing::{debug, warn};

use crate::adapters::ignore::Ignore;
use crate::adapters::reconcile::Reconciler;
use crate::errors::WatcherError;

/// Debounce quiet period — see module comment.
const DEBOUNCE_PERIOD: Duration = Duration::from_millis(100);

/// notify-rs driving adapter. The handle joins all spawned threads on
/// drop so callers never leak background workers.
#[derive(Debug)]
pub struct NotifyWatcher {
    stop: Arc<Mutex<bool>>,
    threads: Vec<JoinHandle<()>>,
}

impl NotifyWatcher {
    /// Spawn the watcher.
    ///
    /// * `root` — workspace root, watched recursively.
    /// * `ignore` — pre-built matcher; events under ignored paths are
    ///   dropped before they reach the sink.
    /// * `sink` — destination for translated invalidations. Wrapped in a
    ///   shared `Mutex` so notify + reconcile threads can both feed it.
    /// * `reconcile_interval` — period between reconciliation passes.
    ///   Pass a long duration (`Duration::from_secs(3600)`) in tests that
    ///   only exercise the notify path.
    ///
    /// # Errors
    /// Returns [`WatcherError::Notify`] when the notify backend refuses to
    /// install platform hooks.
    ///
    /// # Panics
    /// Panics if the stop-flag mutex is poisoned at spawn time, which
    /// can only happen if another thread panicked while holding it before
    /// the watcher started — i.e. never in practice.
    pub fn start(
        root: &Path,
        ignore: Ignore,
        sink: Box<dyn WatcherSink>,
        reconcile_interval: Duration,
    ) -> Result<Self, WatcherError> {
        let ignore = Arc::new(ignore);
        let sink: Arc<Mutex<Box<dyn WatcherSink>>> = Arc::new(Mutex::new(sink));
        let stop = Arc::new(Mutex::new(false));

        // --- notify thread ------------------------------------------------
        let (tx, rx) = mpsc::channel::<DebounceEventResult>();
        let mut debouncer = new_debouncer(DEBOUNCE_PERIOD, None, tx)
            .map_err(|e| WatcherError::Notify(e.to_string()))?;
        debouncer
            .watch(root, RecursiveMode::Recursive)
            .map_err(|e| WatcherError::Notify(e.to_string()))?;

        let notify_thread = {
            let ignore = Arc::clone(&ignore);
            let sink = Arc::clone(&sink);
            let stop = Arc::clone(&stop);
            thread::Builder::new()
                .name("ariadne-watcher-notify".into())
                .spawn(move || {
                    // Hold the debouncer alive for the lifetime of the
                    // pump; drop tears the notify backend down.
                    let _debouncer = debouncer;
                    loop {
                        if *stop.lock().unwrap() {
                            break;
                        }
                        match rx.recv_timeout(Duration::from_millis(200)) {
                            Ok(Ok(events)) => {
                                for ev in events {
                                    dispatch(&ev, ignore.as_ref(), &sink);
                                }
                            }
                            Ok(Err(errs)) => {
                                for e in errs {
                                    warn!(target: "ariadne_watcher", "notify error: {e}");
                                }
                            }
                            Err(RecvTimeoutError::Timeout) => {}
                            Err(RecvTimeoutError::Disconnected) => break,
                        }
                    }
                })
                .map_err(|e| WatcherError::Notify(format!("spawn notify thread: {e}")))?
        };

        // --- reconcile thread --------------------------------------------
        let reconcile_thread = {
            let ignore = Arc::clone(&ignore);
            let sink = Arc::clone(&sink);
            let stop = Arc::clone(&stop);
            let root = root.to_path_buf();
            thread::Builder::new()
                .name("ariadne-watcher-reconcile".into())
                .spawn(move || {
                    let mut reconciler = Reconciler::new(root, ignore);
                    // Poll the stop flag in slices so shutdown is prompt
                    // even when reconcile_interval is large.
                    let tick = Duration::from_millis(100).min(reconcile_interval);
                    let mut elapsed = Duration::ZERO;
                    loop {
                        if *stop.lock().unwrap() {
                            break;
                        }
                        if elapsed >= reconcile_interval {
                            let mut guard = match sink.lock() {
                                Ok(g) => g,
                                Err(poison) => poison.into_inner(),
                            };
                            let report = reconciler.run_pass(&mut **guard);
                            debug!(target: "ariadne_watcher",
                                "reconcile pass: checked={} drifts={} errors={}",
                                report.files_checked, report.drifts_emitted, report.errors.len());
                            elapsed = Duration::ZERO;
                        }
                        thread::sleep(tick);
                        elapsed += tick;
                    }
                })
                .map_err(|e| WatcherError::Notify(format!("spawn reconcile thread: {e}")))?
        };

        Ok(Self {
            stop,
            threads: vec![notify_thread, reconcile_thread],
        })
    }

    /// Signal shutdown and join both worker threads. Idempotent.
    pub fn stop(mut self) {
        self.signal_stop();
        for t in self.threads.drain(..) {
            let _ = t.join();
        }
    }

    fn signal_stop(&self) {
        if let Ok(mut g) = self.stop.lock() {
            *g = true;
        }
    }
}

impl Drop for NotifyWatcher {
    fn drop(&mut self) {
        self.signal_stop();
        for t in self.threads.drain(..) {
            let _ = t.join();
        }
    }
}

fn dispatch(ev: &DebouncedEvent, ignore: &Ignore, sink: &Arc<Mutex<Box<dyn WatcherSink>>>) {
    for inv in translate(ev, ignore) {
        let mut guard = match sink.lock() {
            Ok(g) => g,
            Err(poison) => poison.into_inner(),
        };
        guard.apply_invalidation(&inv);
    }
}

fn translate(ev: &DebouncedEvent, ignore: &Ignore) -> Vec<Invalidation> {
    let kind = ev.event.kind;
    let paths: Vec<PathBuf> = ev
        .event
        .paths
        .iter()
        .filter(|p| !ignore.is_ignored(p, false))
        .cloned()
        .collect();
    if paths.is_empty() {
        return Vec::new();
    }
    match kind {
        EventKind::Create(CreateKind::File | CreateKind::Any | CreateKind::Other) => paths
            .into_iter()
            .map(|path| Invalidation::Created { path })
            .collect(),
        EventKind::Modify(ModifyKind::Data(_) | ModifyKind::Any | ModifyKind::Other) => paths
            .into_iter()
            .map(|path| Invalidation::Modified { path })
            .collect(),
        EventKind::Modify(ModifyKind::Name(RenameMode::Both)) if paths.len() >= 2 => {
            // notify emits the (from, to) pair when both ends were seen.
            let mut iter = paths.into_iter();
            let from = iter.next().expect("paths.len() >= 2");
            let to = iter.next().expect("paths.len() >= 2");
            vec![
                Invalidation::Removed { path: from },
                Invalidation::Created { path: to },
            ]
        }
        EventKind::Modify(ModifyKind::Name(RenameMode::From)) => paths
            .into_iter()
            .map(|path| Invalidation::Removed { path })
            .collect(),
        EventKind::Modify(ModifyKind::Name(
            RenameMode::To | RenameMode::Any | RenameMode::Other,
        )) => paths
            .into_iter()
            .map(|path| Invalidation::Created { path })
            .collect(),
        EventKind::Remove(RemoveKind::File | RemoveKind::Any | RemoveKind::Other) => paths
            .into_iter()
            .map(|path| Invalidation::Removed { path })
            .collect(),
        // Folder create/remove + Access + Modify(Metadata) are noise we
        // skip — reconciliation catches anything important.
        _ => Vec::new(),
    }
}
