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

/// `scip-typescript` driver. Detect-fires when `package.json` plus either a
/// `tsconfig.json` or a `jsconfig.json` sit at the project root — the latter
/// covers JS-only React/Solid apps that ship `jsconfig.json` instead of
/// `tsconfig.json`.
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
        root.join("package.json").is_file()
            && (root.join("tsconfig.json").is_file() || root.join("jsconfig.json").is_file())
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

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use crate::indexer::{ScipIndexer, ScipTypescriptIndexer};

    fn touch(dir: &Path, name: &str) {
        fs::write(dir.join(name), "{}\n").expect("fixture file must write");
    }

    #[test]
    fn detect_fires_on_package_and_tsconfig() {
        let dir = tempfile::tempdir().unwrap();
        touch(dir.path(), "package.json");
        touch(dir.path(), "tsconfig.json");
        assert!(ScipTypescriptIndexer::new().detect(dir.path()));
    }

    #[test]
    fn detect_fires_on_package_and_jsconfig() {
        let dir = tempfile::tempdir().unwrap();
        touch(dir.path(), "package.json");
        touch(dir.path(), "jsconfig.json");
        assert!(
            ScipTypescriptIndexer::new().detect(dir.path()),
            "a JS-only React/Solid app keyed on jsconfig.json must be detected",
        );
    }

    #[test]
    fn detect_skips_package_only() {
        let dir = tempfile::tempdir().unwrap();
        touch(dir.path(), "package.json");
        assert!(
            !ScipTypescriptIndexer::new().detect(dir.path()),
            "package.json alone is not a TypeScript/JavaScript project signal",
        );
    }

    #[test]
    fn detect_skips_config_without_package() {
        let dir = tempfile::tempdir().unwrap();
        touch(dir.path(), "tsconfig.json");
        assert!(
            !ScipTypescriptIndexer::new().detect(dir.path()),
            "scip-typescript requires package.json for the symbol scheme",
        );
    }
}
