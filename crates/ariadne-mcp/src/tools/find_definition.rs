//! `find_definition` — exact canonical-name lookup.

use crate::catalog::Catalog;
use crate::errors::McpError;
use crate::tools::summarize;
use crate::types::{SymbolQuery, SymbolSummary};

/// Resolve `input.symbol` to its [`SymbolSummary`].
///
/// # Errors
/// Returns [`McpError::NotFound`] when no symbol carries the queried
/// canonical name.
pub fn handle(cat: &Catalog, input: &SymbolQuery) -> Result<SymbolSummary, McpError> {
    let id = cat
        .find_symbol(&input.symbol)
        .ok_or_else(|| McpError::NotFound(format!("symbol {}", input.symbol)))?;
    Ok(summarize(cat, id))
}
