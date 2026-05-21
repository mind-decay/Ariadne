//! `IngestPlan` orchestrator (plan step 12).
//!
//! Walks the registered drivers, asks each `detect(root)`, then runs the
//! survivors in parallel inside a rayon thread pool capped at
//! `available_parallelism / 2`. Each driver writes to its own
//! `tempfile::TempDir`, so two drivers cannot collide on `out.scip`. The
//! results funnel into an [`IngestReport`] that distinguishes:
//!
//! * `successes` — driver ran and produced a decodable SCIP doc.
//! * `warnings` — driver was skipped because its binary is missing on
//!   PATH (degraded mode, never a hard failure).
//! * `failures` — driver ran but errored mid-pipeline (`SubprocessFailed`,
//!   `Io`, `Decode`, …).
//!
//! Failures and warnings are also emitted to `tracing` so a CLI/MCP
//! consumer's logs surface the install hint even when the caller throws
//! the report away.

use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::thread;

use ariadne_core::Lang;
use rayon::prelude::*;
use tracing::warn;

use crate::errors::ScipError;
use crate::indexer::{
    IndexerWarning, IngestReport, LsifGoIndexer, RustAnalyzerIndexer, ScipClangIndexer, ScipDoc,
    ScipDotnetIndexer, ScipIndexer, ScipJavaIndexer, ScipPythonIndexer, ScipSvelteIndexer,
    ScipTypescriptIndexer, ScipVueIndexer,
};

/// Outcome of one driver's run, before aggregation into [`IngestReport`].
enum DriverOutcome {
    Success(ScipDoc),
    Missing(IndexerWarning),
    Failure(Lang, ScipError),
}

/// Parallel SCIP ingestion orchestrator.
pub struct IngestPlan {
    drivers: Vec<Box<dyn ScipIndexer>>,
    parallelism: NonZeroUsize,
    temp_root: Option<PathBuf>,
}

impl std::fmt::Debug for IngestPlan {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IngestPlan")
            .field("driver_count", &self.drivers.len())
            .field("parallelism", &self.parallelism)
            .field("temp_root", &self.temp_root)
            .finish()
    }
}

impl IngestPlan {
    /// Empty plan; register drivers via [`Self::with_drivers`].
    #[must_use]
    pub fn new() -> Self {
        Self {
            drivers: Vec::new(),
            parallelism: Self::default_parallelism(),
            temp_root: None,
        }
    }

    /// Plan populated with one instance of every driver shipped by this
    /// crate. Order is intentionally Rust-first so the most reliable
    /// indexer (`rust-analyzer`) lands its results first in the report.
    #[must_use]
    pub fn with_default_drivers() -> Self {
        Self::new().with_drivers(vec![
            Box::new(RustAnalyzerIndexer::new()),
            Box::new(ScipTypescriptIndexer::new()),
            Box::new(ScipVueIndexer::new()),
            Box::new(ScipSvelteIndexer::new()),
            Box::new(ScipPythonIndexer::new()),
            Box::new(ScipJavaIndexer::new()),
            Box::new(ScipClangIndexer::new()),
            Box::new(ScipDotnetIndexer::new()),
            Box::new(LsifGoIndexer::new()),
        ])
    }

    /// Replace the driver set.
    #[must_use]
    pub fn with_drivers(mut self, drivers: Vec<Box<dyn ScipIndexer>>) -> Self {
        self.drivers = drivers;
        self
    }

    /// Override the parallelism cap. Defaults to `available_parallelism / 2`
    /// (min 1).
    #[must_use]
    pub const fn with_parallelism(mut self, parallelism: NonZeroUsize) -> Self {
        self.parallelism = parallelism;
        self
    }

    /// Override the temp-dir root each driver writes through. Defaults to
    /// `std::env::temp_dir()`. Useful for tests that want predictable
    /// cleanup.
    #[must_use]
    pub fn with_temp_root(mut self, temp_root: impl Into<PathBuf>) -> Self {
        self.temp_root = Some(temp_root.into());
        self
    }

    /// Run every registered driver whose `detect(root)` returns true,
    /// aggregating outcomes into an [`IngestReport`].
    ///
    /// # Errors
    /// Never returns `Err` — by contract individual driver failures are
    /// recorded in `IngestReport.failures` so a single broken indexer
    /// cannot poison the whole run.
    #[must_use]
    pub fn ingest(&self, root: &Path) -> IngestReport {
        let candidates: Vec<&dyn ScipIndexer> = self
            .drivers
            .iter()
            .map(Box::as_ref)
            .filter(|d| d.detect(root))
            .collect();

        if candidates.is_empty() {
            return IngestReport::default();
        }

        let outcomes = self.run_candidates(&candidates, root);

        let mut report = IngestReport::default();
        for outcome in outcomes {
            match outcome {
                DriverOutcome::Success(doc) => report.record_success(doc),
                DriverOutcome::Missing(warning) => {
                    warn!(
                        target: "ariadne_scip::ingest",
                        lang = warning.lang.tag(),
                        binary = %warning.binary,
                        hint = %warning.install_hint,
                        "indexer binary missing; skipping language",
                    );
                    report.record_missing(warning);
                }
                DriverOutcome::Failure(lang, err) => {
                    warn!(
                        target: "ariadne_scip::ingest",
                        lang = lang.tag(),
                        error = %err,
                        "indexer failed",
                    );
                    report.record_failure(lang, err);
                }
            }
        }
        report
    }

    fn run_candidates(&self, candidates: &[&dyn ScipIndexer], root: &Path) -> Vec<DriverOutcome> {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(self.parallelism.get())
            .thread_name(|i| format!("ariadne-scip-ingest-{i}"))
            .build()
            .expect("rayon pool with a positive thread count must build");

        pool.install(|| {
            candidates
                .par_iter()
                .map(|driver| self.run_one(*driver, root))
                .collect()
        })
    }

    fn run_one(&self, driver: &dyn ScipIndexer, root: &Path) -> DriverOutcome {
        let lang = driver.lang();
        let tmp = match self.make_temp_dir(lang) {
            Ok(tmp) => tmp,
            Err(err) => return DriverOutcome::Failure(lang, err),
        };
        let out = tmp.path().join("out.scip");
        match driver.run(root, &out) {
            Ok(()) => match std::fs::read(&out) {
                Ok(bytes) => match driver.parse(&bytes) {
                    Ok(doc) => DriverOutcome::Success(doc),
                    Err(err) => DriverOutcome::Failure(lang, err),
                },
                Err(source) => DriverOutcome::Failure(
                    lang,
                    ScipError::Io {
                        path: out.clone(),
                        source,
                    },
                ),
            },
            Err(err) => match err {
                ScipError::IndexerMissing {
                    binary,
                    install_hint,
                } => DriverOutcome::Missing(IndexerWarning {
                    lang,
                    binary,
                    install_hint,
                }),
                other => DriverOutcome::Failure(lang, other),
            },
        }
    }

    fn make_temp_dir(&self, lang: Lang) -> Result<tempfile::TempDir, ScipError> {
        let prefix = format!("ariadne-scip-{}-", lang.tag());
        let mut builder = tempfile::Builder::new();
        builder.prefix(&prefix);
        let dir = match &self.temp_root {
            Some(root) => builder.tempdir_in(root),
            None => builder.tempdir(),
        };
        dir.map_err(|source| ScipError::Io {
            path: self.temp_root.clone().unwrap_or_else(std::env::temp_dir),
            source,
        })
    }

    fn default_parallelism() -> NonZeroUsize {
        let cores = thread::available_parallelism().map_or(1, NonZeroUsize::get);
        NonZeroUsize::new((cores / 2).max(1)).expect("max(1) keeps this non-zero")
    }
}

impl Default for IngestPlan {
    fn default() -> Self {
        Self::new()
    }
}
