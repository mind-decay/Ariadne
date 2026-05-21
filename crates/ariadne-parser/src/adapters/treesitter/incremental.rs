//! `TreeSitterParser` wrapper + `parse_file` (cold + incremental).
//!
//! tree-sitter incremental parsing rule: call `Tree::edit(&InputEdit)` for
//! each text edit applied to the buffer, then re-invoke `Parser::parse`
//! with the same `&old_tree`. The library reuses unchanged nodes —
//! sub-millisecond reparse for a single-token edit on the 10MB benchmark
//! file (src: <https://github.com/tree-sitter/tree-sitter/blob/master/lib/binding_rust/README.md>).
//!
//! Timeout: tier-03 step 4 specifies a 5s cap. tree-sitter 0.26 removed
//! `set_timeout_micros` in favor of `ParseOptions::progress_callback`
//! returning `ControlFlow::Break` once a wall-clock deadline elapses
//! (src: tree-sitter-0.26.8 `binding_rust/lib.rs` `pub fn
//! parse_with_options` + `ParseOptions::progress_callback`).

use std::ops::ControlFlow;
use std::time::{Duration, Instant};

use ariadne_core::{Lang, Parser as ParserPort};
use tree_sitter::{InputEdit, ParseOptions, ParseState, Parser as TsParser};

use super::registry::ParserRegistry;
use super::{ParsedFile, Tree, injection};
use crate::errors::ParserError;

/// Hard cap per parse invocation. Tier-03 step 4 = 5 seconds.
pub const PARSE_TIMEOUT: Duration = Duration::from_secs(5);

/// Single-language tree-sitter parser. Holds a `tree_sitter::Parser` plus
/// the [`Lang`] it was bound to. The inner parser is `!Send` and
/// `!Sync`; create one per worker thread.
pub struct TreeSitterParser {
    inner: TsParser,
    lang: Lang,
}

impl TreeSitterParser {
    /// Build a parser preconfigured for `lang`. Returns
    /// [`ParserError::UnsupportedLang`] when the registry has no grammar
    /// for the requested lang, or [`ParserError::LanguageAssign`] when the
    /// grammar's API version is incompatible with the linked tree-sitter
    /// runtime.
    ///
    /// # Errors
    /// See variants above.
    pub fn for_lang(lang: Lang, registry: &ParserRegistry) -> Result<Self, ParserError> {
        let language = registry
            .language(lang)
            .ok_or(ParserError::UnsupportedLang(lang))?;
        let mut inner = TsParser::new();
        inner
            .set_language(language)
            .map_err(|source| ParserError::LanguageAssign { lang, source })?;
        Ok(Self { inner, lang })
    }

    /// Language this parser is bound to.
    #[must_use]
    pub fn lang(&self) -> Lang {
        self.lang
    }

    /// Parse `content`. When `prev_tree` is `Some`, `edits` must describe
    /// the byte ranges that changed since the previous parse; the prior
    /// tree is mutated via [`Tree::edit`] before re-parsing so tree-sitter
    /// can reuse unchanged subtrees [src: README `binding_rust`].
    ///
    /// # Errors
    /// [`ParserError::ParseAborted`] when the configured timeout fires.
    pub fn parse_file(
        &mut self,
        content: &[u8],
        prev_tree: Option<&Tree>,
        edits: &[InputEdit],
    ) -> Result<Tree, ParserError> {
        let edited_prev = prev_tree.cloned().map(|mut tree| {
            for edit in edits {
                tree.edit(edit);
            }
            tree
        });
        // Throttle wall-clock checks: tree-sitter's progress callback fires
        // very frequently. Hot-path `Instant::now()` showed up as ~95% of
        // cold-parse cost on the 10 MB benchmark, so only sample the
        // deadline every Nth tick. Overrun fraction ≤ 1 / sample-rate.
        let deadline = Instant::now() + PARSE_TIMEOUT;
        let mut ticks: u32 = 0;
        let mut on_progress = |_: &ParseState| -> ControlFlow<()> {
            ticks = ticks.wrapping_add(1);
            if ticks % DEADLINE_SAMPLE_EVERY == 0 && Instant::now() >= deadline {
                ControlFlow::Break(())
            } else {
                ControlFlow::Continue(())
            }
        };
        let options = ParseOptions::new().progress_callback(&mut on_progress);
        let len = content.len();
        let mut read = |i: usize, _| -> &[u8] { if i < len { &content[i..] } else { &[] } };
        self.inner
            .parse_with_options(&mut read, edited_prev.as_ref(), Some(options))
            .ok_or(ParserError::ParseAborted { lang: self.lang })
    }
}

/// Sample the wall-clock deadline once every Nth progress tick. Shared with
/// the injection engine so injected-layer parses honor the same throttle.
pub(super) const DEADLINE_SAMPLE_EVERY: u32 = 256;

impl ParserPort for TreeSitterParser {}

impl std::fmt::Debug for TreeSitterParser {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TreeSitterParser")
            .field("lang", &self.lang)
            .finish_non_exhaustive()
    }
}

/// Parse `content` into a multi-layer [`ParsedFile`].
///
/// `host_lang` selects the whole-file skeleton grammar; `registry` supplies
/// every grammar — the host plus any injected layer. The host tree is parsed
/// incrementally when `prev` is `Some`: `edits` describes the byte ranges
/// changed since the previous parse, exactly as for
/// [`TreeSitterParser::parse_file`].
///
/// Injected layers (a Vue SFC's `<script>` block) are re-derived from the
/// freshly parsed host tree and fully re-parsed on every call. Only the host
/// skeleton reparse is incremental — injected sub-trees are small, a fresh
/// parse keeps the engine simple, and the tier-03 proptest gate proves an
/// incremental `ParsedFile` equals a full reparse layer-for-layer
/// [src: tier-03 step 6].
///
/// A single-grammar file (`.ts`/`.tsx`/`.js`/`.rs`/…) yields a `ParsedFile`
/// whose `injected` vector is empty — the host-only degenerate case.
///
/// # Errors
/// Propagates every [`ParserError`] raised by the host parse or by the
/// injection engine (unsupported lang, grammar assignment, invalid injected
/// ranges, parse abort).
pub fn parse_file(
    host_lang: Lang,
    registry: &ParserRegistry,
    content: &[u8],
    prev: Option<&ParsedFile>,
    edits: &[InputEdit],
) -> Result<ParsedFile, ParserError> {
    let mut host_parser = TreeSitterParser::for_lang(host_lang, registry)?;
    let host_tree = host_parser.parse_file(content, prev.map(|p| &p.host.1), edits)?;
    let injected = injection::injected_layers(host_lang, &host_tree, content, registry)?;
    Ok(ParsedFile {
        host: (host_lang, host_tree),
        injected,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_minimal_javascript_program() {
        let registry = ParserRegistry::new();
        let mut parser = TreeSitterParser::for_lang(Lang::JavaScript, &registry).unwrap();
        let tree = parser.parse_file(b"const x = 1;", None, &[]).unwrap();
        assert_eq!(tree.root_node().kind(), "program");
        assert!(!tree.root_node().has_error());
    }

    #[test]
    fn unsupported_lang_returns_error() {
        let registry = ParserRegistry::new();
        let err = TreeSitterParser::for_lang(Lang::Other("rescript"), &registry).unwrap_err();
        assert!(matches!(err, ParserError::UnsupportedLang(_)));
    }
}
