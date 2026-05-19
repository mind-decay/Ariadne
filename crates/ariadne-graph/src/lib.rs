//! Graph analytics use cases. Builds on `ariadne-core` ports + reads from
//! `ariadne-storage`. Tier-07 wires petgraph + Tarjan + dominators.

#![deny(missing_docs)]

pub mod domain;
pub mod errors;

pub use errors::GraphError;
