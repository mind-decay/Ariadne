//! Parser adapter error enum. Surfaces tree-sitter failure modes in
//! domain-friendly terms; the underlying crate's types do not leak into
//! `pub` signatures [src: docs/folder-layout.md rule 4].

use thiserror::Error;

use ariadne_core::Lang;

/// Errors raised by the tree-sitter parser adapter.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ParserError {
    /// Language not registered with the active [`crate::ParserRegistry`].
    #[error("language {0:?} is not registered in the parser registry")]
    UnsupportedLang(Lang),

    /// `tree_sitter::Parser::set_language` rejected the grammar — usually a
    /// version mismatch between the grammar crate and the tree-sitter
    /// runtime (src: <https://docs.rs/tree-sitter>, `set_language`).
    #[error("failed to assign tree-sitter grammar for {lang:?}: {source}")]
    LanguageAssign {
        /// Affected language.
        lang: Lang,
        /// Underlying assignment error.
        #[source]
        source: tree_sitter::LanguageError,
    },

    /// `tree_sitter::Parser::parse` returned `None`. This happens when the
    /// configured timeout fires or the cancellation flag is set
    /// (src: <https://docs.rs/tree-sitter>, `parse`).
    #[error("tree-sitter parse returned None for {lang:?} (timeout or cancel)")]
    ParseAborted {
        /// Affected language.
        lang: Lang,
    },

    /// A tree-sitter [`tree_sitter::Query`] failed to compile.
    #[error("invalid tree-sitter query for {lang:?}: {source}")]
    QueryCompile {
        /// Affected language.
        lang: Lang,
        /// Underlying compile error.
        #[source]
        source: tree_sitter::QueryError,
    },

    /// Parse-cache codec failure.
    #[error("parse-cache codec failure: {0}")]
    Codec(String),
}
