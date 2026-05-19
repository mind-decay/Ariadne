//! Salsa incremental query DB use case. Tier-04 wires the actual Salsa
//! database, durabilities, and per-table memory probes.

#![deny(missing_docs)]

pub mod domain;
pub mod errors;

pub use errors::SalsaError;
