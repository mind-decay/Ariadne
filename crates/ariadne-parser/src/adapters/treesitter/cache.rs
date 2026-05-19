//! Parse-cache codec.
//!
//! tree-sitter's `Tree` is *not* part of its stable serialization surface:
//! the C library deliberately offers no public `serialize`/`deserialize`
//! pair, and the in-memory representation may change between minor
//! versions (src: <https://github.com/tree-sitter/tree-sitter> discussion
//! 2358 — "no stable on-disk tree format"). Tier-03 step 8 resolves the
//! trade-off by caching `(Lang tag + raw source bytes)` and re-parsing on
//! cold load. Re-parsing a single file via the linked grammar is fast
//! enough to amortize across the tier-04 Salsa query graph (criterion
//! gates calibrated per ADR-0005) (src: tier-03-parser.md step 8).
//!
//! The on-disk codec is bincode + serde so the cache can ride alongside
//! the redb tables tier-02 already serializes via postcard without
//! contending for a single global encoder
//! (src: tier-03 plan files list; <https://docs.rs/bincode/2.0.1>
//! serde-flavored `encode_to_vec` / `decode_from_slice`).

use ariadne_core::Lang;
use bincode::config::Configuration;

use crate::errors::ParserError;

use super::Tree;
use super::incremental::TreeSitterParser;
use super::registry::ParserRegistry;

/// Codec config: standard bincode (varint, little-endian, fixed-array).
/// (src: <https://docs.rs/bincode/2.0.1/bincode/config/fn.standard.html>)
fn codec() -> Configuration {
    bincode::config::standard()
}

/// Round-trippable parse-cache payload. Stores enough state to reconstruct
/// the syntax tree on cold load via [`ParseCache::rehydrate`].
#[derive(Debug, Clone)]
pub struct ParseCache {
    lang: Lang,
    content: Vec<u8>,
}

impl ParseCache {
    /// Snapshot the inputs the tree-sitter parse depends on.
    #[must_use]
    pub fn capture(lang: Lang, content: Vec<u8>) -> Self {
        Self { lang, content }
    }

    /// Affected language.
    #[must_use]
    pub fn lang(&self) -> Lang {
        self.lang
    }

    /// Cached source bytes.
    #[must_use]
    pub fn content(&self) -> &[u8] {
        &self.content
    }

    /// Encode the cache into a byte vector suitable for redb storage. The
    /// codec is bincode-with-serde — the same crate ariadne uses elsewhere
    /// in tier-03.
    ///
    /// # Errors
    /// Encoding can only fail if the underlying writer (a `Vec<u8>`) runs
    /// out of memory; this is surfaced as [`ParserError::Codec`].
    pub fn encode(&self) -> Result<Vec<u8>, ParserError> {
        let payload = (self.lang.tag(), &self.content);
        bincode::serde::encode_to_vec(&payload, codec())
            .map_err(|e| ParserError::Codec(e.to_string()))
    }

    /// Decode a previously-encoded payload. The inverse of [`Self::encode`];
    /// byte-stable for a given `(lang, content)` pair.
    ///
    /// # Errors
    /// [`ParserError::Codec`] when the byte buffer is malformed.
    pub fn decode(bytes: &[u8]) -> Result<Self, ParserError> {
        let ((tag, content), _read): ((String, Vec<u8>), usize) =
            bincode::serde::decode_from_slice(bytes, codec())
                .map_err(|e| ParserError::Codec(e.to_string()))?;
        let lang = Lang::from_tag(&tag)
            .ok_or_else(|| ParserError::Codec(format!("unknown lang: {tag}")))?;
        Ok(Self { lang, content })
    }

    /// Re-parse the cached source against the live registry. Used by the
    /// tier-04 Salsa loader when the in-RAM CST has been evicted.
    ///
    /// # Errors
    /// Propagates [`ParserError::UnsupportedLang`] /
    /// [`ParserError::LanguageAssign`] / [`ParserError::ParseAborted`] as
    /// emitted by [`TreeSitterParser`].
    pub fn rehydrate(&self, registry: &ParserRegistry) -> Result<Tree, ParserError> {
        let mut parser = TreeSitterParser::for_lang(self.lang, registry)?;
        parser.parse_file(&self.content, None, &[])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_decode_round_trip_is_byte_stable() {
        let cache = ParseCache::capture(Lang::JavaScript, b"const a = 1;".to_vec());
        let bytes_a = cache.encode().unwrap();
        let bytes_b = cache.encode().unwrap();
        assert_eq!(bytes_a, bytes_b, "encode must be deterministic");
        let decoded = ParseCache::decode(&bytes_a).unwrap();
        assert_eq!(decoded.lang(), Lang::JavaScript);
        assert_eq!(decoded.content(), cache.content());
    }

    #[test]
    fn rehydrate_returns_well_formed_tree() {
        let cache = ParseCache::capture(Lang::Rust, b"fn main() {}".to_vec());
        let registry = ParserRegistry::new();
        let tree = cache.rehydrate(&registry).unwrap();
        assert_eq!(tree.root_node().kind(), "source_file");
        assert!(!tree.root_node().has_error());
    }
}
