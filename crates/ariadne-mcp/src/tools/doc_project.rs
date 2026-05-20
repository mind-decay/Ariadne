//! `doc_for_project` — Markdown architecture overview for the project.

use ariadne_core::Storage;

use crate::catalog::Catalog;
use crate::errors::McpError;
use crate::tools::coupling_report::build_modules;
use crate::types::{DocOutput, ScopeInput};

/// Render the project architecture overview, optionally restricted to a
/// path prefix.
///
/// # Errors
/// [`McpError::Storage`] / [`McpError::Graph`] on snapshot or render
/// failure.
pub fn handle<S: Storage>(
    cat: &Catalog,
    storage: &S,
    scope: &ScopeInput,
) -> Result<DocOutput, McpError> {
    let modules = build_modules(cat, scope.prefix.as_deref());
    let snap = storage.snapshot().map_err(McpError::Storage)?;
    let markdown =
        ariadne_graph::docgen::for_project(&cat.graph, &snap, &modules).map_err(McpError::Graph)?;
    Ok(DocOutput { markdown })
}
