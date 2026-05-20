//! Go fallback driver: `lsif-go` then `scip convert --from=lsif`.
//!
//! There is no first-party `scip-go` indexer (plan risk R3
//! [src: `.claude/plans/ariadne-core/plan.md`]). Two-step pipeline:
//!
//! 1. `lsif-go --no-animation --output <tmp>/dump.lsif` in `cwd = <root>`
//!    \[src: `.claude/plans/ariadne-core/tier-05-scip-ingest.md` step 11,
//!    <https://github.com/sourcegraph/lsif-go>].
//! 2. `scip convert --from=lsif --in=<tmp>/dump.lsif --out=<out>`
//!    [src: <https://github.com/sourcegraph/scip> CLI docs].
//!
//! The driver holds both binary paths so tests can inject missing
//! binaries independently and the `IngestReport` warning distinguishes
//! the `lsif-go` failure from the `scip` failure.

use std::path::{Path, PathBuf};
use std::process::Command;

use ariadne_core::Lang;
use prost::Message as _;

use crate::errors::ScipError;
use crate::indexer::subprocess::{ensure_parent, run_indexer};
use crate::indexer::{ScipDoc, ScipIndexer};
use crate::proto;

/// Two-step Go driver: `lsif-go` → `scip convert`. Detect-fires on
/// `go.mod` at the project root.
#[derive(Debug, Default)]
pub struct LsifGoIndexer {
    lsif_go_binary: Option<PathBuf>,
    scip_binary: Option<PathBuf>,
}

impl LsifGoIndexer {
    /// Default driver (`lsif-go` + `scip` on PATH).
    #[must_use]
    pub fn new() -> Self {
        Self {
            lsif_go_binary: None,
            scip_binary: None,
        }
    }

    /// Override the `lsif-go` binary path. Test injection point.
    #[must_use]
    pub fn with_lsif_go_binary(mut self, binary: impl Into<PathBuf>) -> Self {
        self.lsif_go_binary = Some(binary.into());
        self
    }

    /// Override the `scip` CLI binary path. Test injection point.
    #[must_use]
    pub fn with_scip_binary(mut self, binary: impl Into<PathBuf>) -> Self {
        self.scip_binary = Some(binary.into());
        self
    }

    fn lsif_go_path(&self) -> &Path {
        self.lsif_go_binary
            .as_deref()
            .unwrap_or_else(|| Path::new("lsif-go"))
    }

    fn scip_path(&self) -> &Path {
        self.scip_binary
            .as_deref()
            .unwrap_or_else(|| Path::new("scip"))
    }

    fn lsif_go_install_hint() -> &'static str {
        "go install github.com/sourcegraph/lsif-go/cmd/lsif-go@latest"
    }

    fn scip_install_hint() -> &'static str {
        "brew install sourcegraph/scip/scip (or download from https://github.com/sourcegraph/scip/releases)"
    }
}

impl ScipIndexer for LsifGoIndexer {
    fn lang(&self) -> Lang {
        Lang::Go
    }

    fn detect(&self, root: &Path) -> bool {
        root.join("go.mod").is_file()
    }

    fn run(&self, root: &Path, out: &Path) -> Result<(), ScipError> {
        let out_dir = ensure_parent(out)?;
        let dump = out_dir.join("dump.lsif");

        // Step 1: lsif-go → dump.lsif (run inside the project root so
        // module resolution sees the local toolchain).
        let lsif_go = self.lsif_go_path();
        let mut step1 = Command::new(lsif_go);
        step1
            .arg("--no-animation")
            .arg("--output")
            .arg(&dump)
            .current_dir(root);
        run_indexer(
            &lsif_go.display().to_string(),
            Self::lsif_go_install_hint(),
            root,
            &mut step1,
        )?;

        // Step 2: scip convert → out.scip.
        let scip = self.scip_path();
        let mut step2 = Command::new(scip);
        step2
            .arg("convert")
            .arg("--from=lsif")
            .arg(format!("--in={}", dump.display()))
            .arg(format!("--out={}", out.display()));
        let convert_result = run_indexer(
            &scip.display().to_string(),
            Self::scip_install_hint(),
            &dump,
            &mut step2,
        );

        // Best-effort cleanup of the intermediate dump regardless of
        // step 2's outcome; failures here would only mask the real error.
        let _ = std::fs::remove_file(&dump);
        convert_result
    }

    fn parse(&self, scip_bytes: &[u8]) -> Result<ScipDoc, ScipError> {
        let index = proto::Index::decode(scip_bytes)?;
        Ok(ScipDoc {
            lang: self.lang(),
            index,
        })
    }
}
