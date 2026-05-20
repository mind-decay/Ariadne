//! `scip-python` driver.
//!
//! Invocation: `scip-python index --project-name <name> --output <out> --cwd <root>`
//! \[src: `.claude/plans/ariadne-core/tier-05-scip-ingest.md` step 7,
//! <https://github.com/sourcegraph/scip-python>]. The `--project-name`
//! flag is mandatory; we derive it from the root directory's file name
//! (falling back to `"project"` when the root has no terminal component,
//! e.g. `/`).

use std::path::{Path, PathBuf};
use std::process::Command;

use ariadne_core::Lang;
use prost::Message as _;

use crate::errors::ScipError;
use crate::indexer::subprocess::{ensure_parent, run_indexer};
use crate::indexer::{ScipDoc, ScipIndexer};
use crate::proto;

/// `scip-python` driver. Detect-fires on `pyproject.toml` or `setup.py`.
#[derive(Debug, Default)]
pub struct ScipPythonIndexer {
    binary: Option<PathBuf>,
}

impl ScipPythonIndexer {
    /// Default driver (`scip-python` on PATH).
    #[must_use]
    pub fn new() -> Self {
        Self { binary: None }
    }

    /// Driver pointed at an explicit binary path. Test injection point.
    #[must_use]
    pub fn with_binary(binary: impl Into<PathBuf>) -> Self {
        Self {
            binary: Some(binary.into()),
        }
    }

    fn binary_path(&self) -> &Path {
        self.binary
            .as_deref()
            .unwrap_or_else(|| Path::new("scip-python"))
    }

    fn install_hint() -> &'static str {
        "npm install -g @sourcegraph/scip-python"
    }

    fn project_name(root: &Path) -> String {
        root.file_name()
            .and_then(|s| s.to_str())
            .filter(|s| !s.is_empty())
            .unwrap_or("project")
            .to_owned()
    }
}

impl ScipIndexer for ScipPythonIndexer {
    fn lang(&self) -> Lang {
        Lang::Python
    }

    fn detect(&self, root: &Path) -> bool {
        root.join("pyproject.toml").is_file() || root.join("setup.py").is_file()
    }

    fn run(&self, root: &Path, out: &Path) -> Result<(), ScipError> {
        let binary = self.binary_path();
        ensure_parent(out)?;
        let project_name = Self::project_name(root);
        let mut cmd = Command::new(binary);
        cmd.arg("index")
            .arg("--project-name")
            .arg(&project_name)
            .arg("--output")
            .arg(out)
            .arg("--cwd")
            .arg(root);
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
