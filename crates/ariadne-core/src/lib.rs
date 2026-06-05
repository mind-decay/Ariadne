//! Ariadne domain interior.
//!
//! Façade only — re-exports the domain module (types + records + ports +
//! changeset) and the crate error types. No logic lives in this file
//! [src: docs/folder-layout.md rule 3].

#![deny(missing_docs)]

pub mod domain;
pub mod errors;

pub use domain::changeset::{Changeset, RevisionId};
pub use domain::daemon::{
    BlastRadiusReport, CoChangeEdge, CoChangeReport, ComplexityReport, ComplexityRow, ComponentRow,
    CouplingReport, CouplingRow, CycleBreakRow, CycleRow, DaemonQuery, DaemonRequest,
    DaemonResponse, DependencyRow, DiffBlastReport, DiffSeed, DocForReport, DocReport,
    EdgeKindFilter, FileSummaryReport, GodModuleRow, Grain, HotspotReport, HotspotRow,
    MisplacedRow, OutboundRow, PlanAssistReport, PlanFileRow, ProjectStatusReport, RefactorReport,
    ReferenceSite, SymbolSummary, WeakSpotsReport,
};
pub use domain::ports::{
    ChunkStream, Indexer, Parser, ReadSnapshot, Storage, WatcherSink, WriteTxn,
};
pub use domain::records::{
    CoChangePair, DiffSpec, EdgeKey, EdgeKind, EdgeRecord, FileChurn, FileRecord, LineHunk,
    SymbolChurn, SymbolRecord,
};
pub use domain::scip::{ScipFacts, ScipOccurrence};
pub use domain::types::{EdgeId, FileId, IdEncode, Lang, Span, SymbolId, Visibility};
pub use domain::watcher::{ContentHash, Invalidation, ReconciliationReport};
pub use errors::{CoreError, StorageError};
