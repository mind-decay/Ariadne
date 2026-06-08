//! `co_change` — logical-coupling (change-coupling) edges honoring code-maat's
//! filters (tier-15b). Calls the pure tier-13 `co_change_report` use case over
//! the catalog's file churn + co-change pairs, resolving each optional
//! threshold against `CoChangeConfig::default()`. Logic identical to the
//! daemon `queries::analytics::co_change` so cold and warm JSON match
//! [src: crates/ariadne-graph/src/co_change.rs:74-95]. Block 1 tier-02 caps the
//! result to a default page + cursor via the shared `ariadne_graph::economy`
//! helper; edges carry no cryptic fields, so concise == detailed and the cap is
//! the only economy win [src: block-1 plan.md D1-D5].

use std::cmp::Ordering;

use ariadne_graph::economy::{self, Budget, Verbosity};
use ariadne_graph::{CoChangeConfig, co_change_report};

use crate::catalog::Catalog;
use crate::errors::McpError;
use crate::types::{CoChangeEdge, CoChangeInput, CoChangeOutput, Verbosity as WireVerbosity};

/// Whether `path` is in scope for an optional path prefix (`None` = all).
fn in_scope(path: &str, prefix: Option<&str>) -> bool {
    prefix.is_none_or(|p| path.starts_with(p))
}

/// Report logical-coupling edges honoring `input`'s thresholds (keeping an edge
/// when either endpoint is in `input.prefix` scope), capped to one page in
/// stable (degree desc, then `(a, b)` asc) order.
///
/// # Errors
/// Returns [`McpError::InvalidInput`] when `input.cursor` is malformed or was
/// minted against a different index revision.
pub fn handle(cat: &Catalog, input: &CoChangeInput) -> Result<CoChangeOutput, McpError> {
    let cfg = resolve_cfg(input);
    let prefix = input.prefix.as_deref();
    let report = co_change_report(&cat.churn, &cat.co_change, &cfg);
    let edges: Vec<CoChangeEdge> = report
        .edges
        .into_iter()
        .filter(|e| in_scope(&e.a, prefix) || in_scope(&e.b, prefix))
        .map(|e| CoChangeEdge {
            a: e.a,
            b: e.b,
            shared_commits: e.shared_commits,
            degree: e.degree,
        })
        .collect();
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
    let total = edges.len();
    let paged = economy::paginate(edges, cmp_edge, &budget, revision, 0);
    let note = paged
        .next_cursor
        .as_ref()
        .map(|_| economy::truncation_note(paged.rows.len(), total, "co-change pairs"));
    Ok(CoChangeOutput {
        edges: paged.rows,
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

/// Stable order for a co-change page: strongest coupling first (degree desc),
/// then the `(a, b)` path pair ascending — a meaningful, deterministic top-N
/// (D4). Degree is an `f32`; `total_cmp` gives a total order (no NaN in [0,1]).
fn cmp_edge(x: &CoChangeEdge, y: &CoChangeEdge) -> Ordering {
    y.degree
        .total_cmp(&x.degree)
        .then_with(|| (&x.a, &x.b).cmp(&(&y.a, &y.b)))
}

/// Resolve the three optional thresholds against `CoChangeConfig::default()`.
fn resolve_cfg(input: &CoChangeInput) -> CoChangeConfig {
    let d = CoChangeConfig::default();
    CoChangeConfig {
        min_revs: input.min_revs.unwrap_or(d.min_revs),
        min_shared_commits: input.min_shared_commits.unwrap_or(d.min_shared_commits),
        min_degree: input.min_degree.unwrap_or(d.min_degree),
    }
}
