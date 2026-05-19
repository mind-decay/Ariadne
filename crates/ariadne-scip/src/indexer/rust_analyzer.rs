//! `rust-analyzer scip` driver.
//!
//! Invocation: `rust-analyzer scip <root>` writes `<cwd>/index.scip`
//! [src: `rust-analyzer scip --help` from rust-analyzer 0.0.0 (homebrew
//! 2026-05-18 build)]. Newer revisions accept `--output <path>`, but the
//! plan letter only mandates "invokes `rust-analyzer scip <root>`"; we
//! supply the deterministic path via the `<cwd>` invariant rather than a
//! flag to stay compatible across the version skew described in the tier
//! plan README.

use std::path::{Path, PathBuf};
use std::process::Command;

use ariadne_core::Lang;
use prost::Message as _;

use crate::errors::ScipError;
use crate::indexer::subprocess::{ensure_parent, run_indexer};
use crate::indexer::{ScipDoc, ScipIndexer};
use crate::proto;

/// `rust-analyzer scip` driver. Detects on the presence of a `Cargo.toml`.
#[derive(Debug, Default)]
pub struct RustAnalyzerIndexer {
    /// Override binary name. Defaults to `"rust-analyzer"`; tests inject
    /// an absolute path or a sentinel to force `IndexerMissing`.
    binary: Option<PathBuf>,
}

impl RustAnalyzerIndexer {
    /// Default driver (`rust-analyzer` on PATH).
    #[must_use]
    pub fn new() -> Self {
        Self { binary: None }
    }

    /// Driver that invokes the given binary path. Used by tests to point
    /// at a non-existent path so the missing-binary path is exercised.
    #[must_use]
    pub fn with_binary(binary: impl Into<PathBuf>) -> Self {
        Self {
            binary: Some(binary.into()),
        }
    }

    fn binary_path(&self) -> &Path {
        self.binary
            .as_deref()
            .unwrap_or_else(|| Path::new("rust-analyzer"))
    }

    fn install_hint() -> &'static str {
        "rustup component add rust-analyzer (or `brew install rust-analyzer`)"
    }
}

impl ScipIndexer for RustAnalyzerIndexer {
    fn lang(&self) -> Lang {
        Lang::Rust
    }

    fn detect(&self, root: &Path) -> bool {
        root.join("Cargo.toml").is_file()
    }

    fn run(&self, root: &Path, out: &Path) -> Result<(), ScipError> {
        let binary = self.binary_path();
        // rust-analyzer writes `index.scip` to the current working
        // directory; setting CWD to `out.parent()` gives us a known
        // destination, and we rename to `out` after the run.
        let out_dir = ensure_parent(out)?;
        let mut cmd = Command::new(binary);
        cmd.arg("scip").arg(root).current_dir(&out_dir);
        run_indexer(
            &binary.display().to_string(),
            Self::install_hint(),
            root,
            &mut cmd,
        )?;
        let produced = out_dir.join("index.scip");
        if produced != *out {
            std::fs::rename(&produced, out).map_err(|source| ScipError::Io {
                path: produced,
                source,
            })?;
        }
        Ok(())
    }

    fn parse(&self, scip_bytes: &[u8]) -> Result<ScipDoc, ScipError> {
        let index = proto::Index::decode(scip_bytes)?;
        Ok(ScipDoc {
            lang: self.lang(),
            index,
        })
    }
}
