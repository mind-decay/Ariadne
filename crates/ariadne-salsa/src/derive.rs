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
    /// Package (crate) the defining file belongs to — the scoping key for
    /// [`resolve_edges`] [src: ADR-0024].
    pub package: String,
}

/// A candidate reduced for [`resolve_edges`]: its id plus the package scoping
/// key, in `(file, def_start)` order. Drops the byte offset once sorting is
/// done [src: ADR-0024].
pub(crate) struct ResolvedCandidate {
    /// Resolved symbol id.
    pub id: SymbolId,
    /// Package (crate) the defining file belongs to.
    pub package: String,
}

/// Package (crate) a path belongs to for edge-resolution scoping: the first
/// segment after a `crates/` prefix (`crates/<name>/…` → `<name>`), else the
/// empty string — all non-`crates/` files share one package, so a single-crate
/// project resolves cross-file as before. Mirrors
/// `ariadne_graph::doc_model::crate_of` so resolution scope matches docgen's
/// crate attribution; replicated here because `ariadne-salsa` may not depend on
/// `ariadne-graph` [src: tests/architecture.rs lines 30-35; ADR-0024].
pub(crate) fn package_of(path: &str) -> &str {
    path.strip_prefix("crates/")
        .and_then(|rest| rest.split('/').next())
        .filter(|name| !name.is_empty())
        .unwrap_or("")
}

/// Syntactic shape of a call site, decoded from `CallRaw.kind_byte` at the
/// changeset boundary. Derive-local because `ariadne-salsa` may not depend on
/// `ariadne-parser`; mirrors `ariadne_parser::CallKind` so the resolver can
/// gate the cross-crate fallback to free calls [src: ADR-0024; tests/architecture.rs].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CallKind {
    /// Bare-identifier call — eligible for the cross-crate fallback.
    Free,
    /// Receiver/member call — cross-crate fallback refused.
    Method,
    /// Scoped/qualified call — cross-crate fallback refused.
    Path,
}

impl CallKind {
    /// Decode the composition-root byte mirror (`0=Free`, `1=Method`,
    /// `2=Path`). An unknown byte falls back to `Free`, the recall-preserving
    /// default — the only producers are the two `call_kind_byte` roots, which
    /// never emit another value [src: crates/ariadne-cli/src/domain/mod.rs].
    pub(crate) fn from_byte(byte: u8) -> Self {
        match byte {
            1 => Self::Method,
            2 => Self::Path,
            _ => Self::Free,
        }
    }
}

/// Per-file facts retained between the symbol pass and the edge pass. Each
/// site is an unresolved `(name, …, range)` — a callee, a rendered child
/// component, or a hook — the edge pass resolves against the global symbol
/// table [src: crates/ariadne-cli/src/domain/mod.rs:177-184].
pub(crate) struct FileFacts {
    /// File the sites live in.
    pub file_id: FileId,
    /// Package (crate) this file belongs to — the caller's scoping key for
    /// [`resolve_edges`] [src: ADR-0024].
    pub package: String,
    /// Evidence language for resolved edges.
    pub lang: Lang,
    /// Local symbols (for the enclosing-symbol `src` lookup).
    pub symbols: Vec<LocalSymbol>,
    /// Call sites: `(callee, shape, range)`. The shape gates the cross-crate
    /// fallback in [`resolve_edges`].
    pub calls: Vec<(String, CallKind, (u32, u32))>,
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
            // The synthesized SFC component owns no decisions of its own;
            // its script's decls carry their per-decl complexity (tier-12 D4).
            complexity: 0,
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
            complexity: decl.complexity,
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

/// Reduce each name's candidate list to `(id, package)` pairs, sorted by
/// `(defining FileId, def byte start)` so edge-`dst` selection is independent
/// of file-iteration order [src: crates/ariadne-cli/src/domain/mod.rs:689-699].
/// The package key is retained for the scoped resolution in [`resolve_edges`]
/// [src: ADR-0024].
pub(crate) fn sort_candidates(
    name_to_symbols: HashMap<String, Vec<SymbolCandidate>>,
) -> HashMap<String, Vec<ResolvedCandidate>> {
    name_to_symbols
        .into_iter()
        .map(|(name, mut cands)| {
            cands.sort_by_key(|c| (c.file, c.def_start));
            let resolved = cands
                .into_iter()
                .map(|c| ResolvedCandidate {
                    id: c.id,
                    package: c.package,
                })
                .collect();
            (name, resolved)
        })
        .collect()
}

/// Build a [`FileFacts`] from a file's parsed facts: decode each call site's
/// shape byte and carry the render/hook sites verbatim. Factored out of
/// [`crate::AriadneDb::commit_revision`]'s per-file loop so the changeset
/// assembly stays within one screen.
pub(crate) fn file_facts(
    file_id: FileId,
    package: String,
    lang: Lang,
    symbols: Vec<LocalSymbol>,
    raw: &SyntacticFactsRaw,
) -> FileFacts {
    FileFacts {
        file_id,
        package,
        lang,
        symbols,
        calls: raw
            .calls
            .iter()
            .map(|c| {
                (
                    c.callee.clone(),
                    CallKind::from_byte(c.kind_byte),
                    c.byte_range,
                )
            })
            .collect(),
        renders: raw
            .renders
            .iter()
            .map(|r| (r.component.clone(), r.byte_range))
            .collect(),
        hooks: raw
            .hooks
            .iter()
            .map(|h| (h.callee.clone(), h.byte_range))
            .collect(),
    }
}

/// Resolve every call / render / hook site to a typed `src -> dst` edge.
///
/// A call site becomes a [`EdgeKind::References`] edge, a render site a
/// [`EdgeKind::Renders`] edge, a hook site a [`EdgeKind::UsesHook`] edge. For
/// each, `src` is the innermost declaration whose span contains the site and
/// `dst` is the named symbol resolved by scope precedence. A `Free` call (and
/// every render/hook) resolves same-file → same-crate → unambiguous-global (the
/// name has exactly one workspace definition); a `Method`/`Path` callee resolves
/// SAME-FILE ONLY. A `Method`/`Path` site (`socket.connect()`, `Foo::new()`)
/// captures only the bare member/segment name, so a same-crate *or* cross-crate
/// bare-name match is a guess — the `ProgressBar::new()` → unrelated same-crate
/// `new` phantom — so both wider tiers are refused, yielding no edge unless a
/// same-file definition exists [src: ADR-0025; r1-resolver-completion D1, D6].
/// A `Free` callee with no in-scope definition that is also ambiguous globally —
/// the std `Vec::new()` shape — likewise binds to no symbol. Render and hook
/// sites keep the full ladder (their resolution is unchanged). An unresolved
/// `src` or `dst`, or a self-loop, drops the edge: the same best-effort policy
/// for all three kinds [src: ADR-0024; ADR-0025;
/// crates/ariadne-cli/src/domain/mod.rs:718-768].
pub(crate) fn resolve_edges(
    facts_by_file: &[FileFacts],
    name_to_symbols: &HashMap<String, Vec<ResolvedCandidate>>,
) -> Vec<(EdgeKey, EdgeRecord)> {
    let mut seen: HashSet<EdgeKey> = HashSet::new();
    let mut out = Vec::new();
    for facts in facts_by_file {
        let local_ids: HashSet<SymbolId> = facts.symbols.iter().map(|l| l.id).collect();
        let caller_package = facts.package.as_str();
        let mut resolve = |edge: EdgeKind, name: &str, range: (u32, u32), wide_scope: bool| {
            let Some(src) = enclosing_symbol(&facts.symbols, range) else {
                return;
            };
            let Some(candidates) = name_to_symbols.get(name) else {
                return;
            };
            // Scope precedence: a definition in the caller's own file always
            // binds. Only a `wide_scope` site (a free-identifier call, or any
            // render/hook) may then fall back to a definition in the caller's
            // crate, and finally — when the name is unambiguous workspace-wide —
            // its single global definition. A narrow site (a `Method`/`Path`
            // callee) resolves same-file only: its bare member/segment name
            // dropped the qualifier, so a same-crate or global bare-name match is
            // a guess (the `X::new()` phantom). No in-scope match ⇒ no edge (the
            // candidate lists are already sorted, so each `find`/`first` is
            // deterministic) [src: ADR-0024; ADR-0025; r1-resolver-completion D1, D6].
            let same_file = candidates.iter().find(|c| local_ids.contains(&c.id));
            let same_crate = || candidates.iter().find(|c| c.package == caller_package);
            let unambiguous = || (candidates.len() == 1).then(|| &candidates[0]);
            let in_scope = if wide_scope {
                same_file.or_else(same_crate)
            } else {
                same_file
            };
            let resolved = if wide_scope {
                in_scope.or_else(unambiguous)
            } else {
                in_scope
            };
            let Some(dst) = resolved.map(|c| c.id) else {
                return;
            };
            if dst == src {
                return;
            }
            let key = EdgeKey {
                src,
                kind: edge,
                dst,
            };
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
        for (callee, kind, range) in &facts.calls {
            // Only a free-identifier callee may resolve beyond its own file (the
            // same-crate then unambiguous-global tiers); a method/path shape
            // resolves same-file only [src: ADR-0025].
            resolve(
                EdgeKind::References,
                callee,
                *range,
                matches!(kind, CallKind::Free),
            );
        }
        for (component, range) in &facts.renders {
            resolve(EdgeKind::Renders, component, *range, true);
        }
        for (callee, range) in &facts.hooks {
            resolve(EdgeKind::UsesHook, callee, *range, true);
        }
    }
    out
}

/// SCIP `SymbolRole::Definition` bit [src: crates/ariadne-scip/proto/scip.proto:526].
const SCIP_ROLE_DEFINITION: u32 = 0x1;
/// SCIP `SymbolRole::Import` bit [src: crates/ariadne-scip/proto/scip.proto:528].
const SCIP_ROLE_IMPORT: u32 = 0x2;
/// SCIP `SymbolRole::WriteAccess` bit [src: crates/ariadne-scip/proto/scip.proto:530].
const SCIP_ROLE_WRITE_ACCESS: u32 = 0x4;
/// SCIP `SymbolRole::ReadAccess` bit [src: crates/ariadne-scip/proto/scip.proto:532].
const SCIP_ROLE_READ_ACCESS: u32 = 0x8;

/// One covered file's SCIP occurrences plus its tree-sitter symbols — the input
/// to [`resolve_scip_edges`]. Occurrence byte ranges and symbol def ranges share
/// one coordinate system (the file's source bytes), so [`enclosing_symbol`] maps
/// each occurrence to its innermost enclosing symbol [src: scip-driven-edges D3].
pub(crate) struct ScipFileFacts {
    /// File the occurrences live in.
    pub file_id: FileId,
    /// Evidence language for resolved edges.
    pub lang: Lang,
    /// Tree-sitter symbols defined in this file (for the enclosing-symbol lookup).
    pub symbols: Vec<LocalSymbol>,
    /// `(symbol_key, byte_range, roles)` occurrences, sorted by `byte_range` so
    /// the edge output is independent of occurrence iteration order.
    pub occurrences: Vec<(String, (u32, u32), u32)>,
}

/// Resolve SCIP occurrences to typed `src -> dst` edges over the covered files
/// (scip-driven-edges plan D3). Two passes:
///
/// 1. Build the global `scip_symbol -> SymbolId` map from `Definition`-role
///    (`0x1`) occurrences: each maps to the innermost tree-sitter symbol whose
///    span encloses it. SCIP symbol strings are globally unique, so the map keys
///    with zero name collision — this is what recovers the cross-crate calls
///    ADR-0025 abstained on.
/// 2. Resolve every non-`Definition` occurrence: `src` is the enclosing
///    tree-sitter symbol, `dst` the map entry for its symbol key. An
///    `Import`-role (`0x2`) occurrence becomes an [`EdgeKind::Imports`] edge, a
///    `WriteAccess` (`0x4`) one an [`EdgeKind::Writes`] edge, a `ReadAccess`
///    (`0x8`) one an [`EdgeKind::Reads`] edge (write wins when both are set),
///    and any other a [`EdgeKind::References`] edge. An unmapped `dst`
///    (std/external), a missing `src`, or a self-loop drops the edge — the same
///    best-effort policy `resolve_edges` applies [src: ADR-0024;
///    scip-driven-edges D3, T2].
///
/// `facts_by_file` is expected pre-sorted by `file_id` and each file's
/// occurrences by `byte_range`, so the `(file, range)` order — and thus the
/// edge set — is deterministic [src: scip-driven-edges `<constraints>`].
pub(crate) fn resolve_scip_edges(facts_by_file: &[ScipFileFacts]) -> Vec<(EdgeKey, EdgeRecord)> {
    // Pass 1: Definition occurrences build the global symbol-key map.
    let mut sym_of_key: HashMap<&str, SymbolId> = HashMap::new();
    for facts in facts_by_file {
        for (key, range, roles) in &facts.occurrences {
            if roles & SCIP_ROLE_DEFINITION == 0 {
                continue;
            }
            if let Some(id) = enclosing_symbol(&facts.symbols, *range) {
                sym_of_key.entry(key.as_str()).or_insert(id);
            }
        }
    }

    // Pass 2: non-Definition occurrences resolve to edges.
    let mut seen: HashSet<EdgeKey> = HashSet::new();
    let mut out = Vec::new();
    for facts in facts_by_file {
        for (key, range, roles) in &facts.occurrences {
            if roles & SCIP_ROLE_DEFINITION != 0 {
                continue;
            }
            let Some(src) = enclosing_symbol(&facts.symbols, *range) else {
                continue;
            };
            let Some(&dst) = sym_of_key.get(key.as_str()) else {
                continue;
            };
            if dst == src {
                continue;
            }
            // Import-role stays `Imports`; otherwise the access bits promote the
            // occurrence to a dedicated edge, write taking precedence over read
            // when both are set (`x += 1`), else a plain `References` edge. Each
            // kind is emitted only on its present bit — no fabrication
            // [src: scip.proto:526-532; scip-driven-edges T2 step 3].
            let kind = if roles & SCIP_ROLE_IMPORT != 0 {
                EdgeKind::Imports
            } else if roles & SCIP_ROLE_WRITE_ACCESS != 0 {
                EdgeKind::Writes
            } else if roles & SCIP_ROLE_READ_ACCESS != 0 {
                EdgeKind::Reads
            } else {
                EdgeKind::References
            };
            let edge_key = EdgeKey { src, kind, dst };
            if !seen.insert(edge_key) {
                continue;
            }
            out.push((
                edge_key,
                EdgeRecord {
                    source_span: span(facts.file_id, *range),
                    evidence_lang: facts.lang,
                    weight: 1,
                },
            ));
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
