//! `blast_radius` — wraps `ariadne_graph::GraphIndex::blast_radius` with
//! name-resolution + wire-shape conversion.

use ariadne_graph::EdgeKindSet;

use crate::catalog::Catalog;
use crate::errors::McpError;
use crate::tools::summarize;
use crate::types::{BlastRadiusInput, BlastRadiusOutput, EdgeKindFilter};

const DEFAULT_DEPTH: u8 = 3;

fn filter_to_set(filter: &[EdgeKindFilter]) -> EdgeKindSet {
    if filter.is_empty() {
        return EdgeKindSet::ALL;
    }
    let mut set = EdgeKindSet::empty();
    for f in filter {
        set |= match f {
            EdgeKindFilter::Calls => EdgeKindSet::CALLS,
            EdgeKindFilter::Imports => EdgeKindSet::IMPORTS,
            EdgeKindFilter::TypeOf => EdgeKindSet::TYPE_OF,
            EdgeKindFilter::Defines => EdgeKindSet::DEFINES,
            EdgeKindFilter::Overrides => EdgeKindSet::OVERRIDES,
            EdgeKindFilter::Reads => EdgeKindSet::READS,
            EdgeKindFilter::Writes => EdgeKindSet::WRITES,
            EdgeKindFilter::Inherits => EdgeKindSet::INHERITS,
        };
    }
    set
}

/// Compute the blast radius of `input.symbol` at hop limit `input.depth`,
/// filtered to the kinds in `input.kinds` (all kinds when missing).
///
/// # Errors
/// Returns [`McpError::NotFound`] when `input.symbol` is unknown.
pub fn handle(cat: &Catalog, input: &BlastRadiusInput) -> Result<BlastRadiusOutput, McpError> {
    let id = cat
        .find_symbol(&input.symbol)
        .ok_or_else(|| McpError::NotFound(format!("symbol {}", input.symbol)))?;
    let depth = input.depth.unwrap_or(DEFAULT_DEPTH).max(1);
    let kinds = filter_to_set(input.kinds.as_deref().unwrap_or(&[]));
    let radius = cat.graph.blast_radius(id, depth, kinds);
    Ok(BlastRadiusOutput {
        must_touch: radius
            .must_touch
            .into_iter()
            .map(|s| summarize(cat, s))
            .collect(),
        may_touch: radius
            .may_touch
            .into_iter()
            .map(|s| summarize(cat, s))
            .collect(),
        depth_used: radius.depth_used,
    })
}
