//! Storage adapter façade. Re-exports the redb-backed implementation of
//! `ariadne_core::Storage`. No logic in this file
//! [src: docs/folder-layout.md rule 3].

#![deny(missing_docs)]

pub mod adapters;
pub mod domain;
pub mod errors;

pub use adapters::redb::{RedbReadSnapshot, RedbStorage, RedbWriteTxn};
pub use errors::RedbStorageError;
