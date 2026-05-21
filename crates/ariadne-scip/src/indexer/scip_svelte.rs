//! `ariadne-sfc-scip` Svelte bridge driver.
//!
//! Invocation: `ariadne-sfc-scip --framework svelte --cwd <root> --output <out>`.
//! The bridge is the same Node CLI the Vue driver invokes, vendored under
//! `tools/ariadne-sfc-scip/`, built and placed on PATH separately from the
//! Cargo workspace — like `scip-typescript`, it is never linked into the
//! `ariadne` binary (plan.md D5, D10). In `--framework svelte` mode it
//! transpiles each `.svelte` file to TypeScript with `svelte2tsx`, type-checks
//! the generated program, and emits SCIP whose occurrences key to the original
//! SFC source [src: docs/adr/0013-scip-sfc-bridge.md].

use std::path::{Path, PathBuf};
use std::process::Command;

use ariadne_core::Lang;
use prost::Message as _;

use crate::errors::ScipError;
use crate::indexer::subprocess::{ensure_parent, run_indexer};
use crate::indexer::{ScipDoc, ScipIndexer};
use crate::proto;

/// `ariadne-sfc-scip` Svelte bridge driver. Detect-fires when a `package.json`
/// names `svelte` as a dependency and the tree carries at least one `.svelte`
/// file.
#[derive(Debug, Default)]
pub struct ScipSvelteIndexer {
    binary: Option<PathBuf>,
}

impl ScipSvelteIndexer {
    /// Default driver (`ariadne-sfc-scip` on PATH).
    #[must_use]
    pub fn new() -> Self {
        Self { binary: None }
    }

    /// Driver pointed at an explicit binary path. Used by tests to inject a
    /// missing path so the `IndexerMissing` degrade arm is exercised.
    #[must_use]
    pub fn with_binary(binary: impl Into<PathBuf>) -> Self {
        Self {
            binary: Some(binary.into()),
        }
    }

    fn binary_path(&self) -> &Path {
        self.binary
            .as_deref()
            .unwrap_or_else(|| Path::new("ariadne-sfc-scip"))
    }

    fn install_hint() -> &'static str {
        "build the vendored bridge: cd tools/ariadne-sfc-scip && npm ci && npm run build, then put ariadne-sfc-scip on PATH"
    }
}

/// `true` when `root/package.json` contains the quoted token `"svelte"`. The
/// check is a substring scan rather than a JSON parse: `ariadne-scip` carries
/// no JSON dependency. The scan is a heuristic — it matches `"svelte"` anywhere
/// in the manifest, not solely the dependency key, so a `keywords: ["svelte"]`
/// array (or any other `"svelte"` string) also passes. This is deliberately
/// loose: `detect` additionally requires a real `.svelte` file, and a false
/// positive degrades gracefully. `"svelte2tsx"` and `"@sveltejs/kit"` do not
/// contain the quoted token.
fn package_declares_svelte(root: &Path) -> bool {
    std::fs::read_to_string(root.join("package.json"))
        .is_ok_and(|contents| contents.contains("\"svelte\""))
}

/// `true` when at least one `.svelte` file exists under `root`, skipping
/// `node_modules` and dot-directories. Filesystem-only — no subprocess, per the
/// [`ScipIndexer::detect`] contract.
fn has_svelte_file(root: &Path) -> bool {
    let Ok(entries) = std::fs::read_dir(root) else {
        return false;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_dir() {
            let skip = entry
                .file_name()
                .to_str()
                .is_none_or(|name| name == "node_modules" || name.starts_with('.'));
            if !skip && has_svelte_file(&path) {
                return true;
            }
        } else if path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("svelte"))
        {
            return true;
        }
    }
    false
}

impl ScipIndexer for ScipSvelteIndexer {
    fn lang(&self) -> Lang {
        Lang::Svelte
    }

    fn detect(&self, root: &Path) -> bool {
        package_declares_svelte(root) && has_svelte_file(root)
    }

    fn run(&self, root: &Path, out: &Path) -> Result<(), ScipError> {
        let binary = self.binary_path();
        ensure_parent(out)?;
        let mut cmd = Command::new(binary);
        cmd.arg("--framework")
            .arg("svelte")
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

    use crate::errors::ScipError;
    use crate::indexer::{ScipIndexer, ScipSvelteIndexer};

    fn write(dir: &Path, name: &str, contents: &str) {
        fs::write(dir.join(name), contents).expect("fixture file must write");
    }

    #[test]
    fn detect_fires_on_svelte_dep_and_svelte_file() {
        let dir = tempfile::tempdir().unwrap();
        write(
            dir.path(),
            "package.json",
            "{\"dependencies\":{\"svelte\":\"^5.0.0\"}}",
        );
        write(dir.path(), "App.svelte", "<main></main>\n");
        assert!(ScipSvelteIndexer::new().detect(dir.path()));
    }

    #[test]
    fn detect_skips_package_without_svelte_dep() {
        let dir = tempfile::tempdir().unwrap();
        write(
            dir.path(),
            "package.json",
            "{\"devDependencies\":{\"svelte2tsx\":\"^0.7.0\"}}",
        );
        write(dir.path(), "App.svelte", "<main></main>\n");
        assert!(
            !ScipSvelteIndexer::new().detect(dir.path()),
            "a package.json without a `svelte` dependency is not a Svelte project signal",
        );
    }

    #[test]
    fn detect_skips_svelte_dep_without_svelte_file() {
        let dir = tempfile::tempdir().unwrap();
        write(
            dir.path(),
            "package.json",
            "{\"dependencies\":{\"svelte\":\"^5.0.0\"}}",
        );
        assert!(
            !ScipSvelteIndexer::new().detect(dir.path()),
            "a `svelte` dependency without any .svelte file is not indexable",
        );
    }

    #[test]
    fn detect_finds_nested_svelte_file() {
        let dir = tempfile::tempdir().unwrap();
        write(
            dir.path(),
            "package.json",
            "{\"dependencies\":{\"svelte\":\"^5.0.0\"}}",
        );
        fs::create_dir(dir.path().join("src")).unwrap();
        write(&dir.path().join("src"), "App.svelte", "<main></main>\n");
        assert!(ScipSvelteIndexer::new().detect(dir.path()));
    }

    #[test]
    fn run_with_missing_binary_degrades_to_indexer_missing() {
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("out.scip");
        let driver = ScipSvelteIndexer::with_binary(dir.path().join("no-such-ariadne-sfc-scip"));
        let err = driver
            .run(dir.path(), &out)
            .expect_err("a missing bridge binary must not succeed");
        assert!(
            matches!(err, ScipError::IndexerMissing { .. }),
            "a missing bridge binary must degrade to IndexerMissing, got {err:?}",
        );
    }
}
