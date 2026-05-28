//! Project-meta query: `project_status` — coarse counts plus the persisted
//! revision the warm graph currently holds. Mirrors the v1 MCP
//! `project_status` tool against the [`WarmCatalog`]. The reported revision is
//! the warm graph's, after any staleness refresh the request triggered.

use ariadne_core::{DaemonResponse, ProjectStatusReport};

use crate::domain::catalog::WarmCatalog;

/// Coarse revision/count summary of the indexed project.
pub(crate) fn project_status(cat: &WarmCatalog) -> DaemonResponse {
    DaemonResponse::ProjectStatus(ProjectStatusReport {
        revision: cat.revision,
        file_count: u32::try_from(cat.paths.len()).unwrap_or(u32::MAX),
        symbol_count: u32::try_from(cat.symbols.len()).unwrap_or(u32::MAX),
        edge_count: u32::try_from(cat.graph.edge_count()).unwrap_or(u32::MAX),
        root: cat.root.clone(),
    })
}
