//! `blast_radius` — wraps `ariadne_graph::GraphIndex::blast_radius` with
//! name-resolution + wire-shape conversion. Block 1 tier-03 caps `must_touch`
//! and `may_touch` independently, sharing ONE opaque multi-list cursor, and
//! projects rows at the requested verbosity (concise default) — all via the
//! shared `ariadne_graph::economy` helper so the cold and warm paths stay
//! byte-identical [src: .claude/plans/data-fidelity-arc/block-1/plan.md D1-D5].

use std::cmp::Ordering;

use ariadne_graph::EdgeKindSet;
use ariadne_graph::economy::{self, Budget, Verbosity};

use crate::catalog::Catalog;
use crate::errors::McpError;
use crate::tools::summarize;
use crate::types::{
    BlastRadiusInput, BlastRadiusOutput, EdgeKindFilter, SymbolSummary, Verbosity as WireVerbosity,
};

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
/// filtered to the kinds in `input.kinds` (all kinds when missing), with
/// `must_touch` / `may_touch` each capped to one page sharing a single
/// multi-list cursor and projected at `input.verbosity`.
///
/// # Errors
/// Returns [`McpError::NotFound`] when `input.symbol` is unknown, or
/// [`McpError::InvalidInput`] when `input.cursor` is malformed or was minted
/// against a different index revision.
pub fn handle(cat: &Catalog, input: &BlastRadiusInput) -> Result<BlastRadiusOutput, McpError> {
    let id = cat
        .find_symbol(&input.symbol)
        .ok_or_else(|| McpError::NotFound(format!("symbol {}", input.symbol)))?;
    let depth = input.depth.unwrap_or(DEFAULT_DEPTH).max(1);
    let kinds = filter_to_set(input.kinds.as_deref().unwrap_or(&[]));
    let radius = cat
        .graph
        .blast_radius(id, depth, kinds)
        .ok_or_else(|| McpError::NotFound(format!("symbol {} absent from graph", input.symbol)))?;
    let must: Vec<SymbolSummary> = radius
        .must_touch
        .into_iter()
        .map(|s| summarize(cat, s))
        .collect();
    let may: Vec<SymbolSummary> = radius
        .may_touch
        .into_iter()
        .map(|s| summarize(cat, s))
        .collect();
    let depth_used = radius.depth_used;
    let symbol = summarize(cat, id);

    let revision = u32::try_from(cat.revision).unwrap_or(u32::MAX);
    let cursor = input
        .cursor
        .as_deref()
        .map(|c| economy::Cursor::decode(c, revision))
        .transpose()
        .map_err(|e| McpError::InvalidInput(e.to_string()))?;
    Ok(page(symbol, must, may, depth_used, cursor, input, revision))
}

/// Map the MCP-facing verbosity onto the economy use case's verbosity.
fn to_economy(v: WireVerbosity) -> Verbosity {
    match v {
        WireVerbosity::Concise => Verbosity::Concise,
        WireVerbosity::Detailed => Verbosity::Detailed,
    }
}

/// Stable order for a dependent page: by file, then byte offset, then name — a
/// meaningful, deterministic top-N independent of graph order (D4). Read before
/// any concise projection nulls `byte_start`, so the offset is `Some`.
fn cmp_sym(a: &SymbolSummary, b: &SymbolSummary) -> Ordering {
    a.file
        .cmp(&b.file)
        .then(a.byte_start.cmp(&b.byte_start))
        .then(a.name.cmp(&b.name))
}

/// Drop the cryptic id/offset fields in concise verbosity (D3).
fn project(mut sym: SymbolSummary, verbosity: Verbosity) -> SymbolSummary {
    if matches!(verbosity, Verbosity::Concise) {
        sym.id = None;
        sym.byte_start = None;
        sym.byte_end = None;
    }
    sym
}

/// Sort, cap, project, and steer the two dependent lists behind one multi-list
/// cursor. Shared shape with the warm daemon handler so their JSON is
/// byte-identical (parity).
// Each parameter is a distinct piece of the already-computed radius the page
// assembles; bundling them into a struct would only add indirection.
#[allow(clippy::too_many_arguments)]
fn page(
    symbol: SymbolSummary,
    must: Vec<SymbolSummary>,
    may: Vec<SymbolSummary>,
    depth_used: u8,
    cursor: Option<economy::Cursor>,
    input: &BlastRadiusInput,
    revision: u32,
) -> BlastRadiusOutput {
    let verbosity = to_economy(input.verbosity);
    let budget = Budget {
        limit: input.limit.map_or(economy::DEFAULT_PAGE, |l| l as usize),
        cursor,
        verbosity,
    };
    let total_must = must.len();
    let total_may = may.len();
    let must_page = economy::paginate_sublist(must, cmp_sym, &budget, 0);
    let may_page = economy::paginate_sublist(may, cmp_sym, &budget, 1);
    let next_cursor = economy::multi_cursor(
        &[
            (must_page.next_offset, must_page.remainder),
            (may_page.next_offset, may_page.remainder),
        ],
        revision,
    );
    let mut truncated = Vec::new();
    if must_page.remainder {
        truncated.push((must_page.rows.len(), total_must, "must_touch"));
    }
    if may_page.remainder {
        truncated.push((may_page.rows.len(), total_may, "may_touch"));
    }
    let note = next_cursor
        .as_ref()
        .map(|_| economy::multi_truncation_note(&truncated));
    BlastRadiusOutput {
        symbol: project(symbol, verbosity),
        must_touch: must_page
            .rows
            .into_iter()
            .map(|s| project(s, verbosity))
            .collect(),
        may_touch: may_page
            .rows
            .into_iter()
            .map(|s| project(s, verbosity))
            .collect(),
        depth_used,
        next_cursor,
        note,
    }
}
