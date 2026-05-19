//! Ariadne domain interior.
//!
//! Façade only — re-exports the domain module (types + records + ports +
//! changeset) and the crate error types. No logic lives in this file
//! [src: docs/folder-layout.md rule 3].

#![deny(missing_docs)]

pub mod domain;
pub mod errors;

pub use domain::changeset::{Changeset, RevisionId};
pub use domain::ports::{Indexer, Parser, ReadSnapshot, Storage, WatcherSink, WriteTxn};
pub use domain::records::{EdgeKey, EdgeKind, EdgeRecord, FileRecord, SymbolRecord};
pub use domain::types::{EdgeId, FileId, IdEncode, Lang, Span, SymbolId};
pub use domain::watcher::{ContentHash, Invalidation, ReconciliationReport};
pub use errors::{CoreError, StorageError};
