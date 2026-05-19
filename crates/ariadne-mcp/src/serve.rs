//! `serve_stdio` — driving entrypoint for the MCP stdio server.
//!
//! Tier-08 step 6: build a tokio rt (current-thread is enough — every
//! tool call returns within microseconds; the multi-thread workers come
//! from rmcp's task dispatch), open redb storage, build the in-RAM
//! [`Catalog`] + petgraph index, hand them to an [`AriadneServer`], and
//! call `server.serve(stdio()).await`.
//!
//! Watcher integration is deferred to tier-10: the workspace hexagonal
//! invariant forbids cross-driving-adapter deps (mcp → watcher), so the
//! end-to-end orchestrator wiring the watcher loop sits in the CLI
//! (tier-10) where both adapters meet [src: tests/architecture.rs
//! lines 39 + 104-113].

use std::path::{Path, PathBuf};
use std::sync::Arc;

use ariadne_storage::RedbStorage;
use rmcp::ServiceExt;
use rmcp::transport::stdio;

use crate::catalog::Catalog;
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
/// Propagates storage open / catalog build / rmcp service errors.
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

/// Open storage + build the server. Exposed for in-process tests / benches.
///
/// # Errors
/// Propagates the underlying storage/catalog failures.
pub async fn build_server(opts: &ServeOpts) -> Result<AriadneServer, McpError> {
    let storage_path = opts.storage_path();
    let storage = open_storage(&storage_path)?;
    let root_str = opts.root.to_string_lossy().into_owned();
    let catalog = Catalog::build(&*storage, root_str)?;
    Ok(AriadneServer::new(storage, catalog))
}

fn open_storage(path: &Path) -> Result<Arc<RedbStorage>, McpError> {
    let storage = RedbStorage::open(path).map_err(McpError::Storage)?;
    Ok(Arc::new(storage))
}
