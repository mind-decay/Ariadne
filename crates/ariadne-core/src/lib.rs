//! Ariadne domain interior.
//!
//! Façade only — re-exports the domain module (types + ports) and the
//! crate error type. No logic lives in this file
//! [src: docs/folder-layout.md rule 3].

#![deny(missing_docs)]

pub mod domain;
pub mod errors;

pub use domain::ports::{Indexer, Parser, Storage, WatcherSink};
pub use domain::types::{EdgeId, FileId, IdEncode, Lang, Span, SymbolId};
pub use errors::CoreError;
