//! `find_references` — incoming-edge scan via the storage snapshot.
//!
//! Each row carries the caller's identity plus the source span of the
//! reference edge. Tier-08 follows every storage edge class — clients
//! filter downstream if they need finer slices. Block 1 tier-01 caps the
//! result to a default page, returns an opaque cursor for the remainder, and
//! projects rows at the requested verbosity (concise by default), all via the
//! shared `ariadne_graph::economy` helper so the cold and warm paths stay
//! byte-identical [src: .claude/plans/data-fidelity-arc/block-1/plan.md D1-D5].

use std::cmp::Ordering;
use std::collections::BTreeMap;

use ariadne_core::{ReadSnapshot, Storage};
use ariadne_graph::economy::{self, Budget, Verbosity};

use crate::catalog::Catalog;
use crate::errors::McpError;
use crate::types::{
    FindReferencesInput, FindReferencesOutput, ReferenceSite, Verbosity as WireVerbosity,
};

/// List the reference sites whose target is `input.symbol`, capped to one page.
///
/// # Errors
/// Returns [`McpError::NotFound`] when `input.symbol` is unknown,
/// [`McpError::Storage`] when the snapshot scan fails, or
/// [`McpError::InvalidInput`] when `input.cursor` is malformed or was minted
/// against a different index revision.
pub fn handle<S: Storage>(
    cat: &Catalog,
    storage: &S,
    input: &FindReferencesInput,
) -> Result<FindReferencesOutput, McpError> {
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
                caller: Some(key.src.get()),
                caller_name,
                file,
                byte_start: Some(rec.source_span.byte_start),
                byte_end: Some(rec.source_span.byte_end),
            },
        );
    }
    let rows: Vec<ReferenceSite> = by_caller.into_values().collect();
    // The cursor stamps the catalog revision as a u32 (D2); a revision that
    // never realistically exceeds u32 saturates rather than wrapping, and both
    // serving paths compute it identically so the stamp stays parity-stable.
    let revision = u32::try_from(cat.revision).unwrap_or(u32::MAX);
    // A malformed / stale cursor is a caller fault: surface it as invalid_params
    // (−32602) so the client re-queries instead of mis-paging (D2), never
    // silently wrong. The Display message matches the warm daemon path.
    let cursor = input
        .cursor
        .as_deref()
        .map(|c| economy::Cursor::decode(c, revision))
        .transpose()
        .map_err(|e| McpError::InvalidInput(e.to_string()))?;
    Ok(page(rows, cursor, input, revision))
}

/// Map the MCP-facing verbosity onto the economy use case's verbosity.
fn to_economy(v: WireVerbosity) -> Verbosity {
    match v {
        WireVerbosity::Concise => Verbosity::Concise,
        WireVerbosity::Detailed => Verbosity::Detailed,
    }
}

/// Stable order for a reference page: by file, then byte offset, then caller
/// name — a meaningful, deterministic top-N independent of graph order (D4).
fn cmp_site(a: &ReferenceSite, b: &ReferenceSite) -> Ordering {
    a.file
        .cmp(&b.file)
        .then(a.byte_start.cmp(&b.byte_start))
        .then(a.caller_name.cmp(&b.caller_name))
}

/// Sort, cap, project, and steer one page of reference rows. Shared shape with
/// the warm daemon handler so their JSON is byte-identical (parity).
fn page(
    rows: Vec<ReferenceSite>,
    cursor: Option<economy::Cursor>,
    input: &FindReferencesInput,
    revision: u32,
) -> FindReferencesOutput {
    let verbosity = to_economy(input.verbosity);
    let budget = Budget {
        limit: input.limit.map_or(economy::DEFAULT_PAGE, |l| l as usize),
        cursor,
        verbosity,
    };
    let total = rows.len();
    let paged = economy::paginate(rows, cmp_site, &budget, revision, 0);
    let references: Vec<ReferenceSite> = paged
        .rows
        .into_iter()
        .map(|r| project(r, verbosity))
        .collect();
    let note = paged
        .next_cursor
        .as_ref()
        .map(|_| economy::truncation_note(references.len(), total, "references"));
    FindReferencesOutput {
        references,
        next_cursor: paged.next_cursor,
        note,
    }
}

/// Drop the cryptic id/offset fields in concise verbosity (D3).
fn project(mut site: ReferenceSite, verbosity: Verbosity) -> ReferenceSite {
    if matches!(verbosity, Verbosity::Concise) {
        site.caller = None;
        site.byte_start = None;
        site.byte_end = None;
    }
    site
}
