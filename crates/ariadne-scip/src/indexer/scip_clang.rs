//! `scip-clang` driver.
//!
//! Invocation: `scip-clang --compdb <compile_commands.json> --out <out>`
//! [src: `.claude/plans/ariadne-core/tier-05-scip-ingest.md` step 9,
//! <https://github.com/sourcegraph/scip-clang>]. Requires a
//! `compile_commands.json` produced by `CMake` (`-DCMAKE_EXPORT_COMPILE_COMMANDS=ON`),
//! Bear, Bazel `bazel-compdb`, or an equivalent tool.
//!
//! `Lang::Other("clang")` because `ariadne-core`'s `Lang` enum does not
//! carry a C/C++ variant — the only place a per-language tag matters for
//! C/C++ today is `IngestReport.successes`, which is a free-form list,
//! and the salsa layer keys files by the `tree-sitter-<grammar>` name
//! the parser tier emits (`tree-sitter-cpp`, `tree-sitter-c`).

use std::path::{Path, PathBuf};
use std::process::Command;

use ariadne_core::Lang;
use prost::Message as _;

use crate::errors::ScipError;
use crate::indexer::subprocess::{ensure_parent, run_indexer};
use crate::indexer::{ScipDoc, ScipIndexer};
use crate::proto;

/// `scip-clang` driver. Detect-fires on a `compile_commands.json` at root.
#[derive(Debug, Default)]
pub struct ScipClangIndexer {
    binary: Option<PathBuf>,
}

impl ScipClangIndexer {
    /// Default driver (`scip-clang` on PATH).
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
            .unwrap_or_else(|| Path::new("scip-clang"))
    }

    fn install_hint() -> &'static str {
        "download from https://github.com/sourcegraph/scip-clang/releases"
    }
}

impl ScipIndexer for ScipClangIndexer {
    fn lang(&self) -> Lang {
        Lang::Other("clang")
    }

    fn detect(&self, root: &Path) -> bool {
        root.join("compile_commands.json").is_file()
    }

    fn run(&self, root: &Path, out: &Path) -> Result<(), ScipError> {
        let binary = self.binary_path();
        ensure_parent(out)?;
        let compdb = root.join("compile_commands.json");
        let mut cmd = Command::new(binary);
        cmd.arg("--compdb")
            .arg(&compdb)
            .arg("--out")
            .arg(out)
            .current_dir(root);
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
