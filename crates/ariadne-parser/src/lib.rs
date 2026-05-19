//! Parser adapter façade — re-exports the tree-sitter implementation of
//! `ariadne_core::Parser`. No logic in this file.

#![deny(missing_docs)]

pub mod adapters;
pub mod domain;
pub mod errors;

pub use adapters::treesitter::TreeSitterParser;
pub use errors::ParserError;
