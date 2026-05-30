//! Pure per-file derivation (tier-07a, RD11).
//!
//! Moved verbatim-in-behaviour out of the `ariadne-cli` driving adapter so the
//! cold index and the daemon warm graph share one derivation path
//! [src: crates/ariadne-cli/src/domain/mod.rs:497-768]. Two phases:
//!
//! * [`build_symbols`] — the memoized per-file step (called from the salsa
//!   tracked query [`crate::symbols_for_file`]): decls become symbols and an
//!   SFC file synthesizes its per-file `Component` symbol.
//! * [`resolve_edges`] — the pure driver pass (called from
//!   [`crate::AriadneDb::commit_revision`]): it needs every file's symbols, so
//!   it does not fit per-file memoization — it mirrors the CLI's two-phase
//!   structure [src: post-v1-roadmap plan.md RD11, R-B4].
//!
//! `decl_kind_tag` and the `SyntacticFacts -> SyntacticFactsRaw` conversion
//! stay at the composition root: they read `ariadne_parser::DeclKind`, and
//! `ariadne-salsa` may not depend on `ariadne-parser`
//! [src: tests/architecture.rs lines 30-33].

use std::collections::{HashMap, HashSet};
use std::path::Path;

use ariadne_core::{EdgeKey, EdgeKind, EdgeRecord, FileId, Lang, Span, SymbolId, Visibility};

use crate::derived::{SymbolFactsRaw, SyntacticFactsRaw};

/// A declaration promoted to a symbol, kept for edge resolution.
pub(crate) struct LocalSymbol {
    /// Resolved symbol id.
    pub id: SymbolId,
    /// `(byte_start, byte_end)` of the defining occurrence.
    pub def_range: (u32, u32),
}

/// A symbol-name candidate kept for deterministic edge-`dst` selection. The
/// candidate lists are sorted by `(file, def_start)` so edge-`dst` selection
/// is independent of file-iteration order
/// [src: crates/ariadne-cli/src/domain/mod.rs:167-171].
pub(crate) struct SymbolCandidate {
    /// Resolved symbol id.
    pub id: SymbolId,
    /// Defining file.
    pub file: FileId,
    /// Defining-occurrence byte start.
    pub def_start: u32,
}

/// Per-file facts retained between the symbol pass and the edge pass. Each
/// `(name, range)` pair is an unresolved site — a callee, a rendered child
/// component, or a hook — the edge pass resolves against the global symbol
/// table [src: crates/ariadne-cli/src/domain/mod.rs:177-184].
pub(crate) struct FileFacts {
    /// File the sites live in.
    pub file_id: FileId,
    /// Evidence language for resolved edges.
    pub lang: Lang,
    /// Local symbols (for the enclosing-symbol `src` lookup).
    pub symbols: Vec<LocalSymbol>,
    /// Call sites: `(callee, range)`.
    pub calls: Vec<(String, (u32, u32))>,
    /// Render sites: `(component, range)`.
    pub renders: Vec<(String, (u32, u32))>,
    /// Hook sites: `(callee, range)`.
    pub hooks: Vec<(String, (u32, u32))>,
}

/// Build the per-file symbols from parsed facts: one [`SymbolFactsRaw`] per
/// declaration, plus a synthesized per-file `Component` symbol for the
/// single-file-component langs.
///
/// `defining_file_raw` is left `0` here — the driver fills the real [`FileId`]
/// from the seeded file when it materialises `SymbolRecord`s, mirroring the
/// CLI committer that knew the id at absorb time
/// [src: crates/ariadne-cli/src/domain/mod.rs:531-578].
pub(crate) fn build_symbols(
    rel_path: &str,
    file_len: u32,
    facts: &SyntacticFactsRaw,
) -> Vec<SymbolFactsRaw> {
    let mut out = Vec::with_capacity(facts.decls.len() + 1);
    let lang = Path::new(rel_path)
        .extension()
        .and_then(|e| e.to_str())
        .and_then(Lang::from_extension);
    // An SFC (`.vue`/`.svelte`/`.astro`) carries exactly one component — the
    // file itself — but emits no enclosing `Component` decl: its template
    // render sites sit in the host layer, its decls in the injected
    // `<script>` layer. Synthesize a file-spanning `Component` symbol named
    // for the file stem so those renders have a graph source, and so a
    // cross-file `<Child/>` resolves to `Child`'s SFC
    // [src: crates/ariadne-cli/src/domain/mod.rs:520-552].
    if lang.is_some_and(is_sfc_lang) {
        out.push(SymbolFactsRaw {
            canonical_name: sfc_component_name(rel_path),
            kind: "component".to_owned(),
            defining_file_raw: 0,
            defining_byte_range: (0, file_len),
            visibility_byte: Visibility::Public.to_byte(),
            attributes: Vec::new(),
        });
    }
    for decl in &facts.decls {
        out.push(SymbolFactsRaw {
            canonical_name: decl.name.clone(),
            kind: decl.kind.clone(),
            defining_file_raw: 0,
            defining_byte_range: decl.def_byte_range,
            visibility_byte: decl.visibility_byte,
            attributes: decl.attributes.clone(),
        });
    }
    out
}

/// Edit-stable 64-bit symbol id: blake3 of `path#kind#name#nth`, forced
/// non-zero. `nth` is the 0-based occurrence index among same-`(name, kind)`
/// declarations in the file in source order, so the id is independent of byte
/// offsets — an edit elsewhere in the file leaves an unchanged symbol's id (and
/// the edges to it) intact. The synthesized SFC component passes
/// `kind = "component"`, `nth = 0` [src: post-v1-roadmap plan.md RD12; ADR-0017].
///
/// Residual churn is bounded to inserting a same-`(name, kind)` sibling before
/// an existing one in the same file (it shifts the later `nth`); the
/// divergence-0 proptest corrects any such state and ADR-0017 records the
/// accepted limitation (plan R-B5).
pub(crate) fn symbol_id(path: &str, kind: &str, name: &str, nth: u32) -> SymbolId {
    let key = format!("{path}#{kind}#{name}#{nth}");
    let digest = blake3::hash(key.as_bytes());
    let raw = u64::from_le_bytes(digest.as_bytes()[..8].try_into().expect("8 bytes"));
    SymbolId::new(raw).unwrap_or_else(|| SymbolId::new(1).expect("1 is non-zero"))
}

/// True for the framework single-file-component langs. An SFC's template
/// render sites have no enclosing function declaration, so the per-file
/// derivation synthesizes a `Component` symbol for them
/// [src: crates/ariadne-cli/src/domain/mod.rs:606-608].
pub(crate) fn is_sfc_lang(lang: Lang) -> bool {
    matches!(lang, Lang::Vue | Lang::Svelte | Lang::Astro)
}

/// Component name for a synthesized SFC symbol: the file stem (`Card` for
/// `ui/Card.vue`). Falls back to the whole relative path if it has no stem.
fn sfc_component_name(rel_path: &str) -> String {
    Path::new(rel_path)
        .file_stem()
        .map_or_else(|| rel_path.to_owned(), |s| s.to_string_lossy().into_owned())
}

/// Reduce each name's candidate list to [`SymbolId`]s, sorted by
/// `(defining FileId, def byte start)` so edge-`dst` selection is independent
/// of file-iteration order [src: crates/ariadne-cli/src/domain/mod.rs:689-699].
pub(crate) fn sort_candidates(
    name_to_symbols: HashMap<String, Vec<SymbolCandidate>>,
) -> HashMap<String, Vec<SymbolId>> {
    name_to_symbols
        .into_iter()
        .map(|(name, mut cands)| {
            cands.sort_by_key(|c| (c.file, c.def_start));
            (name, cands.into_iter().map(|c| c.id).collect())
        })
        .collect()
}

/// Resolve every call / render / hook site to a typed `src -> dst` edge.
///
/// A call site becomes a [`EdgeKind::References`] edge, a render site a
/// [`EdgeKind::Renders`] edge, a hook site a [`EdgeKind::UsesHook`] edge. For
/// each, `src` is the innermost declaration whose span contains the site and
/// `dst` is the named symbol — same-file match preferred, else the first
/// global match. An unresolved `src` or `dst`, or a self-loop, drops the edge:
/// the same best-effort policy for all three kinds
/// [src: crates/ariadne-cli/src/domain/mod.rs:718-768].
pub(crate) fn resolve_edges(
    facts_by_file: &[FileFacts],
    name_to_symbols: &HashMap<String, Vec<SymbolId>>,
) -> Vec<(EdgeKey, EdgeRecord)> {
    let mut seen: HashSet<EdgeKey> = HashSet::new();
    let mut out = Vec::new();
    for facts in facts_by_file {
        let local_ids: HashSet<SymbolId> = facts.symbols.iter().map(|l| l.id).collect();
        let mut resolve = |kind: EdgeKind, name: &str, range: (u32, u32)| {
            let Some(src) = enclosing_symbol(&facts.symbols, range) else {
                return;
            };
            let Some(candidates) = name_to_symbols.get(name) else {
                return;
            };
            let Some(dst) = candidates
                .iter()
                .find(|c| local_ids.contains(c))
                .or_else(|| candidates.first())
                .copied()
            else {
                return;
            };
            if dst == src {
                return;
            }
            let key = EdgeKey { src, kind, dst };
            if !seen.insert(key) {
                return;
            }
            out.push((
                key,
                EdgeRecord {
                    source_span: span(facts.file_id, range),
                    evidence_lang: facts.lang,
                    weight: 1,
                },
            ));
        };
        for (callee, range) in &facts.calls {
            resolve(EdgeKind::References, callee, *range);
        }
        for (component, range) in &facts.renders {
            resolve(EdgeKind::Renders, component, *range);
        }
        for (callee, range) in &facts.hooks {
            resolve(EdgeKind::UsesHook, callee, *range);
        }
    }
    out
}

/// Innermost declaration whose definition span contains `range`.
pub(crate) fn enclosing_symbol(locals: &[LocalSymbol], range: (u32, u32)) -> Option<SymbolId> {
    locals
        .iter()
        .filter(|l| l.def_range.0 <= range.0 && range.1 <= l.def_range.1)
        .min_by_key(|l| l.def_range.1 - l.def_range.0)
        .map(|l| l.id)
}

/// Build a [`Span`] from a file id and a byte range.
pub(crate) fn span(file: FileId, range: (u32, u32)) -> Span {
    Span {
        file,
        byte_start: range.0,
        byte_end: range.1,
    }
}
