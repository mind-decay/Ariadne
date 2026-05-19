//! `scip-dotnet` driver.
//!
//! Invocation: `scip-dotnet index --output <out>` run with `current_dir =
//! <root>` [src: `.claude/plans/ariadne-core/tier-05-scip-ingest.md`
//! step 10, <https://github.com/sourcegraph/scip-dotnet>]. Detect-fires
//! on any `*.sln` or `*.csproj` directly under the project root; the
//! shallow walk is intentional — recursing the entire repo on every
//! `detect()` blows the `IngestPlan` scheduling budget on monorepos.

use std::path::{Path, PathBuf};
use std::process::Command;

use ariadne_core::Lang;
use prost::Message as _;

use crate::errors::ScipError;
use crate::indexer::subprocess::{ensure_parent, run_indexer};
use crate::indexer::{ScipDoc, ScipIndexer};
use crate::proto;

/// `scip-dotnet` driver. Detect-fires on a `.sln` or `.csproj` at root.
#[derive(Debug, Default)]
pub struct ScipDotnetIndexer {
    binary: Option<PathBuf>,
}

impl ScipDotnetIndexer {
    /// Default driver (`scip-dotnet` on PATH).
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
            .unwrap_or_else(|| Path::new("scip-dotnet"))
    }

    fn install_hint() -> &'static str {
        "dotnet tool install -g SourcegraphScipDotnet"
    }

    fn has_marker(root: &Path) -> bool {
        let Ok(read_dir) = std::fs::read_dir(root) else {
            return false;
        };
        for entry in read_dir.flatten() {
            let path = entry.path();
            let Some(ext) = path.extension() else {
                continue;
            };
            if ext.eq_ignore_ascii_case("sln") || ext.eq_ignore_ascii_case("csproj") {
                return true;
            }
        }
        false
    }
}

impl ScipIndexer for ScipDotnetIndexer {
    fn lang(&self) -> Lang {
        Lang::CSharp
    }

    fn detect(&self, root: &Path) -> bool {
        Self::has_marker(root)
    }

    fn run(&self, root: &Path, out: &Path) -> Result<(), ScipError> {
        let binary = self.binary_path();
        ensure_parent(out)?;
        let mut cmd = Command::new(binary);
        cmd.arg("index").arg("--output").arg(out).current_dir(root);
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
