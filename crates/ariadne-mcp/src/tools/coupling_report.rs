//! `coupling_report` — Martin metrics with each file treated as a module.
//!
//! Tier-08 has no first-class module taxonomy; using files keeps the
//! tool useful immediately and matches the per-file-unit boundary the
//! plan adopts from Glean (D12). Block 1 tier-02 caps the result to a default
//! page, returns an opaque cursor for the remainder, and steers on truncation
//! via the shared `ariadne_graph::economy` helper so the cold and warm paths
//! stay byte-identical. The rows carry no cryptic fields, so concise ==
//! detailed — the cap is the only economy win [src: block-1 plan.md D1-D5].

use std::cmp::Ordering;
use std::collections::BTreeSet;

use ariadne_core::FileId;
use ariadne_graph::economy::{self, Budget, Verbosity};
use ariadne_graph::{CouplingMetrics, ModuleSpec};

use crate::catalog::Catalog;
use crate::errors::McpError;
use crate::types::{CouplingInput, CouplingOutput, CouplingRow, Verbosity as WireVerbosity};

/// Compute per-file coupling metrics filtered by `input.prefix`, capped to one
/// page in stable (Ca desc, module asc) order.
///
/// # Errors
/// Returns [`McpError::InvalidInput`] when `input.cursor` is malformed or was
/// minted against a different index revision.
pub fn handle(cat: &Catalog, input: &CouplingInput) -> Result<CouplingOutput, McpError> {
    let modules = build_modules(cat, input.prefix.as_deref());
    let report = cat.graph.coupling_report(&modules);
    let rows: Vec<CouplingRow> = report.rows.iter().map(to_row).collect();
    let revision = u32::try_from(cat.revision).unwrap_or(u32::MAX);
    let cursor = input
        .cursor
        .as_deref()
        .map(|c| economy::Cursor::decode(c, revision))
        .transpose()
        .map_err(|e| McpError::InvalidInput(e.to_string()))?;
    let budget = Budget {
        limit: input.limit.map_or(economy::DEFAULT_PAGE, |l| l as usize),
        cursor,
        verbosity: to_economy(input.verbosity),
    };
    let total = rows.len();
    let paged = economy::paginate(rows, cmp_row, &budget, revision, 0);
    let note = paged
        .next_cursor
        .as_ref()
        .map(|_| economy::truncation_note(paged.rows.len(), total, "modules"));
    Ok(CouplingOutput {
        rows: paged.rows,
        next_cursor: paged.next_cursor,
        note,
    })
}

/// Map the MCP-facing verbosity onto the economy use case's verbosity.
fn to_economy(v: WireVerbosity) -> Verbosity {
    match v {
        WireVerbosity::Concise => Verbosity::Concise,
        WireVerbosity::Detailed => Verbosity::Detailed,
    }
}

/// Stable order for a coupling page: most-depended-on first (afferent desc),
/// then module path ascending — a meaningful, deterministic top-N (D4).
fn cmp_row(a: &CouplingRow, b: &CouplingRow) -> Ordering {
    b.afferent
        .cmp(&a.afferent)
        .then_with(|| a.module.cmp(&b.module))
}

/// Project `Catalog` symbols into one `ModuleSpec` per file, optionally
/// gated by a path prefix.
#[must_use]
pub fn build_modules(cat: &Catalog, prefix: Option<&str>) -> Vec<ModuleSpec> {
    let mut by_file: std::collections::BTreeMap<FileId, BTreeSet<ariadne_core::SymbolId>> =
        std::collections::BTreeMap::new();
    for (sid, meta) in &cat.symbols {
        by_file.entry(meta.file).or_default().insert(*sid);
    }
    let mut out = Vec::with_capacity(by_file.len());
    for (fid, members) in by_file {
        let Some(path) = cat.path_of(fid) else {
            continue;
        };
        if let Some(p) = prefix {
            if !path.starts_with(p) {
                continue;
            }
        }
        out.push(ModuleSpec {
            name: path.to_owned(),
            members,
            abstract_members: BTreeSet::new(),
        });
    }
    out
}

fn to_row(metrics: &CouplingMetrics) -> CouplingRow {
    CouplingRow {
        module: metrics.name.clone(),
        afferent: metrics.afferent,
        efferent: metrics.efferent,
        instability: metrics.instability,
        abstractness: metrics.abstractness,
        distance: metrics.distance,
    }
}
