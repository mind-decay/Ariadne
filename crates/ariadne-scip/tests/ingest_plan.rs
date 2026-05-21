//! `IngestPlan` orchestration tests. Stub drivers stand in for the
//! external indexer binaries so the parallel run-loop is exercised
//! deterministically across the success / missing / failed paths.
//!
//! Plan ref: `.claude/plans/ariadne-core/tier-05-scip-ingest.md` step 12.

mod common;

use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use ariadne_core::Lang;
use ariadne_scip::{IngestPlan, ScipDoc, ScipError, ScipIndexer};

use crate::common::{SymBp, synth_bytes};

#[derive(Debug, Clone)]
enum StubRun {
    /// Write synthetic SCIP bytes to the requested `out` path.
    Success {
        tool: &'static str,
        scheme: &'static str,
    },
    /// Pretend the binary is missing on PATH.
    Missing,
    /// Pretend the subprocess exited non-zero.
    Failed { status: i32, stderr: &'static str },
}

#[derive(Debug)]
struct StubIndexer {
    lang: Lang,
    detect: bool,
    binary: String,
    install_hint: String,
    run_outcome: StubRun,
    run_count: Arc<AtomicUsize>,
}

impl StubIndexer {
    fn new(
        lang: Lang,
        detect: bool,
        binary: &str,
        run_outcome: StubRun,
        run_count: Arc<AtomicUsize>,
    ) -> Self {
        Self {
            lang,
            detect,
            binary: binary.to_owned(),
            install_hint: format!("install {binary}"),
            run_outcome,
            run_count,
        }
    }
}

impl ScipIndexer for StubIndexer {
    fn lang(&self) -> Lang {
        self.lang
    }

    fn detect(&self, _root: &Path) -> bool {
        self.detect
    }

    fn run(&self, _root: &Path, out: &Path) -> Result<(), ScipError> {
        self.run_count.fetch_add(1, Ordering::SeqCst);
        match &self.run_outcome {
            StubRun::Success { tool, scheme } => {
                let bytes = synth_bytes(
                    tool,
                    "src/lib.rs",
                    "Stub",
                    &[SymBp {
                        raw: Box::leak(
                            format!("{scheme} cargo demo 1.0 lib/main/").into_boxed_str(),
                        ),
                        occurrences: 2,
                        relationships: 1,
                    }],
                );
                std::fs::write(out, &bytes).map_err(|source| ScipError::Io {
                    path: out.to_path_buf(),
                    source,
                })
            }
            StubRun::Missing => Err(ScipError::IndexerMissing {
                binary: self.binary.clone(),
                install_hint: self.install_hint.clone(),
            }),
            StubRun::Failed { status, stderr } => Err(ScipError::SubprocessFailed {
                binary: self.binary.clone(),
                status: *status,
                stderr: (*stderr).to_owned(),
            }),
        }
    }

    fn parse(&self, scip_bytes: &[u8]) -> Result<ScipDoc, ScipError> {
        use prost::Message as _;
        Ok(ScipDoc {
            lang: self.lang,
            index: ariadne_scip::proto::Index::decode(scip_bytes)?,
        })
    }
}

fn temp_root() -> PathBuf {
    let root = std::env::temp_dir().join("ariadne-scip-plan-tests");
    std::fs::create_dir_all(&root).expect("temp root creation must succeed");
    root
}

#[test]
fn detect_filters_drivers_before_running() {
    let calls = Arc::new(AtomicUsize::new(0));
    let plan = IngestPlan::new()
        .with_drivers(vec![
            Box::new(StubIndexer::new(
                Lang::Rust,
                false,
                "stub-rust",
                StubRun::Success {
                    tool: "stub-rust",
                    scheme: "scip-rust",
                },
                Arc::clone(&calls),
            )),
            Box::new(StubIndexer::new(
                Lang::TypeScript,
                false,
                "stub-ts",
                StubRun::Success {
                    tool: "stub-ts",
                    scheme: "scip-typescript",
                },
                Arc::clone(&calls),
            )),
        ])
        .with_parallelism(NonZeroUsize::new(2).unwrap())
        .with_temp_root(temp_root());

    let report = plan.ingest(Path::new("/nowhere"));
    assert!(
        report.docs.is_empty(),
        "no driver should have produced a doc"
    );
    assert!(report.warnings.is_empty(), "no driver should have warned");
    assert!(report.failures.is_empty(), "no driver should have failed");
    assert_eq!(
        calls.load(Ordering::SeqCst),
        0,
        "run() must never fire when detect()=false"
    );
}

#[test]
fn aggregates_success_warning_and_failure_in_one_run() {
    let calls = Arc::new(AtomicUsize::new(0));
    let plan = IngestPlan::new()
        .with_drivers(vec![
            Box::new(StubIndexer::new(
                Lang::Rust,
                true,
                "stub-rust",
                StubRun::Success {
                    tool: "stub-rust",
                    scheme: "scip-rust",
                },
                Arc::clone(&calls),
            )),
            Box::new(StubIndexer::new(
                Lang::Python,
                true,
                "stub-py",
                StubRun::Missing,
                Arc::clone(&calls),
            )),
            Box::new(StubIndexer::new(
                Lang::Java,
                true,
                "stub-java",
                StubRun::Failed {
                    status: 2,
                    stderr: "build failed",
                },
                Arc::clone(&calls),
            )),
        ])
        .with_parallelism(NonZeroUsize::new(3).unwrap())
        .with_temp_root(temp_root());

    let report = plan.ingest(Path::new("/anywhere"));

    assert_eq!(report.successes, vec![Lang::Rust]);
    assert_eq!(report.docs.len(), 1);
    assert_eq!(report.warnings.len(), 1);
    let warning = &report.warnings[0];
    assert_eq!(warning.lang, Lang::Python);
    assert_eq!(warning.binary, "stub-py");
    assert_eq!(report.failures.len(), 1);
    let (failed_lang, err) = &report.failures[0];
    assert_eq!(*failed_lang, Lang::Java);
    assert!(matches!(err, ScipError::SubprocessFailed { status: 2, .. }));
    assert_eq!(
        calls.load(Ordering::SeqCst),
        3,
        "every detect-true driver must run exactly once"
    );
}

#[test]
fn empty_driver_set_yields_empty_report() {
    let plan = IngestPlan::new();
    let report = plan.ingest(Path::new("/anywhere"));
    assert!(report.docs.is_empty());
    assert!(report.warnings.is_empty());
    assert!(report.failures.is_empty());
    assert!(report.successes.is_empty());
}

#[test]
fn default_driver_set_registers_eight_drivers() {
    // Use a guaranteed-missing path so every driver's run() returns
    // IndexerMissing and we observe the full registration in `warnings`
    // without needing any real binaries on PATH.
    let plan = IngestPlan::with_default_drivers().with_temp_root(temp_root());
    // We don't trigger run() (detect() = false on /tmp/empty); rather
    // assert by Debug that 8 drivers are registered — the seven tier-05
    // indexers plus the tier-07 Vue bridge.
    let dbg = format!("{plan:?}");
    assert!(
        dbg.contains("driver_count: 8"),
        "default driver set must register all 8 indexers: {dbg}"
    );
}
