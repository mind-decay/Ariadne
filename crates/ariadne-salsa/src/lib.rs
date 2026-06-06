//! Ariadne incremental query database. Tier-04 wires the salsa surface
//! (inputs, derived queries, per-table memory probe). Real parser/SCIP
//! orchestration is performed by the driver layer in later tiers — salsa
//! holds inputs + tracked-fn shells and never imports adapter crates
//! [src: tests/architecture.rs lines 30-33].

#![deny(missing_docs)]

pub mod db;
// The pure per-file derivation (tier-07a, RD11). Crate-private: its functions
// are an internal contract between `derived` (memoized per-file step) and `db`
// (the driver pass), not a public API surface.
mod derive;
// `salsa::input` and `salsa::tracked` macros generate public methods without
// `///` doc comments; `missing_docs` would block compilation. Limit the
// allow to the modules where salsa codegen lives; hand-authored items in
// the rest of the crate keep the deny.
#[allow(missing_docs)]
pub mod derived;
pub mod errors;
#[allow(missing_docs)]
pub mod inputs;
pub mod memory;

mod domain;

pub use db::{AriadneDb, EventLog, FileDerivation};
pub use derived::{
    CallRaw, DeclRaw, EdgeFactsRaw, HookRaw, ImportRaw, RenderRaw, ScipFactsRaw, ScipOccurrenceRaw,
    ScipRelationshipRaw, SymbolFactsRaw, SyntacticFactsRaw, blast_radius, edges_for_file,
    scip_facts_for_file, symbols_for_file, syntactic_facts,
};
pub use errors::SalsaError;
pub use inputs::{
    FileContentInput, FileMetadataInput, ProjectConfigInput, ScipFactsInput, SyntacticFactsInput,
    durability_for,
};
pub use memory::{MemoryReport, TABLE_BUDGET_BYTES};

// Re-export salsa's durability tag + the `Setter` trait so callers (e.g.
// `ariadne-watcher`) don't need to add salsa to their direct deps just to
// drive input mutations via the `.with_durability(...).to(...)` chain.
pub use salsa::{Durability, Setter};
