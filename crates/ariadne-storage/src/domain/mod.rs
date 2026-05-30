//! Crate-local domain logic for the storage adapter.
//!
//! Holds the schema-migration framework (`migration`): pure version-chain
//! planning kept separate from the redb IO in `adapters/redb`.

pub(crate) mod migration;
