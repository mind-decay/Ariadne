//! `scip-typescript` driver.
//!
//! Invocation: `scip-typescript index --cwd <root> --output <out>`
//! \[src: `.claude/plans/ariadne-core/tier-05-scip-ingest.md` step 6,
//! <https://github.com/sourcegraph/scip-typescript>]. Reports `Lang::TypeScript`
//! for both `.ts` and `.js` projects — the indexer itself walks both file
//! types when a `tsconfig.json` permits, and the salsa layer distinguishes
//! per-file via `FileMetadataInput.lang_tag`.

use std::path::{Path, PathBuf};
use std::process::Command;

use ariadne_core::Lang;
use prost::Message as _;

use crate::errors::ScipError;
use crate::indexer::subprocess::{ensure_parent, run_indexer};
use crate::indexer::{ScipDoc, ScipIndexer};
use crate::proto;

/// `scip-typescript` driver. Detect-fires when both `package.json` and
/// `tsconfig.json` sit at the project root.
#[derive(Debug, Default)]
pub struct ScipTypescriptIndexer {
    binary: Option<PathBuf>,
}

impl ScipTypescriptIndexer {
    /// Default driver (`scip-typescript` on PATH).
    #[must_use]
    pub fn new() -> Self {
        Self { binary: None }
    }

    /// Driver pointed at an explicit binary path. Used by tests to inject
    /// a missing path so the `IndexerMissing` arm is exercised.
    #[must_use]
    pub fn with_binary(binary: impl Into<PathBuf>) -> Self {
        Self {
            binary: Some(binary.into()),
        }
    }

    fn binary_path(&self) -> &Path {
        self.binary
            .as_deref()
            .unwrap_or_else(|| Path::new("scip-typescript"))
    }

    fn install_hint() -> &'static str {
        "npm install -g @sourcegraph/scip-typescript"
    }
}

impl ScipIndexer for ScipTypescriptIndexer {
    fn lang(&self) -> Lang {
        Lang::TypeScript
    }

    fn detect(&self, root: &Path) -> bool {
        root.join("package.json").is_file() && root.join("tsconfig.json").is_file()
    }

    fn run(&self, root: &Path, out: &Path) -> Result<(), ScipError> {
        let binary = self.binary_path();
        ensure_parent(out)?;
        let mut cmd = Command::new(binary);
        cmd.arg("index")
            .arg("--cwd")
            .arg(root)
            .arg("--output")
            .arg(out);
        run_indexer(
            &binary.display().to_string(),
            Self::install_hint(),
            root,
            &mut cmd,
        )
    }

    fn parse(&self, scip_bytes: &[u8]) -> Result<ScipDoc, ScipError> {
        let index = proto::Index::decode(scip_bytes)?;
        Ok(ScipDoc {
            lang: self.lang(),
            index,
        })
    }
}
