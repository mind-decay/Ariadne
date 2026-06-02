//! `co_change` — logical-coupling (change-coupling) edges honoring code-maat's
//! filters (tier-15b). Calls the pure tier-13 `co_change_report` use case over
//! the catalog's file churn + co-change pairs, resolving each optional
//! threshold against `CoChangeConfig::default()`. Logic identical to the
//! daemon `queries::analytics::co_change` so cold and warm JSON match
//! [src: crates/ariadne-graph/src/co_change.rs:74-95].

use ariadne_graph::{CoChangeConfig, co_change_report};

use crate::catalog::Catalog;
use crate::types::{CoChangeEdge, CoChangeInput, CoChangeOutput};

/// Whether `path` is in scope for an optional path prefix (`None` = all).
fn in_scope(path: &str, prefix: Option<&str>) -> bool {
    prefix.is_none_or(|p| path.starts_with(p))
}

/// Report logical-coupling edges honoring `input`'s thresholds, keeping an
/// edge when either endpoint is in `input.prefix` scope.
#[must_use]
pub fn handle(cat: &Catalog, input: &CoChangeInput) -> CoChangeOutput {
    let cfg = resolve_cfg(input);
    let prefix = input.prefix.as_deref();
    let report = co_change_report(&cat.churn, &cat.co_change, &cfg);
    let edges = report
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
    CoChangeOutput { edges }
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
