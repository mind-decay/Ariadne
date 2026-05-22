//! Per-language SCIP driver trait + ingest report types.
//!
//! Each driver wraps one external indexer binary (`rust-analyzer scip …`,
//! `scip-typescript`, …). Drivers are small subprocess+protobuf adapters;
//! they return `Result<…, ScipError>` rather than panicking so one
//! broken indexer never poisons a multi-language ingest run (plan §scope:
//! "missing indexers degrade to syntactic-only … never crash"
//! [src: `.claude/plans/ariadne-core/tier-05-scip-ingest.md`]).
//!
//! `IngestPlan` (plan step 12) lives in the `plan` module; the driver-agnostic
//! [`parse`] free function exists so consumers of `ScipDocInput.raw_proto`
//! (tier-04 salsa input → driving adapters in tier-07+) can decode raw
//! SCIP bytes without first instantiating a per-language driver
//! [src: `.claude/plans/ariadne-core/tier-05-scip-ingest.md` step 16].

mod plan;
mod rust_analyzer;
mod scip_clang;
mod scip_dotnet;
mod scip_go;
mod scip_java;
mod scip_python;
mod scip_svelte;
mod scip_typescript;
mod scip_vue;
mod subprocess;

use std::path::Path;

use ariadne_core::Lang;
use prost::Message as _;

use crate::errors::ScipError;
use crate::proto;

pub use plan::IngestPlan;
pub use rust_analyzer::RustAnalyzerIndexer;
pub use scip_clang::ScipClangIndexer;
pub use scip_dotnet::ScipDotnetIndexer;
pub use scip_go::ScipGoIndexer;
pub use scip_java::ScipJavaIndexer;
pub use scip_python::ScipPythonIndexer;
pub use scip_svelte::ScipSvelteIndexer;
pub use scip_typescript::ScipTypescriptIndexer;
pub use scip_vue::ScipVueIndexer;

/// Decode raw SCIP protobuf bytes (e.g. the `raw_proto` payload pulled
/// from `ariadne_salsa::ScipDocInput`) into a typed [`ScipDoc`]. Free
/// function because driving adapters know the file's [`Lang`] from
/// `ariadne_salsa::FileMetadataInput` long before they touch a driver
/// instance; round-tripping through `ScipIndexer::parse` would only force
/// them to keep a registry of drivers for what is essentially a stateless
/// decode [src: `.claude/plans/ariadne-core/tier-05-scip-ingest.md`
/// step 16].
///
/// # Errors
/// [`ScipError::Decode`] on malformed protobuf.
pub fn parse(lang: Lang, bytes: &[u8]) -> Result<ScipDoc, ScipError> {
    let index = proto::Index::decode(bytes)?;
    Ok(ScipDoc { lang, index })
}

/// Parsed SCIP document plus the language whose driver produced it.
#[derive(Debug, Clone)]
pub struct ScipDoc {
    /// Language tag of the driver that produced this doc.
    pub lang: Lang,
    /// Decoded SCIP `Index` proto.
    pub index: proto::Index,
}

/// Per-language SCIP driver. Implementations live one-per-binary under
/// [`crate::indexer`].
pub trait ScipIndexer: Send + Sync + std::fmt::Debug {
    /// Language this driver targets.
    fn lang(&self) -> Lang;

    /// `true` when `root` looks like a project this driver should index
    /// (e.g. a `Cargo.toml` for the Rust driver). Detection must not run
    /// any subprocess.
    fn detect(&self, root: &Path) -> bool;

    /// Run the external indexer and write a SCIP file to `out`. Driver is
    /// responsible for shaping CLI flags; orchestrator owns temp-dir
    /// management.
    ///
    /// # Errors
    /// [`ScipError::IndexerMissing`] when the binary is absent from PATH,
    /// [`ScipError::SubprocessFailed`] on non-zero exit, [`ScipError::Io`]
    /// for filesystem failures.
    fn run(&self, root: &Path, out: &Path) -> Result<(), ScipError>;

    /// Decode the SCIP bytes the driver produced.
    ///
    /// # Errors
    /// [`ScipError::Decode`] on a malformed protobuf payload.
    fn parse(&self, scip_bytes: &[u8]) -> Result<ScipDoc, ScipError>;
}

/// Warning surfaced to the caller when an indexer is unavailable on this
/// host. The orchestrator collects these in [`IngestReport::warnings`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexerWarning {
    /// Language whose driver could not run.
    pub lang: Lang,
    /// Binary the driver tried to invoke.
    pub binary: String,
    /// One-line install hint, e.g. `"brew install rust-analyzer"`.
    pub install_hint: String,
}

/// Result of indexing a repository. `successes` lists languages whose SCIP
/// docs were produced; `warnings` lists drivers skipped because their
/// binary is missing (degraded mode, not failures); `failures` lists
/// drivers that did execute but errored.
#[derive(Debug, Default)]
pub struct IngestReport {
    /// SCIP docs from drivers that succeeded.
    pub docs: Vec<ScipDoc>,
    /// Languages whose driver succeeded (mirrors `docs`, kept separately
    /// for callers that only need the lang set).
    pub successes: Vec<Lang>,
    /// Drivers skipped because the binary is missing from PATH.
    pub warnings: Vec<IndexerWarning>,
    /// Drivers that ran but errored. Each entry has its own `ScipError`.
    pub failures: Vec<(Lang, ScipError)>,
}

impl IngestReport {
    /// Record a successful driver run.
    pub fn record_success(&mut self, doc: ScipDoc) {
        self.successes.push(doc.lang);
        self.docs.push(doc);
    }

    /// Record a missing-binary warning.
    pub fn record_missing(&mut self, warning: IndexerWarning) {
        self.warnings.push(warning);
    }

    /// Record a driver failure.
    pub fn record_failure(&mut self, lang: Lang, err: ScipError) {
        self.failures.push((lang, err));
    }
}
