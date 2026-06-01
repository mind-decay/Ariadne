//! Change-coupling (logical-coupling) use case (tier-13).
//!
//! Surfaces files that change together despite no static edge. The degree is
//! code-maat's `shared-revs / average(revs_a, revs_b)`, gated by code-maat's
//! three filters — `min_revs` (per-entity revisions), `min_shared_commits`
//! (support), and `min_degree` (coupling strength) [src: post-v1-roadmap
//! plan.md RD7 + tier-13 D3;
//! <https://github.com/adamtornhill/code-maat/blob/master/src/code_maat/analysis/logical_coupling.clj>;
//! <https://github.com/adamtornhill/code-maat/blob/master/README.md>].
//!
//! Pure and deterministic: a free function over owned inputs, no clock and no
//! RNG, output total-ordered (degree descending, key ascending) so a re-run is
//! byte-identical (D4). `degree ∈ [0, 1]` since `shared ≤ min(revs) ≤ mean(revs)`.

use std::collections::BTreeMap;

use ariadne_core::{CoChangePair, FileChurn};

/// code-maat's three coupling filters. `Default` mirrors code-maat's published
/// defaults: `min_revs = 5`, `min_shared_commits = 5`, `min_degree = 0.30`
/// [src: tier-13 D3].
#[derive(Debug, Clone, PartialEq)]
pub struct CoChangeConfig {
    /// Minimum individual revisions for an entity to be eligible; an endpoint
    /// below this excludes the pair.
    pub min_revs: u32,
    /// Minimum shared-commit count (support) for a pair to be reported.
    pub min_shared_commits: u32,
    /// Minimum degree (coupling strength) ∈ [0, 1] for a pair to be reported.
    pub min_degree: f32,
}

impl Default for CoChangeConfig {
    fn default() -> Self {
        Self {
            min_revs: 5,
            min_shared_commits: 5,
            min_degree: 0.30,
        }
    }
}

/// One reported coupling edge between two files. `a`/`b` are copied from the
/// input [`CoChangePair`] (already canonical, `a < b`).
#[derive(Debug, Clone, PartialEq)]
pub struct CoChangeEdge {
    /// Lexicographically-smaller path of the pair.
    pub a: String,
    /// Lexicographically-larger path of the pair.
    pub b: String,
    /// Commits that changed both files (the pair's support).
    pub shared_commits: u32,
    /// Coupling degree `shared / mean(revs_a, revs_b)` ∈ [0, 1].
    pub degree: f32,
}

/// Reported coupling edges, degree descending with ties broken by key ascending
/// (`a`, then `b`) (D4).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CoChangeReport {
    /// Coupling edges that cleared all three filters.
    pub edges: Vec<CoChangeEdge>,
}

/// Report logical-coupling edges. For each [`CoChangePair`], looks up both
/// endpoints' revisions in `churn`; skips the pair if either endpoint is
/// missing or below `cfg.min_revs`, or the shared count is below
/// `cfg.min_shared_commits`; computes `degree = count / mean(revs_a, revs_b)`
/// and keeps the pair when `degree >= cfg.min_degree` (D3). The f64→f32
/// narrowing on a value in [0, 1] loses precision only past ~7 decimal digits,
/// mirroring `coupling.rs:117-120`.
#[must_use]
#[allow(clippy::cast_possible_truncation)]
pub fn co_change_report(
    churn: &[FileChurn],
    pairs: &[CoChangePair],
    cfg: &CoChangeConfig,
) -> CoChangeReport {
    let revs: BTreeMap<&str, u32> = churn.iter().map(|c| (c.path.as_str(), c.commits)).collect();
    let mut edges = Vec::new();
    for p in pairs {
        let (Some(&ra), Some(&rb)) = (revs.get(p.a.as_str()), revs.get(p.b.as_str())) else {
            continue;
        };
        if ra < cfg.min_revs || rb < cfg.min_revs || p.count < cfg.min_shared_commits {
            continue;
        }
        // f64 midpoint = mean(revs_a, revs_b) without intermediate overflow.
        let degree = f64::from(p.count) / f64::midpoint(f64::from(ra), f64::from(rb));
        if degree < f64::from(cfg.min_degree) {
            continue;
        }
        edges.push(CoChangeEdge {
            a: p.a.clone(),
            b: p.b.clone(),
            shared_commits: p.count,
            degree: degree as f32,
        });
    }
    edges.sort_by(|x, y| {
        y.degree
            .total_cmp(&x.degree)
            .then_with(|| x.a.cmp(&y.a))
            .then_with(|| x.b.cmp(&y.b))
    });
    CoChangeReport { edges }
}
