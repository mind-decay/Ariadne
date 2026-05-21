//! `ariadne watch` — run the file watcher, log invalidations, idle on SIGINT.

use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use ariadne_core::{Invalidation, WatcherSink};
use ariadne_salsa::AriadneDb;
use ariadne_watcher::{AriadneDbSink, Ignore, NotifyWatcher};

/// Period between watcher reconciliation passes.
const RECONCILE_INTERVAL: Duration = Duration::from_secs(30);

/// Sink that drives the salsa DB and logs each invalidation plus the
/// wall-clock cost of applying it.
#[derive(Debug)]
struct LoggingSink {
    inner: AriadneDbSink,
}

impl WatcherSink for LoggingSink {
    fn apply_invalidation(&mut self, inv: &Invalidation) {
        let started = Instant::now();
        self.inner.apply_invalidation(inv);
        let micros = started.elapsed().as_micros();
        let kind = match inv {
            Invalidation::Created { .. } => "created",
            Invalidation::Modified { .. } => "modified",
            Invalidation::Removed { .. } => "removed",
            Invalidation::HashDrift { .. } => "hash-drift",
        };
        eprintln!(
            "[watch] {kind:<10} {} ({micros} us apply)",
            inv.path().display()
        );
    }
}

/// Start the watcher and block until Ctrl-C, then join its threads
/// [src: tier-10 step 5].
///
/// # Errors
/// Propagates watcher-start, tokio-init, and signal-wait failures.
pub fn run(root: &Path) -> Result<()> {
    let db = Arc::new(Mutex::new(AriadneDb::new()));
    let ignore = Ignore::build(root).context("build ignore matcher")?;
    let sink = Box::new(LoggingSink {
        inner: AriadneDbSink::new(db),
    });
    let watcher =
        NotifyWatcher::start(root, ignore, sink, RECONCILE_INTERVAL).context("start watcher")?;
    eprintln!("[watch] watching {} — Ctrl-C to stop", root.display());

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .build()
        .context("build tokio runtime")?;
    runtime
        .block_on(tokio::signal::ctrl_c())
        .context("await SIGINT")?;

    eprintln!("[watch] shutting down");
    watcher.stop();
    Ok(())
}
