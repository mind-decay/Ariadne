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
    ///
    /// Two autodetect signals feed `enabled_langs`: a source file whose
    /// extension maps to a [`Lang`], and a framework dependency named in a
    /// root `package.json` (`react`/`vue`/`svelte`/`astro`/`solid-js`) — so
    /// a `.vue`/`.svelte`/`.astro`/`.tsx` repo, or one that merely declares
    /// the dependency, enables the framework langs [src: tier-05 step 5].
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
        detect_package_json_langs(root, &mut tags);
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

/// Framework `package.json` dependency → the [`Lang`] tags its presence
/// enables. React and `SolidJS` share the JSX (`javascript`) / TSX (`tsx`)
/// path — no dedicated grammar (plan.md D3, D7); Vue / Svelte / Astro each
/// map to their own host lang [src: tier-05 step 5].
const FRAMEWORK_DEPS: &[(&str, &[&str])] = &[
    ("react", &["javascript", "tsx"]),
    ("solid-js", &["javascript", "tsx"]),
    ("vue", &["vue"]),
    ("svelte", &["svelte"]),
    ("astro", &["astro"]),
];

/// Append language tags implied by framework dependencies in a root
/// `package.json`. Best-effort: a missing or malformed `package.json`, or
/// one naming no recognised framework, leaves `tags` untouched. A dependency
/// counts whether it sits under `dependencies` or `devDependencies`.
fn detect_package_json_langs(root: &Path, tags: &mut Vec<String>) {
    let Ok(text) = std::fs::read_to_string(root.join("package.json")) else {
        return;
    };
    let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) else {
        return;
    };
    for (dep, dep_tags) in FRAMEWORK_DEPS {
        let declared = ["dependencies", "devDependencies"].iter().any(|section| {
            json.get(section)
                .and_then(serde_json::Value::as_object)
                .is_some_and(|deps| deps.contains_key(*dep))
        });
        if declared {
            for tag in *dep_tags {
                if !tags.iter().any(|t| t == tag) {
                    tags.push((*tag).to_owned());
                }
            }
        }
    }
}

/// Resolve `binary` against `$PATH`, returning the first executable match.
#[must_use]
pub fn resolve_on_path(binary: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(binary))
        .find(|candidate| candidate.is_file())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A root `package.json` naming a framework dependency enables its
    /// langs even when the repo carries no source file of that framework
    /// [src: tier-05 `exit_criteria` #4 — "or matching `package.json` deps"].
    #[test]
    fn package_json_deps_enable_framework_langs() {
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            tmp.path().join("package.json"),
            r#"{ "dependencies": { "vue": "^3.4.0" },
                "devDependencies": { "svelte": "^4.2.0", "react": "^18.2.0" } }"#,
        )
        .expect("write package.json");

        let config = Config::detect(tmp.path());

        for tag in ["vue", "svelte", "tsx", "javascript"] {
            assert!(
                config.enabled_langs.iter().any(|t| t == tag),
                "expected `{tag}` enabled from package.json deps; got {:?}",
                config.enabled_langs,
            );
        }
    }

    /// A repo with neither framework files nor a `package.json` detects no
    /// framework langs — the autodetect stays a signal, never a default.
    #[test]
    fn no_package_json_leaves_framework_langs_off() {
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::write(tmp.path().join("main.rs"), "fn main() {}\n").expect("write rs");

        let config = Config::detect(tmp.path());

        assert_eq!(config.enabled_langs, vec!["rust".to_owned()]);
    }
}
