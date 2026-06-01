//! Hotspot ranking use cases (tier-13).
//!
//! A hotspot is code that is *both* frequently changed *and* complex — the
//! overlap `CodeScene` and Tornhill describe as the strongest predictor of
//! defect/maintenance cost [src: post-v1-roadmap plan.md RD8 + tier-13 D2;
//! <https://docs.enterprise.codescene.io/versions/4.0.16/guides/technical/hotspots.html>;
//! Tornhill, "Your Code as a Crime Scene", 2015].
//!
//! Each factor is max-normalized over the input set (`x / max(x)`, `0` when
//! `max == 0`) and the score is their product `norm_churn * norm_complexity`
//! ∈ [0, 1]; a zero in either factor forces the score to `0`, encoding the AND
//! exactly. Pure and deterministic: free functions over owned inputs, no clock
//! and no RNG, output total-ordered (score descending, key ascending) so a
//! re-run is byte-identical (D4), mirroring `attribute_symbol_churn` [src:
//! crates/ariadne-graph/src/symbol_churn.rs:56-106].

use std::collections::BTreeMap;

use ariadne_core::{FileChurn, SymbolChurn, SymbolId};

/// Which grain a [`HotspotEntry`] ranks, carrying its identifying key. A report
/// holds entries of a single grain, so ordering by `grain` breaks score ties by
/// key ascending (path, then [`SymbolId`]) (D4).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum HotspotGrain {
    /// File-level hotspot keyed by repository-root-relative path.
    File {
        /// Repository-root-relative path of the file.
        path: String,
    },
    /// Symbol-level hotspot keyed by [`SymbolId`].
    Symbol {
        /// The symbol the entry ranks.
        symbol: SymbolId,
    },
}

/// One ranked unit: its identifying [`HotspotGrain`], the raw churn and
/// complexity it was scored from, and the product score ∈ [0, 1] (D2).
#[derive(Debug, Clone, PartialEq)]
pub struct HotspotEntry {
    /// Grain + identifying key of the ranked unit.
    pub grain: HotspotGrain,
    /// Raw churn (commits touching the unit) before normalization.
    pub churn: u32,
    /// Raw complexity (`McCabe`, aggregated for files) before normalization.
    pub complexity: u32,
    /// `norm_churn * norm_complexity` ∈ [0, 1]; `0` when either factor is `0`.
    pub score: f32,
}

/// Ranked hotspots, score descending with ties broken by key ascending (D4).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct HotspotReport {
    /// Ranked entries; first is the strongest hotspot.
    pub entries: Vec<HotspotEntry>,
}

/// Max-normalize `value` over `max`: `value / max` in f64, or `0` when `max`
/// is `0` (an all-zero factor contributes nothing).
fn norm(value: u32, max: u32) -> f64 {
    if max == 0 {
        0.0
    } else {
        f64::from(value) / f64::from(max)
    }
}

/// Score one grain's units: max-normalize churn and complexity over the input
/// set, set `score = norm_churn * norm_complexity`, sort per D4. The f64→f32
/// narrowing only loses precision past ~7 decimal digits on a value in [0, 1]
/// — acceptable for a presentation metric, mirroring `coupling.rs:117-120`.
#[allow(clippy::cast_possible_truncation)]
fn rank(units: Vec<(HotspotGrain, u32, u32)>) -> HotspotReport {
    let max_churn = units.iter().map(|u| u.1).max().unwrap_or(0);
    let max_complexity = units.iter().map(|u| u.2).max().unwrap_or(0);
    let mut entries: Vec<HotspotEntry> = units
        .into_iter()
        .map(|(grain, churn, complexity)| {
            let score = (norm(churn, max_churn) * norm(complexity, max_complexity)) as f32;
            HotspotEntry {
                grain,
                churn,
                complexity,
                score,
            }
        })
        .collect();
    entries.sort_by(|a, b| {
        b.score
            .total_cmp(&a.score)
            .then_with(|| a.grain.cmp(&b.grain))
    });
    HotspotReport { entries }
}

/// Rank files by churn × complexity. Churn is [`FileChurn::commits`];
/// complexity is the file's entry in `file_complexity` (the Σ of its symbols'
/// `McCabe` complexity, aggregated by the composition root in tier-15), or `0`
/// when absent. One entry per [`FileChurn`].
#[must_use]
pub fn file_hotspots(
    churn: &[FileChurn],
    file_complexity: &BTreeMap<String, u32>,
) -> HotspotReport {
    let units = churn
        .iter()
        .map(|c| {
            let complexity = file_complexity.get(&c.path).copied().unwrap_or(0);
            (
                HotspotGrain::File {
                    path: c.path.clone(),
                },
                c.commits,
                complexity,
            )
        })
        .collect();
    rank(units)
}

/// Rank symbols by churn × complexity. Churn is [`SymbolChurn::commits`];
/// complexity is the symbol's entry in `symbol_complexity` (its `McCabe`
/// complexity), or `0` when absent. One entry per [`SymbolChurn`].
#[must_use]
pub fn symbol_hotspots(
    churn: &[SymbolChurn],
    symbol_complexity: &BTreeMap<SymbolId, u32>,
) -> HotspotReport {
    let units = churn
        .iter()
        .map(|c| {
            let complexity = symbol_complexity.get(&c.symbol).copied().unwrap_or(0);
            (
                HotspotGrain::Symbol { symbol: c.symbol },
                c.commits,
                complexity,
            )
        })
        .collect();
    rank(units)
}
