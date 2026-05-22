//! `scip-go` driver.
//!
//! Invocation: `scip-go index --output <out>` run with `cwd = <root>`
//! \[src: `scip-go index --help`, scip-go v0.2.6; post-v1-roadmap RD1].
//! `scip-go` is the native first-party Go SCIP indexer; it replaces the v1
//! two-step fallback that ran a separate dump-and-convert pipeline (v1
//! plan risk R3).
//!
//! `scip-go` shells out to the `go` toolchain to load and type-check the
//! module's packages. When `go` is absent the subprocess exits non-zero
//! and [`run_indexer`] surfaces a [`ScipError::SubprocessFailed`] carrying
//! scip-go's stderr, so the orchestrator records a failure and degrades to
//! syntactic-only rather than crashing. The optional `--module-path` /
//! `--module-version` overrides let a caller pin module metadata when it
//! is known ahead of the run [src: `scip-go index --help`, scip-go v0.2.6].

use std::path::{Path, PathBuf};
use std::process::Command;

use ariadne_core::Lang;
use prost::Message as _;

use crate::errors::ScipError;
use crate::indexer::subprocess::{ensure_parent, run_indexer};
use crate::indexer::{ScipDoc, ScipIndexer};
use crate::proto;

/// `scip-go` driver. Detect-fires on a `go.mod` at the project root.
#[derive(Debug, Default)]
pub struct ScipGoIndexer {
    binary: Option<PathBuf>,
    module_path: Option<String>,
    module_version: Option<String>,
}

impl ScipGoIndexer {
    /// Default driver (`scip-go` on PATH).
    #[must_use]
    pub fn new() -> Self {
        Self {
            binary: None,
            module_path: None,
            module_version: None,
        }
    }

    /// Driver pointed at an explicit binary path. Test injection point.
    #[must_use]
    pub fn with_binary(binary: impl Into<PathBuf>) -> Self {
        Self {
            binary: Some(binary.into()),
            ..Self::new()
        }
    }

    /// Override the module path scip-go would otherwise infer from
    /// `go.mod` (`--module-path`). Set it when the path is known ahead of
    /// the run [src: `scip-go index --help`, scip-go v0.2.6].
    #[must_use]
    pub fn with_module_path(mut self, module_path: impl Into<String>) -> Self {
        self.module_path = Some(module_path.into());
        self
    }

    /// Override the module version scip-go would otherwise infer from the
    /// VCS (`--module-version`). Set it when the version is known ahead of
    /// the run [src: `scip-go index --help`, scip-go v0.2.6].
    #[must_use]
    pub fn with_module_version(mut self, module_version: impl Into<String>) -> Self {
        self.module_version = Some(module_version.into());
        self
    }

    fn binary_path(&self) -> &Path {
        self.binary
            .as_deref()
            .unwrap_or_else(|| Path::new("scip-go"))
    }

    fn install_hint() -> &'static str {
        "go install github.com/scip-code/scip-go/cmd/scip-go@latest"
    }
}

impl ScipIndexer for ScipGoIndexer {
    fn lang(&self) -> Lang {
        Lang::Go
    }

    fn detect(&self, root: &Path) -> bool {
        root.join("go.mod").is_file()
    }

    fn run(&self, root: &Path, out: &Path) -> Result<(), ScipError> {
        let binary = self.binary_path();
        ensure_parent(out)?;
        let mut cmd = Command::new(binary);
        cmd.arg("index").arg("--output").arg(out).current_dir(root);
        if let Some(module_path) = &self.module_path {
            cmd.arg("--module-path").arg(module_path);
        }
        if let Some(module_version) = &self.module_version {
            cmd.arg("--module-version").arg(module_version);
        }
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
