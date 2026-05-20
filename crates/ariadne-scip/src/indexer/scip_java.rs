//! `scip-java` driver.
//!
//! Invocation: `scip-java index --output <out> --build-tool <gradle|maven|bazel|sbt>`
//! \[src: `.claude/plans/ariadne-core/tier-05-scip-ingest.md` step 8,
//! <https://github.com/sourcegraph/scip-java>]. The `--build-tool` flag
//! is mandatory; we pick the first marker present on the project root.
//! `detect` covers any of the four marker files / dirs.

use std::path::{Path, PathBuf};
use std::process::Command;

use ariadne_core::Lang;
use prost::Message as _;

use crate::errors::ScipError;
use crate::indexer::subprocess::{ensure_parent, run_indexer};
use crate::indexer::{ScipDoc, ScipIndexer};
use crate::proto;

/// `scip-java` driver. Detect-fires on a Gradle / Maven / Bazel / sbt
/// project marker.
#[derive(Debug, Default)]
pub struct ScipJavaIndexer {
    binary: Option<PathBuf>,
}

impl ScipJavaIndexer {
    /// Default driver (`scip-java` on PATH).
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
            .unwrap_or_else(|| Path::new("scip-java"))
    }

    fn install_hint() -> &'static str {
        "coursier install scip-java (or `brew install sourcegraph/scip/scip-java`)"
    }

    fn build_tool(root: &Path) -> Option<&'static str> {
        if root.join("build.gradle").is_file()
            || root.join("build.gradle.kts").is_file()
            || root.join("settings.gradle").is_file()
            || root.join("settings.gradle.kts").is_file()
        {
            return Some("gradle");
        }
        if root.join("pom.xml").is_file() {
            return Some("maven");
        }
        if root.join("BUILD").is_file()
            || root.join("BUILD.bazel").is_file()
            || root.join("WORKSPACE").is_file()
            || root.join("WORKSPACE.bazel").is_file()
        {
            return Some("bazel");
        }
        if root.join("build.sbt").is_file() {
            return Some("sbt");
        }
        None
    }
}

impl ScipIndexer for ScipJavaIndexer {
    fn lang(&self) -> Lang {
        Lang::Java
    }

    fn detect(&self, root: &Path) -> bool {
        Self::build_tool(root).is_some()
    }

    fn run(&self, root: &Path, out: &Path) -> Result<(), ScipError> {
        let binary = self.binary_path();
        let build_tool = Self::build_tool(root).ok_or_else(|| ScipError::Io {
            path: root.to_path_buf(),
            source: std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "no Gradle/Maven/Bazel/sbt marker at root",
            ),
        })?;
        ensure_parent(out)?;
        let mut cmd = Command::new(binary);
        cmd.arg("index")
            .arg("--output")
            .arg(out)
            .arg("--build-tool")
            .arg(build_tool)
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
