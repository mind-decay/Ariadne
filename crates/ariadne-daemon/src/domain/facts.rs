//! Composition-root parse → facts conversion for the daemon (tier-08).
//!
//! The daemon parses changed files with `ariadne-parser` and feeds the result
//! into the shared `ariadne-salsa` derivation as a [`SyntacticFactsRaw`]. RD11
//! puts the parse at each composition root (CLI cold, daemon warm) and the
//! derivation in `ariadne-salsa`, which may not depend on `ariadne-parser`
//! [src: tests/architecture.rs lines 30-33]. This module therefore replicates
//! the CLI's `parse_facts`/`convert_facts`/`decl_kind_tag` exactly — the two
//! roots must produce identical facts or an incremental commit would churn
//! un-edited files; the tier-08 divergence-0 proptest guards the warm side and
//! the manual self-index run exercises the real fs path
//! [src: crates/ariadne-cli/src/domain/mod.rs:433-542; tier-08 build notes].

use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::path::Path;

use ariadne_core::Lang;
use ariadne_parser::{DeclKind, FactExtractor, ParserRegistry, SyntacticFacts};
use ariadne_salsa::{CallRaw, DeclRaw, HookRaw, ImportRaw, RenderRaw, SyntacticFactsRaw};

/// Map a path to its [`Lang`] by file extension, via [`Lang::from_extension`].
/// Returns `None` for paths the syntactic indexer skips — mirrors the CLI's
/// `lang_for_path` [src: crates/ariadne-cli/src/domain/mod.rs:52-54].
#[must_use]
pub(crate) fn lang_for_path(path: &Path) -> Option<Lang> {
    Lang::from_extension(path.extension()?.to_str()?)
}

/// Parse `content` as `lang` and convert the merged multi-layer facts into the
/// [`SyntacticFactsRaw`] the salsa input carries. A parse/extractor failure
/// yields the default (empty) facts so the file is still re-derived (and sheds
/// any stale symbols) rather than skipped — matching the CLI committer's
/// `facts: None` fall-through [src: crates/ariadne-cli/src/domain/mod.rs:224-229].
///
/// `extractors` is a per-engine cache keyed by every layer [`Lang`] met, so an
/// SFC's injected `<script>` reuses the compiled query of a plain `.ts` file.
pub(crate) fn parse_facts(
    lang: Lang,
    content: &[u8],
    registry: &ParserRegistry,
    extractors: &mut HashMap<Lang, FactExtractor>,
) -> SyntacticFactsRaw {
    match parse_layers(lang, content, registry, extractors) {
        Some(facts) => convert_facts(&facts),
        None => SyntacticFactsRaw::default(),
    }
}

/// Parse the host layer plus any injected layers and fold their facts through
/// the shared `absorb_layer` + `finalize` merge — the daemon equivalent of the
/// CLI's `parse_facts` [src: crates/ariadne-cli/src/domain/mod.rs:433-458].
fn parse_layers(
    lang: Lang,
    content: &[u8],
    registry: &ParserRegistry,
    extractors: &mut HashMap<Lang, FactExtractor>,
) -> Option<SyntacticFacts> {
    let parsed = ariadne_parser::parse_file(lang, registry, content, None, &[]).ok()?;
    let mut merged = SyntacticFacts::default();
    for (layer_lang, tree) in std::iter::once(&parsed.host).chain(parsed.injected.iter()) {
        let extractor = match extractors.entry(*layer_lang) {
            Entry::Occupied(e) => e.into_mut(),
            Entry::Vacant(e) => e.insert(FactExtractor::for_lang(*layer_lang, registry).ok()?),
        };
        merged.absorb_layer(extractor.extract(tree, content));
    }
    merged.finalize();
    Some(merged)
}

/// Convert one file's parser [`SyntacticFacts`] into the `Update`-safe
/// [`SyntacticFactsRaw`] the salsa input carries. Copied verbatim from the CLI
/// composition-root boundary so both roots emit identical facts
/// [src: crates/ariadne-cli/src/domain/mod.rs:474-521].
fn convert_facts(facts: &SyntacticFacts) -> SyntacticFactsRaw {
    SyntacticFactsRaw {
        decls: facts
            .decls
            .iter()
            .map(|d| DeclRaw {
                kind: decl_kind_tag(&d.kind),
                name: d.name.clone(),
                name_byte_range: d.name_byte_range,
                def_byte_range: d.def_byte_range,
                visibility_byte: d.visibility.to_byte(),
                attributes: d.attributes.clone(),
                complexity: d.complexity,
            })
            .collect(),
        imports: facts
            .imports
            .iter()
            .map(|i| ImportRaw {
                path: i.path.clone(),
                byte_range: i.byte_range,
            })
            .collect(),
        calls: facts
            .calls
            .iter()
            .map(|c| CallRaw {
                callee: c.callee.clone(),
                byte_range: c.byte_range,
            })
            .collect(),
        renders: facts
            .renders
            .iter()
            .map(|r| RenderRaw {
                component: r.component.clone(),
                byte_range: r.byte_range,
            })
            .collect(),
        hooks: facts
            .hooks
            .iter()
            .map(|h| HookRaw {
                callee: h.callee.clone(),
                byte_range: h.byte_range,
            })
            .collect(),
    }
}

/// Short stable tag for an `ariadne_parser` declaration kind. Copied verbatim
/// from the CLI so the two roots key symbols identically
/// [src: crates/ariadne-cli/src/domain/mod.rs:524-542].
fn decl_kind_tag(kind: &DeclKind) -> String {
    match kind {
        DeclKind::Function => "function",
        DeclKind::Method => "method",
        DeclKind::Class => "class",
        DeclKind::Struct => "struct",
        DeclKind::Enum => "enum",
        DeclKind::Interface => "interface",
        DeclKind::Trait => "trait",
        DeclKind::TypeAlias => "type",
        DeclKind::Record => "record",
        DeclKind::Object => "object",
        DeclKind::Module => "module",
        DeclKind::Variable => "variable",
        DeclKind::Component => "component",
        DeclKind::Other(s) => s.as_str(),
    }
    .to_owned()
}
