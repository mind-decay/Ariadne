//! SCIP adapter façade — re-exports the subprocess+protobuf implementation
//! of `ariadne_core::Indexer`. No logic in this file.

#![deny(missing_docs)]

pub mod adapters;
pub mod domain;
pub mod errors;

pub use adapters::scip_subprocess::ScipSubprocessIndexer;
pub use errors::ScipError;
