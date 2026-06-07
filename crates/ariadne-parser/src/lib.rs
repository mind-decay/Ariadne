//! Parser adapter façade — re-exports the tree-sitter implementation of the
//! `ariadne_core::Parser` port plus its associated value types. No logic
//! in this file [src: docs/folder-layout.md rule 3].

#![deny(missing_docs)]

pub mod adapters;
pub mod domain;
pub mod errors;

pub use adapters::treesitter::cache::ParseCache;
pub use adapters::treesitter::facts::{
    CallKind, CallSite, Decl, DeclKind, FactExtractor, HookSite, Import, RenderSite,
    SyntacticFacts, extract_syntactic_facts,
};
pub use adapters::treesitter::registry::ParserRegistry;
pub use adapters::treesitter::surface::public_surface;
pub use adapters::treesitter::{ParsedFile, Tree, TreeSitterParser, parse_file};
pub use errors::ParserError;
