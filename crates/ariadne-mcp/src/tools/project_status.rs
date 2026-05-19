//! `project_status` — coarse counts + persisted revision.

use crate::catalog::Catalog;
use crate::types::ProjectStatusOutput;

/// Coarse revision/count summary of the indexed project.
#[must_use]
pub fn handle(cat: &Catalog) -> ProjectStatusOutput {
    ProjectStatusOutput {
        revision: cat.revision,
        file_count: u32::try_from(cat.paths.len()).unwrap_or(u32::MAX),
        symbol_count: u32::try_from(cat.symbols.len()).unwrap_or(u32::MAX),
        edge_count: u32::try_from(cat.graph.edge_count()).unwrap_or(u32::MAX),
        root: cat.root.clone(),
    }
}
