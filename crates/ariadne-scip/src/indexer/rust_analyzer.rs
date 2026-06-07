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
        let mut cmd = build_command(binary, root, &out_dir)?;
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

/// Build `rust-analyzer scip <root>` with CWD set to `out_dir`.
///
/// The project path is canonicalized to an absolute path before it is
/// passed as the `scip` argument. CWD is moved off the project (to
/// `out_dir`, where rust-analyzer drops `index.scip`), so a relative
/// `root` — e.g. the CLI's default `.` — would otherwise be re-resolved
/// against `out_dir`, where there is no `Cargo.toml`, and rust-analyzer
/// aborts with `Error: no projects`
/// [src: `project_model::ProjectManifest::discover_single`].
fn build_command(binary: &Path, root: &Path, out_dir: &Path) -> Result<Command, ScipError> {
    let abs_root = root.canonicalize().map_err(|source| ScipError::Io {
        path: root.to_path_buf(),
        source,
    })?;
    let mut cmd = Command::new(binary);
    cmd.arg("scip").arg(abs_root).current_dir(out_dir);
    Ok(cmd)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Regression: a relative `root` combined with `current_dir(out_dir)`
    /// made rust-analyzer resolve the project path against the temp out-dir
    /// and abort with `Error: no projects`. The path handed to the indexer
    /// must be absolute regardless of the caller's (relative) root.
    #[test]
    fn passes_absolute_root_even_when_cwd_moves_to_out_dir() {
        let out_dir = std::env::temp_dir();
        let cmd = build_command(Path::new("rust-analyzer"), Path::new("."), &out_dir)
            .expect("the process working directory must be canonicalizable");

        let args: Vec<_> = cmd.get_args().collect();
        assert_eq!(args[0], "scip", "first arg is the scip subcommand");

        let root_arg = Path::new(args[1]);
        assert!(
            root_arg.is_absolute(),
            "project path handed to rust-analyzer must be absolute, got {root_arg:?}",
        );
        assert_eq!(
            root_arg,
            Path::new(".")
                .canonicalize()
                .expect("the process working directory must be canonicalizable"),
            "relative `.` must be resolved against the process CWD, not out_dir",
        );
        assert_eq!(
            cmd.get_current_dir(),
            Some(out_dir.as_path()),
            "CWD must be the out-dir so rust-analyzer drops index.scip there",
        );
    }
}
