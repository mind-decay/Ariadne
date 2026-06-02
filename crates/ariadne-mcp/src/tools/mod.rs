//! Per-tool handlers. Each `<name>.rs` exposes a single `handle` function
//! that turns the typed input into the typed output by querying the
//! [`Catalog`]. The `#[tool]` methods on
//! `AriadneServer` delegate one-line so the macro-expanded router stays
//! small.

pub mod blast_radius;
pub mod co_change;
pub mod complexity;
pub mod coupling_report;
pub mod doc_for;
pub mod doc_module;
pub mod doc_project;
pub mod file_summary;
pub mod find_definition;
pub mod find_references;
pub mod hotspots;
pub mod list_symbols;
pub mod plan_assist;
pub mod project_status;
pub mod refactor;
pub mod weak_spots;

use ariadne_core::SymbolId;

use crate::catalog::Catalog;
use crate::types::SymbolSummary;

/// Convert a raw [`SymbolId`] into the wire [`SymbolSummary`]. Unknown
/// ids (possible only when a tool synthesises an id outside the catalog)
/// collapse into an "unknown" placeholder.
#[must_use]
pub fn summarize(cat: &Catalog, id: SymbolId) -> SymbolSummary {
    let meta = cat.meta_of(id);
    let (name, kind, file, byte_start, byte_end) = match meta {
        Some(m) => {
            let file = cat.path_of(m.file).unwrap_or("").to_owned();
            (
                m.name.clone(),
                m.kind.clone(),
                file,
                m.byte_start,
                m.byte_end,
            )
        }
        None => (
            String::from("<unknown>"),
            String::new(),
            String::new(),
            0,
            0,
        ),
    };
    SymbolSummary {
        id: id.get(),
        name,
        kind,
        file,
        byte_start,
        byte_end,
    }
}
