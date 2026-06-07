//! API-surface semver classifier (block A, A2).
//!
//! [`api_surface_diff`] classifies the public-surface delta between two refs as
//! a [`SemverBump`] per the Cargo `SemVer` taxonomy: a removed or
//! signature-changed public item is a major (breaking) change, an added one is
//! a minor change, and no surface change is none [src:
//! <https://doc.rust-lang.org/cargo/reference/semver.html>].
//!
//! Pure and deterministic: it is a total function over two [`PublicSymbol`]
//! lists with no clock, RNG, or IO, and every output collection is sorted by
//! `(name, kind)` so re-runs are byte-identical. The inputs are produced by the
//! `ariadne-parser` public-surface extractor on both refs through the SAME
//! tree-sitter path, so the comparison is header-for-header with no phantom
//! visibility diffs [src: .claude/plans/intelligence-platform/block-a/plan.md
//! D3/D6].

use ariadne_core::PublicSymbol;

/// `SemVer` bump implied by a public-surface delta, ordered
/// `None < Patch < Minor < Major` so the verdict is the maximum bump over every
/// delta (derived [`Ord`] follows declaration order) [src:
/// <https://doc.rust-lang.org/cargo/reference/semver.html>].
///
/// [`SemverBump::Patch`] is part of the taxonomy but never emitted by
/// [`api_surface_diff`]: this surface model classifies only additions,
/// removals, and signature changes, none of which is a patch-level change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SemverBump {
    /// No public-surface change.
    None,
    /// A backward-compatible change with no surface delta (never emitted here).
    Patch,
    /// A backward-compatible addition (a new public item).
    Minor,
    /// A breaking change (a removed or signature-changed public item).
    Major,
}

/// One public symbol whose declaration header changed between the two refs:
/// same identity `(name, kind)`, differing [`PublicSymbol::signature`]. Both
/// header texts are carried so the report reads as a before→after diff.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignatureChange {
    /// Declared identifier name (the changed item's identity, with `kind`).
    pub name: String,
    /// Free-form kind tag (the changed item's identity, with `name`).
    pub kind: String,
    /// Whitespace-normalized declaration header on the base ref.
    pub base_signature: String,
    /// Whitespace-normalized declaration header on the head ref.
    pub head_signature: String,
}

/// The classified public-surface delta between two refs.
///
/// `added`/`removed` are the [`PublicSymbol`]s present on only one side;
/// `changed` are the items present on both whose signature differs. All three
/// are sorted by `(name, kind)`. `verdict` is the maximum bump over every delta.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiDiffReport {
    /// Overall verdict: the maximum bump implied by any delta.
    pub verdict: SemverBump,
    /// Public items present on the head ref but not the base ref (each a minor
    /// bump), sorted by `(name, kind)`.
    pub added: Vec<PublicSymbol>,
    /// Public items present on the base ref but not the head ref (each a major
    /// bump), sorted by `(name, kind)`.
    pub removed: Vec<PublicSymbol>,
    /// Public items present on both refs whose signature changed (each a major
    /// bump), sorted by `(name, kind)`.
    pub changed: Vec<SignatureChange>,
}

/// Identity key for a public symbol: `(name, kind)`. Two refs' surfaces compare
/// position-free, so identity is the declared name plus its kind tag, never the
/// source order or span [src: block-a plan.md step 2].
fn key(symbol: &PublicSymbol) -> (&str, &str) {
    (symbol.name.as_str(), symbol.kind.as_str())
}

/// Classify the public-surface delta from `base` to `head`.
///
/// Identity is `(name, kind)`. An item only in `head` is `added` (minor); only
/// in `base` is `removed` (major); in both with a differing
/// [`PublicSymbol::signature`] is `changed` (major). The `verdict` is the
/// maximum bump over every delta — `Major` if anything is removed or changed,
/// else `Minor` if anything is added, else `None`. Because the inputs hold only
/// [`ariadne_core::Visibility::Public`] symbols, a visibility narrowing drops a
/// symbol from `head` and therefore surfaces as a removal [src: block-a plan.md
/// step 2; <https://doc.rust-lang.org/cargo/reference/semver.html>].
///
/// Deterministic: `added`, `removed`, and `changed` are each sorted by
/// `(name, kind)`, so the report is byte-identical across runs on the same
/// inputs.
#[must_use]
pub fn api_surface_diff(base: &[PublicSymbol], head: &[PublicSymbol]) -> ApiDiffReport {
    use std::collections::BTreeMap;

    // `(name, kind)` → symbol, keyed for O(log n) cross-side lookup. The
    // BTreeMap also gives a sorted iteration order, so the derived lists need no
    // extra sort pass for the common single-symbol-per-key surface.
    let base_by_key: BTreeMap<(&str, &str), &PublicSymbol> =
        base.iter().map(|s| (key(s), s)).collect();
    let head_by_key: BTreeMap<(&str, &str), &PublicSymbol> =
        head.iter().map(|s| (key(s), s)).collect();

    // Removed: in base, absent from head (a breaking change).
    let mut removed: Vec<PublicSymbol> = base_by_key
        .iter()
        .filter(|(k, _)| !head_by_key.contains_key(*k))
        .map(|(_, s)| (*s).clone())
        .collect();

    // Added: in head, absent from base (a minor change). Changed: in both with
    // a differing header (a breaking change).
    let mut added: Vec<PublicSymbol> = Vec::new();
    let mut changed: Vec<SignatureChange> = Vec::new();
    for (k, h) in &head_by_key {
        match base_by_key.get(k) {
            None => added.push((*h).clone()),
            Some(b) if b.signature != h.signature => changed.push(SignatureChange {
                name: h.name.clone(),
                kind: h.kind.clone(),
                base_signature: b.signature.clone(),
                head_signature: h.signature.clone(),
            }),
            Some(_) => {}
        }
    }

    // Deterministic order by (name, kind). BTreeMap iteration is already sorted,
    // but the sort is explicit so the guarantee survives any future change to
    // the collection step.
    added.sort_by(|a, b| key(a).cmp(&key(b)));
    removed.sort_by(|a, b| key(a).cmp(&key(b)));
    changed.sort_by(|a, b| {
        (a.name.as_str(), a.kind.as_str()).cmp(&(b.name.as_str(), b.kind.as_str()))
    });

    // Verdict = the maximum bump over every delta (Major > Minor > Patch > None).
    let verdict = removed
        .iter()
        .map(|_| SemverBump::Major)
        .chain(changed.iter().map(|_| SemverBump::Major))
        .chain(added.iter().map(|_| SemverBump::Minor))
        .max()
        .unwrap_or(SemverBump::None);

    ApiDiffReport {
        verdict,
        added,
        removed,
        changed,
    }
}

#[cfg(test)]
mod tests {
    use ariadne_core::{PublicSymbol, Visibility};

    use super::{ApiDiffReport, SemverBump, SignatureChange, api_surface_diff};

    /// Build a public `PublicSymbol` with the given name and header.
    fn sym(name: &str, signature: &str) -> PublicSymbol {
        PublicSymbol {
            name: name.to_owned(),
            kind: "function".to_owned(),
            visibility: Visibility::Public,
            signature: signature.to_owned(),
        }
    }

    #[test]
    fn identical_surface_is_none() {
        // TDD anchor (step 1): no surface delta → None, all lists empty.
        let base = [sym("a", "fn a()"), sym("b", "fn b()")];
        let head = [sym("a", "fn a()"), sym("b", "fn b()")];
        let report = api_surface_diff(&base, &head);
        assert_eq!(
            report,
            ApiDiffReport {
                verdict: SemverBump::None,
                added: vec![],
                removed: vec![],
                changed: vec![],
            }
        );
    }

    #[test]
    fn removed_public_symbol_is_major() {
        // In base, absent from head → removed → Major.
        let base = [sym("a", "fn a()"), sym("gone", "fn gone()")];
        let head = [sym("a", "fn a()")];
        let report = api_surface_diff(&base, &head);
        assert_eq!(report.verdict, SemverBump::Major);
        assert_eq!(report.removed, vec![sym("gone", "fn gone()")]);
        assert!(report.added.is_empty());
        assert!(report.changed.is_empty());
    }

    #[test]
    fn added_public_symbol_is_minor() {
        // In head, absent from base → added → Minor.
        let base = [sym("a", "fn a()")];
        let head = [sym("a", "fn a()"), sym("new", "fn new()")];
        let report = api_surface_diff(&base, &head);
        assert_eq!(report.verdict, SemverBump::Minor);
        assert_eq!(report.added, vec![sym("new", "fn new()")]);
        assert!(report.removed.is_empty());
        assert!(report.changed.is_empty());
    }

    #[test]
    fn signature_changed_public_symbol_is_major() {
        // Same identity (name, kind), differing header → changed → Major.
        let base = [sym("a", "fn a(x: u32)")];
        let head = [sym("a", "fn a(x: u32, y: u32)")];
        let report = api_surface_diff(&base, &head);
        assert_eq!(report.verdict, SemverBump::Major);
        assert_eq!(
            report.changed,
            vec![SignatureChange {
                name: "a".to_owned(),
                kind: "function".to_owned(),
                base_signature: "fn a(x: u32)".to_owned(),
                head_signature: "fn a(x: u32, y: u32)".to_owned(),
            }]
        );
        assert!(report.added.is_empty());
        assert!(report.removed.is_empty());
    }

    #[test]
    fn same_name_different_kind_is_not_a_change() {
        // Identity includes `kind`, so a `b` function removed and a `b` struct
        // added are a removal + an addition, never a signature change.
        let base = [PublicSymbol {
            name: "b".to_owned(),
            kind: "function".to_owned(),
            visibility: Visibility::Public,
            signature: "fn b()".to_owned(),
        }];
        let head = [PublicSymbol {
            name: "b".to_owned(),
            kind: "struct".to_owned(),
            visibility: Visibility::Public,
            signature: "struct b".to_owned(),
        }];
        let report = api_surface_diff(&base, &head);
        assert_eq!(report.verdict, SemverBump::Major);
        assert_eq!(report.removed.len(), 1, "the function is removed");
        assert_eq!(report.added.len(), 1, "the struct is added");
        assert!(report.changed.is_empty());
    }

    #[test]
    fn verdict_is_the_max_bump_over_mixed_deltas() {
        // One removed (Major), one added (Minor), one signature-changed (Major),
        // one unchanged → verdict is the max (Major) with the exact lists.
        let base = [
            sym("keep", "fn keep()"),
            sym("gone", "fn gone()"),
            sym("sig", "fn sig(x: u32)"),
        ];
        let head = [
            sym("keep", "fn keep()"),
            sym("sig", "fn sig(x: u64)"),
            sym("fresh", "fn fresh()"),
        ];
        let report = api_surface_diff(&base, &head);
        assert_eq!(report.verdict, SemverBump::Major);
        assert_eq!(report.added, vec![sym("fresh", "fn fresh()")]);
        assert_eq!(report.removed, vec![sym("gone", "fn gone()")]);
        assert_eq!(
            report.changed,
            vec![SignatureChange {
                name: "sig".to_owned(),
                kind: "function".to_owned(),
                base_signature: "fn sig(x: u32)".to_owned(),
                head_signature: "fn sig(x: u64)".to_owned(),
            }]
        );
    }

    #[test]
    fn added_only_with_no_breaking_change_is_minor() {
        // Only additions → Minor (additions never escalate past minor).
        let base = [sym("a", "fn a()")];
        let head = [sym("a", "fn a()"), sym("b", "fn b()"), sym("c", "fn c()")];
        let report = api_surface_diff(&base, &head);
        assert_eq!(report.verdict, SemverBump::Minor);
        assert_eq!(report.added, vec![sym("b", "fn b()"), sym("c", "fn c()")]);
    }

    #[test]
    fn output_lists_are_sorted_by_name_then_kind() {
        // Source order (zeta, alpha) differs from the (name, kind) order the
        // report must emit, proving the sort is applied for determinism.
        let base: [PublicSymbol; 0] = [];
        let head = [sym("zeta", "fn zeta()"), sym("alpha", "fn alpha()")];
        let report = api_surface_diff(&base, &head);
        let names: Vec<&str> = report.added.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, vec!["alpha", "zeta"]);
    }
}
