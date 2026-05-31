//! `serve_stdio` — driving entrypoint for the MCP stdio server.
//!
//! Tier-08 step 6: build a tokio rt (current-thread is enough — every
//! tool call returns within microseconds; the multi-thread workers come
//! from rmcp's task dispatch), open redb storage to read its revision, hand
//! the index path + revision to an [`AriadneServer`], and call
//! `server.serve(stdio()).await`. The in-RAM [`crate::Catalog`] + petgraph index is
//! built lazily on the first cold-fallback miss (tier-02), not at startup, so
//! session-open does not scale with index size when the daemon is warm.
//!
//! Watcher integration is deferred to tier-10: the workspace hexagonal
//! invariant forbids cross-driving-adapter deps (mcp → watcher), so the
//! end-to-end orchestrator wiring the watcher loop sits in the CLI
//! (tier-10) where both adapters meet [src: tests/architecture.rs
//! lines 39 + 104-113].

use std::path::{Path, PathBuf};
use std::sync::Arc;

use ariadne_core::Storage;
use ariadne_storage::RedbStorage;
use rmcp::ServiceExt;
use rmcp::transport::stdio;

use crate::errors::McpError;
use crate::server::AriadneServer;

/// Caller-supplied launch options.
#[derive(Debug, Clone)]
pub struct ServeOpts {
    /// Project root. `<root>/.ariadne/index.redb` is the on-disk index.
    pub root: PathBuf,
}

impl ServeOpts {
    /// Convenience constructor used by the bin entrypoint.
    #[must_use]
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    fn storage_path(&self) -> PathBuf {
        self.root.join(".ariadne").join("index.redb")
    }
}

/// Run the MCP server until stdin closes (Claude session shutdown) or
/// SIGINT is observed.
///
/// # Errors
/// Propagates storage open / revision read / rmcp service errors.
pub async fn serve_stdio(opts: ServeOpts) -> Result<(), McpError> {
    let server = build_server(&opts).await?;
    let transport = stdio();
    let running = server
        .serve(transport)
        .await
        .map_err(|e| McpError::Other(format!("rmcp initialize: {e}")))?;
    tokio::select! {
        result = running.waiting() => {
            result.map_err(|e| McpError::Other(format!("rmcp waiting: {e}")))?;
        }
        _ = tokio::signal::ctrl_c() => {}
    }
    Ok(())
}

/// Read the index revision + build the server. Exposed for in-process tests /
/// benches.
///
/// # Errors
/// Propagates the underlying storage-open / revision-read failure.
pub async fn build_server(opts: &ServeOpts) -> Result<AriadneServer, McpError> {
    let storage_path = opts.storage_path();
    // Read the persisted revision from a transient storage handle, then drop it
    // before returning — a single `KEY_REVISION` lookup (cheap atomic load), no
    // graph build. Session-open stays O(1): the cold-fallback catalog is built
    // lazily on the first daemon miss, not here, so a warm session that routes
    // every tool to the daemon never reads the full index. The dropped handle
    // also leaves the redb single-open lock free for the auto-spawned daemon
    // [src: crates/ariadne-mcp/src/server.rs `AriadneServer::catalog`].
    let revision = {
        let storage = open_storage(&storage_path)?;
        storage.revision().0
    };
    Ok(AriadneServer::new(
        storage_path,
        opts.root.clone(),
        revision,
    ))
}

fn open_storage(path: &Path) -> Result<Arc<RedbStorage>, McpError> {
    let storage = RedbStorage::open(path).map_err(McpError::Storage)?;
    Ok(Arc::new(storage))
}
