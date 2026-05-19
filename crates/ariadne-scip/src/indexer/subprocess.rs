//! Shared spawn helper used by every SCIP driver.
//!
//! Each per-language driver configures its own [`Command`] (binary, args,
//! cwd). The shared concerns are uniform: turn `ErrorKind::NotFound` into
//! [`ScipError::IndexerMissing`] so a missing toolchain degrades to a
//! warning rather than a crash (plan §scope:
//! "missing indexers degrade to syntactic-only … never crash"
//! [src: `.claude/plans/ariadne-core/tier-05-scip-ingest.md`]), bound the
//! captured stderr so a runaway indexer cannot blow the [`IngestReport`]
//! memory budget, and surface non-zero exit codes as a structured
//! [`ScipError::SubprocessFailed`].

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::errors::ScipError;

const STDERR_CHAR_LIMIT: usize = 4096;

pub(crate) fn run_indexer(
    binary: &str,
    install_hint: &str,
    context_path: &Path,
    cmd: &mut Command,
) -> Result<(), ScipError> {
    let output = match cmd.output() {
        Ok(o) => o,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return Err(ScipError::IndexerMissing {
                binary: binary.to_owned(),
                install_hint: install_hint.to_owned(),
            });
        }
        Err(err) => {
            return Err(ScipError::Io {
                path: context_path.to_path_buf(),
                source: err,
            });
        }
    };
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr)
            .chars()
            .take(STDERR_CHAR_LIMIT)
            .collect();
        return Err(ScipError::SubprocessFailed {
            binary: binary.to_owned(),
            status: output.status.code().unwrap_or(-1),
            stderr,
        });
    }
    Ok(())
}

pub(crate) fn ensure_parent(out: &Path) -> Result<PathBuf, ScipError> {
    let parent = out
        .parent()
        .ok_or_else(|| ScipError::Io {
            path: out.to_path_buf(),
            source: std::io::Error::new(std::io::ErrorKind::InvalidInput, "out path has no parent"),
        })?
        .to_path_buf();
    std::fs::create_dir_all(&parent).map_err(|source| ScipError::Io {
        path: parent.clone(),
        source,
    })?;
    Ok(parent)
}
