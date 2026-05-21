//! tree-sitter port implementation. Submodules carry one concern each:
//! `registry` (per-lang grammar table), `incremental` (parser wrapper +
//! `parse_file`), `injection` (multi-region layer derivation), `facts`
//! (query-driven syntactic fact extraction), `cache` (round-trippable
//! parse-cache codec). Public surface of this adapter is re-exported from
//! `lib.rs` (src: docs/folder-layout.md rule 4; tier-03 plan `files`).

use ariadne_core::Lang;

pub mod cache;
pub mod facts;
pub mod incremental;
pub mod registry;

pub(crate) mod injection;

pub use incremental::{TreeSitterParser, parse_file};

/// Re-export of the underlying tree-sitter syntax tree as the parser
/// adapter's CST type. Keeping the alias inside this crate's namespace
/// shields callers from the raw `tree_sitter::Tree` path — downstream
/// crates depend on `ariadne_parser::Tree`, never the grammar crate
/// directly [src: docs/folder-layout.md rule 4].
pub type Tree = tree_sitter::Tree;

/// A parsed file as one or more grammar layers.
///
/// `host` is the whole-file skeleton: the HTML grammar for a `.vue` SFC, or
/// the language's own grammar for a single-grammar file. `injected` holds
/// zero or more sub-trees parsed from embedded regions — a Vue SFC's
/// `<script>` block becomes one JS/TS layer. A single-grammar file
/// (`.ts`/`.tsx`/`.js`/`.rs`/…) is the degenerate case: `host` only, with
/// `injected` empty (src: js-framework plan `<architecture>`; tier-03
/// step 4).
///
/// Every layer's node byte offsets are file-absolute. Injected layers are
/// parsed via `Parser::set_included_ranges` over the *full* file bytes, so
/// an injected sub-tree's offsets already share the file's coordinate
/// space — no manual remap
/// [src: <https://tree-sitter.github.io/tree-sitter/3-syntax-highlighting.html>].
#[derive(Debug)]
pub struct ParsedFile {
    /// Whole-file skeleton layer: its grammar [`Lang`] and syntax [`Tree`].
    pub host: (Lang, Tree),
    /// Embedded sub-trees, each with its own grammar [`Lang`] and [`Tree`],
    /// in document order. Empty for a single-grammar file.
    pub injected: Vec<(Lang, Tree)>,
}
