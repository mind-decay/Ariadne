//! SCIP adapter façade — re-exports the protobuf types, the per-language
//! `Indexer` driver trait, the `ingest_repo` orchestrator types, and the
//! canonical-symbol normalizer. No logic in this file
//! [src: docs/folder-layout.md rule 3].
//!
//! Plan ref: `.claude/plans/ariadne-core/tier-05-scip-ingest.md`.

#![deny(missing_docs)]

pub mod domain;
pub mod errors;
pub mod indexer;
pub mod normalize;
pub mod proto;

pub use errors::ScipError;
pub use indexer::{
    IndexerWarning, IngestPlan, IngestReport, LsifGoIndexer, RustAnalyzerIndexer, ScipClangIndexer,
    ScipDoc, ScipDotnetIndexer, ScipIndexer, ScipJavaIndexer, ScipPythonIndexer, ScipSvelteIndexer,
    ScipTypescriptIndexer, ScipVueIndexer, parse,
};
pub use normalize::{
    CanonicalSymbol, Descriptor, DescriptorSuffix, SymbolId, normalize_scip_symbol,
};
