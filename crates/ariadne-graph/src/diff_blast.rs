//! Diff-aware blast-radius use case (tier-14).
//!
//! v1 [`GraphIndex::blast_radius`](crate::GraphIndex::blast_radius) answers
//! "what depends on symbol X". A reviewer's real question is "what does *this
//! change* affect". This use case composes the two: it resolves a changeset's
//! line hunks to the changed-symbol seed set (via [`crate::span_lines`]), runs
//! v1 blast radius per seed, and returns the deduped must/may union plus the
//! changed paths that resolved to no symbol.
//!
//! The seed resolution reuses the shared span↔line↔overlap primitives (D1); the
//! diff itself (working tree, a commit, or a ref range) is produced by the
//! symbol-agnostic `ariadne-git` adapter and joined here, never in the adapter
//! (the ADR-0022 / ADR-0019 boundary).
//!
//! Pure and deterministic: no clock, no RNG; every output collection is sorted
//! (`seeds` by `SymbolId`, the unions by `SymbolId`, `unresolved` by path) so
//! re-runs are byte-identical [src:
//! .claude/plans/post-v1-roadmap/tier-14-diff-aware-blast-radius.md D4].

use std::collections::BTreeSet;

use ariadne_core::{LineHunk, SymbolId};

use crate::build::{EdgeKindSet, GraphIndex};
use crate::span_lines::{FileSymbolSpans, changed_symbols};

/// One changed symbol's blast radius, mirroring v1
/// [`BlastRadius`](crate::BlastRadius) plus the seed it was computed for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffSeed {
    /// The changed symbol the radius was seeded from.
    pub symbol: SymbolId,
    /// First-hop dependents of the seed (every caller funnel point).
    pub must_touch: Vec<SymbolId>,
    /// Transitive dependents beyond the first hop.
    pub may_touch: Vec<SymbolId>,
    /// Largest hop depth in this seed's returned set.
    pub depth_used: u8,
}

/// Result of [`GraphIndex::diff_blast`].
///
/// `seeds` lists each changed symbol's individual radius (sorted by
/// `SymbolId`). `must_touch`/`may_touch` are the deduped union across all
/// seeds: a symbol that is `must` for *any* seed lands in `must_touch`, every
/// other reached symbol in `may_touch` (the two are disjoint — must wins on
/// conflict). `unresolved` are changed paths that produced no symbol seed (new,
/// binary, or deleted files) — surfaced, never silently dropped.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DiffBlastReport {
    /// Per-seed radius for each changed symbol, sorted by `SymbolId`.
    pub seeds: Vec<DiffSeed>,
    /// Union of every seed's first-hop dependents.
    pub must_touch: Vec<SymbolId>,
    /// Union of every seed's transitive dependents, minus `must_touch`.
    pub may_touch: Vec<SymbolId>,
    /// Changed paths that resolved to no symbol seed, sorted.
    pub unresolved: Vec<String>,
}

impl GraphIndex {
    /// Compute the diff-aware blast radius of a changeset.
    ///
    /// `symbol_lines` carries the HEAD symbol spans per file (the indexed
    /// graph's view); `hunks` are the changeset's new-side changed line ranges
    /// and `changed_paths` its full changed-path list — both emitted by the
    /// `ariadne-git` adapter. Each hunk is resolved to the symbols whose
    /// defining span covers it (the seed set); each seed's
    /// [`blast_radius`](GraphIndex::blast_radius) is taken at `depth`/`kinds`
    /// and folded into the deduped must/may union. A changed path owning no
    /// seed symbol is reported in [`DiffBlastReport::unresolved`].
    ///
    /// Deterministic: the seed set, both unions, and `unresolved` are sorted, so
    /// the report is byte-identical across runs on the same inputs.
    // Each input is a distinct, plan-mandated facet of the query: the HEAD spans,
    // the changeset's hunks, its full changed-path list, and the v1 blast-radius
    // depth/kind filter. Bundling them would add an input type the tier does not
    // call for.
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub fn diff_blast(
        &self,
        symbol_lines: &[FileSymbolSpans],
        hunks: &[LineHunk],
        changed_paths: &[String],
        depth: u8,
        kinds: EdgeKindSet,
    ) -> DiffBlastReport {
        // Seed set: symbols whose HEAD span covers a changed line. `BTreeSet`
        // iteration is `SymbolId`-ordered, so `seeds` comes out sorted.
        let seed_set = changed_symbols(symbol_lines, hunks);

        let mut seeds = Vec::with_capacity(seed_set.len());
        let mut must_union: BTreeSet<SymbolId> = BTreeSet::new();
        let mut may_union: BTreeSet<SymbolId> = BTreeSet::new();
        for &symbol in &seed_set {
            // A seed always present in `symbol_lines` is normally a graph node;
            // an absent node yields an empty radius (no known dependents)
            // rather than dropping the seed.
            let radius = self.blast_radius(symbol, depth, kinds).unwrap_or_default();
            must_union.extend(radius.must_touch.iter().copied());
            may_union.extend(radius.may_touch.iter().copied());
            seeds.push(DiffSeed {
                symbol,
                must_touch: radius.must_touch,
                may_touch: radius.may_touch,
                depth_used: radius.depth_used,
            });
        }

        // Must wins on conflict: a symbol that is `must` for any seed is dropped
        // from the `may` union.
        let must_touch: Vec<SymbolId> = must_union.iter().copied().collect();
        let may_touch: Vec<SymbolId> = may_union.difference(&must_union).copied().collect();

        // A changed path is resolved when it owns at least one seed symbol;
        // everything else (new / binary / deleted files, or a change landing in
        // no symbol's span) is an unresolved-impact entry.
        let resolved_paths: BTreeSet<&str> = symbol_lines
            .iter()
            .filter(|file| file.symbols.iter().any(|(id, _, _)| seed_set.contains(id)))
            .map(|file| file.path.as_str())
            .collect();
        let mut unresolved: Vec<String> = changed_paths
            .iter()
            .filter(|path| !resolved_paths.contains(path.as_str()))
            .cloned()
            .collect();
        unresolved.sort();
        unresolved.dedup();

        DiffBlastReport {
            seeds,
            must_touch,
            may_touch,
            unresolved,
        }
    }
}
