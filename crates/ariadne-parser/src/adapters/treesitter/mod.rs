//! tree-sitter port implementation. Submodules carry one concern each:
//! `registry` (per-lang grammar table), `incremental` (parser wrapper +
//! `parse_file`), `facts` (query-driven syntactic fact extraction),
//! `cache` (round-trippable parse-cache codec). Public surface of this
//! adapter is re-exported from `lib.rs` (src: docs/folder-layout.md
//! rule 4; tier-03 plan `files`).

pub mod cache;
pub mod facts;
pub mod incremental;
pub mod registry;

pub use incremental::TreeSitterParser;

/// Re-export of the underlying tree-sitter syntax tree as the parser
/// adapter's CST type. Keeping the alias inside this crate's namespace
/// shields callers from the raw `tree_sitter::Tree` path — downstream
/// crates depend on `ariadne_parser::Tree`, never the grammar crate
/// directly [src: docs/folder-layout.md rule 4].
pub type Tree = tree_sitter::Tree;
