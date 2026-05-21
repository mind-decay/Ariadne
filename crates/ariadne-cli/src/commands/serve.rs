//! `ariadne serve` — host the MCP stdio server, optionally with the watcher.

use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{Context, Result};
use ariadne_mcp::{ServeOpts, serve_stdio};
use ariadne_salsa::AriadneDb;
use ariadne_watcher::{AriadneDbSink, Ignore, NotifyWatcher};

/// Period between watcher reconciliation passes when `--watch` is set.
const RECONCILE_INTERVAL: Duration = Duration::from_secs(30);

/// Run `ariadne-mcp serve` on stdio. With `watch`, the file watcher runs in
/// the same process and is shut down cleanly once stdin closes
/// [src: tier-10 step 6].
///
/// # Errors
/// Propagates watcher-start, tokio-init, and MCP serve failures.
pub fn run(root: &Path, watch: bool) -> Result<()> {
    let watcher = if watch {
        Some(start_watcher(root)?)
    } else {
        None
    };

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_io()
        .enable_time()
        .build()
        .context("build tokio runtime")?;
    let result = runtime.block_on(serve_stdio(ServeOpts::new(root)));

    if let Some(watcher) = watcher {
        watcher.stop();
    }
    result.context("mcp serve")?;
    Ok(())
}

/// Spawn the file watcher behind an [`AriadneDbSink`].
fn start_watcher(root: &Path) -> Result<NotifyWatcher> {
    let db = Arc::new(Mutex::new(AriadneDb::new()));
    let ignore = Ignore::build(root).context("build ignore matcher")?;
    let sink = Box::new(AriadneDbSink::new(db));
    NotifyWatcher::start(root, ignore, sink, RECONCILE_INTERVAL).context("start watcher")
}
