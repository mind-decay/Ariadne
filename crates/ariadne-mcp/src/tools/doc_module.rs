//! `doc_for_module` — Markdown documentation summary for one module.
//!
//! Tier-08 has no first-class module taxonomy, so a module is one source
//! file (matches the per-file-unit boundary from D12); the input path is
//! the module identity.

use ariadne_core::Storage;
use ariadne_graph::DocScope;

use crate::catalog::Catalog;
use crate::errors::McpError;
use crate::tools::coupling_report::build_modules;
use crate::types::{DocOutput, FileQuery};

/// Render the module documentation for the file at `input.path`.
///
/// # Errors
/// [`McpError::NotFound`] when the path is not an indexed module;
/// [`McpError::Storage`] / [`McpError::Graph`] on snapshot or render
/// failure.
pub fn handle<S: Storage>(
    cat: &Catalog,
    storage: &S,
    input: &FileQuery,
) -> Result<DocOutput, McpError> {
    let modules = build_modules(cat, None);
    let module = modules
        .iter()
        .find(|m| m.name == input.path)
        .ok_or_else(|| McpError::NotFound(format!("module {}", input.path)))?;
    let snap = storage.snapshot().map_err(McpError::Storage)?;
    let markdown = ariadne_graph::docgen::for_module(
        &cat.graph,
        &snap,
        module,
        &cat.churn,
        &DocScope::default(),
    )
    .map_err(McpError::Graph)?;
    Ok(DocOutput { markdown })
}
