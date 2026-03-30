pub mod gomod;
pub mod jsonc;
pub mod pyproject;
pub mod tsconfig;

use std::collections::BTreeMap;
use std::path::Path;

use crate::diagnostic::DiagnosticCollector;
use crate::model::FileSet;

pub use gomod::GoModConfig;
pub use pyproject::PyProjectConfig;
pub use tsconfig::TsConfig;

/// Aggregated project configuration discovered from config files.
pub struct ProjectConfig {
    /// TypeScript configs, keyed by directory containing tsconfig.json.
    pub ts_configs: BTreeMap<std::path::PathBuf, TsConfig>,
    /// Go module config.
    pub go_config: Option<GoModConfig>,
    /// Python project config.
    pub py_config: Option<PyProjectConfig>,
}

/// Discover and parse all project configuration files.
///
/// - TypeScript: scans `known_files` for `tsconfig.json` entries, parses each with extends resolution.
/// - Go: looks for `go.mod` at `project_root`.
/// - Python: looks for `pyproject.toml` at `project_root`.
pub fn discover_config(
    project_root: &Path,
    known_files: &FileSet,
    diagnostics: &DiagnosticCollector,
) -> ProjectConfig {
    let ts_configs = discover_tsconfigs(project_root, known_files, diagnostics);
    let go_config = discover_go_config(project_root);
    let py_config = discover_py_config(project_root);

    ProjectConfig {
        ts_configs,
        go_config,
        py_config,
    }
}

/// Scan known_files for tsconfig.json and parse each with extends resolution.
fn discover_tsconfigs(
    project_root: &Path,
    known_files: &FileSet,
    diagnostics: &DiagnosticCollector,
) -> BTreeMap<std::path::PathBuf, TsConfig> {
    let mut configs = BTreeMap::new();

    // Canonicalize project root once for stripping absolute prefixes below.
    let abs_root = std::fs::canonicalize(project_root).unwrap_or_else(|_| project_root.to_path_buf());

    for file in known_files.iter() {
        if file.file_name() == "tsconfig.json" {
            let full_path = project_root.join(file.as_str());
            if let Some(mut config) = tsconfig::parse_tsconfig_with_extends(&full_path, diagnostics) {
                // parse_tsconfig_with_extends sets config_dir to the canonical
                // (absolute) directory of the tsconfig.  The resolver, however,
                // works with project-relative CanonicalPaths, so we convert
                // config_dir to a project-relative path.  This ensures that
                // resolve_path_alias produces project-relative candidates that
                // match the project-relative FileSet entries.
                let rel_dir = config
                    .config_dir
                    .strip_prefix(&abs_root)
                    .unwrap_or(&config.config_dir)
                    .to_path_buf();
                config.config_dir = rel_dir.clone();
                configs.insert(rel_dir, config);
            }
        }
    }

    configs
}

/// Look for go.mod at project root and parse it.
fn discover_go_config(project_root: &Path) -> Option<GoModConfig> {
    let go_mod_path = project_root.join("go.mod");
    match std::fs::read_to_string(&go_mod_path) {
        Ok(content) => gomod::parse_go_mod(&content),
        Err(_) => None,
    }
}

/// Look for pyproject.toml at project root and parse it.
fn discover_py_config(project_root: &Path) -> Option<PyProjectConfig> {
    let pyproject_path = project_root.join("pyproject.toml");
    match std::fs::read_to_string(&pyproject_path) {
        Ok(content) => pyproject::parse_pyproject(&content),
        Err(_) => None,
    }
}

/// Find the nearest tsconfig.json for a given file directory (D-121).
///
/// Walks up from `file_dir`, checking if each ancestor is a key in `ts_configs`.
/// Returns the first match (nearest ancestor).
pub fn find_nearest_tsconfig<'a>(
    file_dir: &Path,
    ts_configs: &'a BTreeMap<std::path::PathBuf, TsConfig>,
) -> Option<&'a TsConfig> {
    let mut current = Some(file_dir);
    while let Some(dir) = current {
        if let Some(config) = ts_configs.get(dir) {
            return Some(config);
        }
        current = dir.parent();
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_nearest_tsconfig_exact_match() {
        let mut configs = BTreeMap::new();
        let config = TsConfig {
            config_dir: std::path::PathBuf::from("/project/src"),
            base_url: None,
            paths: BTreeMap::new(),
        };
        configs.insert(std::path::PathBuf::from("/project/src"), config);

        let result = find_nearest_tsconfig(
            std::path::Path::new("/project/src"),
            &configs,
        );
        assert!(result.is_some());
    }

    #[test]
    fn find_nearest_tsconfig_walks_up() {
        let mut configs = BTreeMap::new();
        let config = TsConfig {
            config_dir: std::path::PathBuf::from("/project"),
            base_url: Some(".".to_string()),
            paths: BTreeMap::new(),
        };
        configs.insert(std::path::PathBuf::from("/project"), config);

        let result = find_nearest_tsconfig(
            std::path::Path::new("/project/src/components"),
            &configs,
        );
        assert!(result.is_some());
        assert_eq!(result.unwrap().base_url.as_deref(), Some("."));
    }

    #[test]
    fn find_nearest_tsconfig_no_match() {
        let configs: BTreeMap<std::path::PathBuf, TsConfig> = BTreeMap::new();
        let result = find_nearest_tsconfig(
            std::path::Path::new("/other/path"),
            &configs,
        );
        assert!(result.is_none());
    }

    #[test]
    fn find_nearest_tsconfig_prefers_nearest() {
        let mut configs = BTreeMap::new();

        let root_config = TsConfig {
            config_dir: std::path::PathBuf::from("/project"),
            base_url: Some("root".to_string()),
            paths: BTreeMap::new(),
        };
        configs.insert(std::path::PathBuf::from("/project"), root_config);

        let nested_config = TsConfig {
            config_dir: std::path::PathBuf::from("/project/packages/app"),
            base_url: Some("nested".to_string()),
            paths: BTreeMap::new(),
        };
        configs.insert(
            std::path::PathBuf::from("/project/packages/app"),
            nested_config,
        );

        let result = find_nearest_tsconfig(
            std::path::Path::new("/project/packages/app/src"),
            &configs,
        );
        assert!(result.is_some());
        assert_eq!(result.unwrap().base_url.as_deref(), Some("nested"));
    }

    #[test]
    fn discover_config_with_empty_fileset() {
        let dir = tempfile::tempdir().unwrap();
        let diag = DiagnosticCollector::new();
        let files = FileSet::new();
        let config = discover_config(dir.path(), &files, &diag);

        assert!(config.ts_configs.is_empty());
        assert!(config.go_config.is_none());
        assert!(config.py_config.is_none());
    }

    #[test]
    fn discover_config_finds_go_mod() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("go.mod"),
            "module github.com/test/project\n\ngo 1.21\n",
        )
        .unwrap();

        let diag = DiagnosticCollector::new();
        let files = FileSet::new();
        let config = discover_config(dir.path(), &files, &diag);

        assert!(config.go_config.is_some());
        assert_eq!(
            config.go_config.unwrap().module_path,
            "github.com/test/project"
        );
    }

    #[test]
    fn discover_config_finds_pyproject() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("pyproject.toml"),
            "[project]\nname = \"myapp\"\n",
        )
        .unwrap();

        let diag = DiagnosticCollector::new();
        let files = FileSet::new();
        let config = discover_config(dir.path(), &files, &diag);

        assert!(config.py_config.is_some());
        assert_eq!(
            config.py_config.unwrap().package_name.as_deref(),
            Some("myapp")
        );
    }
}
