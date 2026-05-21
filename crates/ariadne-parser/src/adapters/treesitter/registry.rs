//! Per-language tree-sitter grammar table.
//!
//! The registry holds one `tree_sitter::Language` per [`Lang`] variant the
//! v1 lang set supports. `tree_sitter::Language` is cheap to clone — the
//! handle behind the curtain is an `Arc` to a static C grammar function
//! (src: <https://docs.rs/tree-sitter/0.26.8/tree_sitter/struct.Language.html>).

use std::collections::HashMap;

use ariadne_core::Lang;
use tree_sitter::Language;

/// Tier-03 v1 language coverage. The table is the single source of truth
/// for which grammars ship in the binary; new langs land via a tier
/// follow-up + ADR (src: tier-03 plan `files` lang list).
const V1_LANGS: &[Lang] = &[
    Lang::TypeScript,
    Lang::JavaScript,
    Lang::Python,
    Lang::Rust,
    Lang::Go,
    Lang::Java,
    Lang::Kotlin,
    Lang::CSharp,
    Lang::C,
    Lang::Cpp,
    Lang::Tsx,
];

/// Per-`Lang` `tree_sitter::Language` table. Cloning is `O(1)`; new
/// instances cost only an `Arc` bump [src: tree-sitter crate notes].
#[derive(Debug, Clone)]
pub struct ParserRegistry {
    langs: HashMap<Lang, Language>,
}

impl ParserRegistry {
    /// Construct a registry preloaded with the v1 grammar set.
    #[must_use]
    pub fn new() -> Self {
        let mut langs = HashMap::with_capacity(V1_LANGS.len());
        for lang in V1_LANGS {
            langs.insert(*lang, language_for(*lang));
        }
        Self { langs }
    }

    /// `true` when the registry has a grammar registered for `lang`.
    #[must_use]
    pub fn supports(&self, lang: Lang) -> bool {
        self.langs.contains_key(&lang)
    }

    /// Iterate over the registered languages (insertion order is undefined).
    pub fn languages(&self) -> impl Iterator<Item = Lang> + '_ {
        self.langs.keys().copied()
    }

    /// Internal accessor for the underlying `tree_sitter::Language`. Kept
    /// `pub(crate)` so the type never leaks past the adapter boundary
    /// [src: docs/folder-layout.md rule 4].
    pub(crate) fn language(&self, lang: Lang) -> Option<&Language> {
        self.langs.get(&lang)
    }
}

impl Default for ParserRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Map a [`Lang`] to its compiled tree-sitter grammar. Each grammar crate
/// publishes a `LANGUAGE: LanguageFn` constant whose `Into<Language>`
/// conversion is the documented entry point
/// (src: <https://docs.rs/tree-sitter-rust/latest>).
fn language_for(lang: Lang) -> Language {
    match lang {
        Lang::TypeScript => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        // `.tsx` requires the distinct TSX grammar — `LANGUAGE_TSX` is a
        // separate `LanguageFn` from `LANGUAGE_TYPESCRIPT` because the TSX
        // grammar resolves `<T>x` as JSX, not a type cast (src: plan.md D2;
        // <https://docs.rs/tree-sitter-typescript/0.23.2>).
        Lang::Tsx => tree_sitter_typescript::LANGUAGE_TSX.into(),
        Lang::JavaScript => tree_sitter_javascript::LANGUAGE.into(),
        Lang::Python => tree_sitter_python::LANGUAGE.into(),
        Lang::Rust => tree_sitter_rust::LANGUAGE.into(),
        Lang::Go => tree_sitter_go::LANGUAGE.into(),
        Lang::Java => tree_sitter_java::LANGUAGE.into(),
        Lang::Kotlin => tree_sitter_kotlin_ng::LANGUAGE.into(),
        Lang::CSharp => tree_sitter_c_sharp::LANGUAGE.into(),
        Lang::C => tree_sitter_c::LANGUAGE.into(),
        Lang::Cpp => tree_sitter_cpp::LANGUAGE.into(),
        _ => unreachable!("V1_LANGS covers all registered Lang variants"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_covers_v1_langs() {
        let registry = ParserRegistry::new();
        for lang in V1_LANGS {
            assert!(registry.supports(*lang), "missing grammar for {lang:?}");
        }
    }

    #[test]
    fn registry_skips_unregistered_langs() {
        let registry = ParserRegistry::new();
        assert!(!registry.supports(Lang::Other("rescript")));
    }
}
