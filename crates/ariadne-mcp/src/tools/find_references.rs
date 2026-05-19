//! `find_references` — incoming-edge scan via the storage snapshot.
//!
//! Each row carries the caller's identity plus the source span of the
//! reference edge. Tier-08 follows every storage edge class — clients
//! filter downstream if they need finer slices.

use std::collections::BTreeMap;

use ariadne_core::{ReadSnapshot, Storage};

use crate::catalog::Catalog;
use crate::errors::McpError;
use crate::types::{ReferenceSite, SymbolQuery};

/// List the reference sites whose target is `input.symbol`.
///
/// # Errors
/// Returns [`McpError::NotFound`] when `input.symbol` is unknown, or
/// [`McpError::Storage`] when the snapshot scan fails.
pub fn handle<S: Storage>(
    cat: &Catalog,
    storage: &S,
    input: &SymbolQuery,
) -> Result<Vec<ReferenceSite>, McpError> {
    let id = cat
        .find_symbol(&input.symbol)
        .ok_or_else(|| McpError::NotFound(format!("symbol {}", input.symbol)))?;
    let snap = storage.snapshot().map_err(McpError::Storage)?;
    let edges = snap.incoming_edges(id).map_err(McpError::Storage)?;
    let mut by_caller: BTreeMap<u64, ReferenceSite> = BTreeMap::new();
    for (key, rec) in edges {
        let caller_name = cat
            .meta_of(key.src)
            .map(|m| m.name.clone())
            .unwrap_or_default();
        let file = cat.path_of(rec.source_span.file).unwrap_or("").to_owned();
        by_caller.insert(
            key.src.get(),
            ReferenceSite {
                caller: key.src.get(),
                caller_name,
                file,
                byte_start: rec.source_span.byte_start,
                byte_end: rec.source_span.byte_end,
            },
        );
    }
    Ok(by_caller.into_values().collect())
}
