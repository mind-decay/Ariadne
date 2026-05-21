//! `.ariadne/config.toml` — per-project configuration.
//!
//! `init` auto-detects enabled languages from repo signals and writes the
//! file; `load` reads it back and layers `ARIADNE_*` environment overrides on
//! top [src: .claude/plans/ariadne-core/tier-10-cli-e2e.md `exit_criteria` #2].

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use ariadne_core::Lang;
use ignore::WalkBuilder;
use serde::{Deserialize, Serialize};

use crate::domain::lang_for_path;

/// Current config schema version. A mismatch is a hard load error.
pub const SCHEMA_VERSION: u32 = 1;

/// Standard SCIP indexer binaries, paired with the language each serves.
/// Used by `init` (PATH probe) and `status` (availability matrix).
pub const INDEXER_BINARIES: &[(&str, &str)] = &[
    ("rust", "rust-analyzer"),
    ("typescript", "scip-typescript"),
    ("python", "scip-python"),
    ("java", "scip-java"),
    ("csharp", "scip-dotnet"),
    ("c/c++", "scip-clang"),
    ("go", "lsif-go"),
];

/// Parsed `.ariadne/config.toml`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Schema version; must equal [`SCHEMA_VERSION`].
    pub schema_version: u32,
    /// Languages the indexer parses, by [`Lang`] tag.
    pub enabled_langs: Vec<String>,
    /// Whether the file walk honours `.gitignore` / `.git/info/exclude`.
    pub respect_gitignore: bool,
    /// Extra path segments to skip during the walk.
    pub ignore: Vec<String>,
    /// Optional explicit indexer-binary paths discovered by `init`.
    #[serde(default)]
    pub indexers: BTreeMap<String, String>,
}

/// Hard-coded ignore segments seeded into every generated config.
fn default_ignores() -> Vec<String> {
    [
        "target",
        "node_modules",
        ".git",
        ".ariadne",
        "dist",
        "build",
        "vendor",
        "__pycache__",
    ]
    .iter()
    .map(|s| (*s).to_owned())
    .collect()
}

impl Config {
    /// `<root>/.ariadne/config.toml`.
    #[must_use]
    pub fn path(root: &Path) -> PathBuf {
        root.join(".ariadne").join("config.toml")
    }

    /// Load + validate the project config, then apply `ARIADNE_*` overrides.
    ///
    /// # Errors
    /// Fails when the file is missing, malformed, or carries a foreign
    /// schema version.
    pub fn load(root: &Path) -> Result<Self> {
        let path = Self::path(root);
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("read {} (run `ariadne init` first)", path.display()))?;
        let mut config: Self =
            toml::from_str(&text).with_context(|| format!("parse {}", path.display()))?;
        config.apply_env_overrides();
        config.validate()?;
        Ok(config)
    }

    /// Build a default config by detecting languages present under `root`.
    #[must_use]
    pub fn detect(root: &Path) -> Self {
        let mut tags: Vec<String> = Vec::new();
        for entry in WalkBuilder::new(root).build().flatten() {
            if !entry.file_type().is_some_and(|t| t.is_file()) {
                continue;
            }
            if let Some(lang) = lang_for_path(entry.path()) {
                let tag = lang.tag();
                if !tags.contains(&tag) {
                    tags.push(tag);
                }
            }
        }
        tags.sort();
        Self {
            schema_version: SCHEMA_VERSION,
            enabled_langs: tags,
            respect_gitignore: true,
            ignore: default_ignores(),
            indexers: BTreeMap::new(),
        }
    }

    /// Write the config to `<root>/.ariadne/config.toml`, creating dirs.
    ///
    /// # Errors
    /// Propagates directory-creation and write failures.
    pub fn write(&self, root: &Path) -> Result<()> {
        let path = Self::path(root);
        std::fs::create_dir_all(path.parent().expect("config path has a parent"))
            .context("create .ariadne directory")?;
        let text = toml::to_string_pretty(self).context("serialize config")?;
        std::fs::write(&path, text).with_context(|| format!("write {}", path.display()))?;
        Ok(())
    }

    /// Decode `enabled_langs` tags into [`Lang`] values, dropping unknowns.
    #[must_use]
    pub fn enabled_langs(&self) -> Vec<Lang> {
        self.enabled_langs
            .iter()
            .filter_map(|t| Lang::from_tag(t))
            .collect()
    }

    /// Layer `ARIADNE_*` environment overrides over the parsed config.
    fn apply_env_overrides(&mut self) {
        if let Ok(v) = std::env::var("ARIADNE_ENABLED_LANGS") {
            self.enabled_langs = split_csv(&v);
        }
        if let Ok(v) = std::env::var("ARIADNE_IGNORE") {
            self.ignore = split_csv(&v);
        }
        if let Ok(v) = std::env::var("ARIADNE_RESPECT_GITIGNORE") {
            self.respect_gitignore = matches!(v.trim(), "1" | "true" | "yes");
        }
    }

    /// Reject foreign schema versions and unknown language tags.
    fn validate(&self) -> Result<()> {
        if self.schema_version != SCHEMA_VERSION {
            bail!(
                "config schema_version {} != supported {SCHEMA_VERSION}; \
                 delete .ariadne/ and re-run `ariadne init`",
                self.schema_version
            );
        }
        for tag in &self.enabled_langs {
            if Lang::from_tag(tag).is_none() {
                bail!("config enabled_langs carries unknown tag `{tag}`");
            }
        }
        Ok(())
    }
}

/// Split a comma-separated env value into trimmed, non-empty entries.
fn split_csv(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect()
}

/// Resolve `binary` against `$PATH`, returning the first executable match.
#[must_use]
pub fn resolve_on_path(binary: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(binary))
        .find(|candidate| candidate.is_file())
}
