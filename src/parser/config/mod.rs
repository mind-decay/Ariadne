pub mod bundler;
pub mod csproj;
pub mod gomod;
pub mod gradle;
pub mod jsonc;
pub mod maven;
pub mod nextjs;
pub mod pyproject;
pub mod tsconfig;
pub mod turbo;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::diagnostic::{DiagnosticCollector, Warning, WarningCode};
use crate::model::{CanonicalPath, FileSet};

pub use bundler::BundlerConfig;
pub use csproj::{CsprojConfig, DotnetSolutionInfo};
pub use gomod::GoModConfig;
pub use nextjs::{NextRouteInfo, NextRouteKind, NextRouterType};
pub use turbo::TurboConfig;
pub use gradle::{GradleConfig, GradleDep, GradleDepScope, GradleSubproject};
pub use maven::{MavenConfig, MavenDep, MavenParentRef};
pub use pyproject::PyProjectConfig;
pub use tsconfig::TsConfig;

/// Aggregated project configuration discovered from config files.
#[derive(Debug)]
pub struct ProjectConfig {
    /// TypeScript configs, keyed by directory containing tsconfig.json.
    pub ts_configs: BTreeMap<std::path::PathBuf, TsConfig>,
    /// Go module config.
    pub go_config: Option<GoModConfig>,
    /// Python project config.
    pub py_config: Option<PyProjectConfig>,
    /// C# project configs, keyed by project-relative directory of the .csproj.
    pub csproj_configs: BTreeMap<std::path::PathBuf, CsprojConfig>,
    /// .NET solution info (from .sln file).
    pub dotnet_solution: Option<DotnetSolutionInfo>,
    /// Gradle build configs, keyed by directory containing build.gradle.
    pub gradle_configs: BTreeMap<std::path::PathBuf, GradleConfig>,
    /// Maven POM configs, keyed by directory containing pom.xml.
    pub maven_configs: BTreeMap<std::path::PathBuf, MavenConfig>,
    /// Bundler alias configs (Vite/Webpack), keyed by directory containing config file.
    pub bundler_configs: BTreeMap<std::path::PathBuf, BundlerConfig>,
    /// Turborepo pipeline config.
    pub turbo_config: Option<TurboConfig>,
    /// Next.js filesystem route info.
    pub next_routes: Option<NextRouteInfo>,
}

impl ProjectConfig {
    /// Flatten all discovered TypeScript path aliases into a single map.
    ///
    /// Phase 8c B1: Theseus embeds path aliases in planner / test_gen prompts
    /// so the LLM does not burn turns opening `tsconfig.json` to understand
    /// `@app/*`-style imports. When multiple tsconfigs define the same alias
    /// key the last-seen (BTreeMap iteration order — alphabetical by dir)
    /// wins; this is deterministic and intentionally simple.
    pub fn js_path_aliases(&self) -> BTreeMap<String, Vec<String>> {
        let mut out = BTreeMap::new();
        for ts in self.ts_configs.values() {
            for (k, v) in &ts.paths {
                out.insert(k.clone(), v.clone());
            }
        }
        out
    }

    /// List the project-relative paths of discovered config files that are
    /// worth naming in a prompt (tsconfig.json, pyproject.toml, go.mod, etc.).
    ///
    /// Phase 8c B1: downstream consumers embed this in prompts so the LLM
    /// knows where to look if it needs raw config. Paths are relative to
    /// whatever `project_root` the caller used during `discover_config`.
    pub fn discovered_config_files(&self) -> Vec<std::path::PathBuf> {
        let mut out: Vec<std::path::PathBuf> = Vec::new();

        for ts_dir in self.ts_configs.keys() {
            out.push(ts_dir.join("tsconfig.json"));
        }
        if self.go_config.is_some() {
            out.push(std::path::PathBuf::from("go.mod"));
        }
        if self.py_config.is_some() {
            out.push(std::path::PathBuf::from("pyproject.toml"));
        }
        for cs_dir in self.csproj_configs.keys() {
            // .csproj file name isn't stored on the struct; best-effort hint.
            out.push(cs_dir.join("*.csproj"));
        }
        for gradle_dir in self.gradle_configs.keys() {
            out.push(gradle_dir.join("build.gradle"));
        }
        for mvn_dir in self.maven_configs.keys() {
            out.push(mvn_dir.join("pom.xml"));
        }
        out.sort();
        out.dedup();
        out
    }
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
    let csproj_configs = discover_csproj_configs(project_root, known_files, diagnostics);
    let dotnet_solution = discover_dotnet_solution(project_root, known_files, diagnostics);
    let gradle_configs = discover_gradle_configs(project_root, known_files, diagnostics);
    let maven_configs = discover_maven_configs(project_root, known_files, diagnostics);
    let bundler_configs = discover_bundler_configs(project_root, known_files, diagnostics);
    let turbo_config = discover_turbo_config(project_root, known_files, diagnostics);
    let next_routes = nextjs::discover_next_routes(project_root, known_files, diagnostics);

    ProjectConfig {
        ts_configs,
        go_config,
        py_config,
        csproj_configs,
        dotnet_solution,
        gradle_configs,
        maven_configs,
        bundler_configs,
        turbo_config,
        next_routes,
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

/// Scan known_files for .csproj files, parse each, and return keyed by project-relative directory.
fn discover_csproj_configs(
    project_root: &Path,
    known_files: &FileSet,
    diagnostics: &DiagnosticCollector,
) -> BTreeMap<std::path::PathBuf, CsprojConfig> {
    let mut configs = BTreeMap::new();

    for file in known_files.iter() {
        let file_str = file.as_str();
        if !file_str.ends_with(".csproj") {
            continue;
        }

        let full_path = project_root.join(file_str);
        let content = match std::fs::read_to_string(&full_path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        // Use the project-relative path for parsing
        let rel_path = std::path::PathBuf::from(file_str);
        if let Some(config) = csproj::parse_csproj(&content, &rel_path, diagnostics) {
            let rel_dir = rel_path
                .parent()
                .unwrap_or(Path::new(""))
                .to_path_buf();
            configs.insert(rel_dir, config);
        }
    }

    configs
}

/// Scan known_files for .sln files. If multiple found, emit W037 and use first alphabetically.
fn discover_dotnet_solution(
    project_root: &Path,
    known_files: &FileSet,
    diagnostics: &DiagnosticCollector,
) -> Option<DotnetSolutionInfo> {
    let mut sln_files: Vec<String> = Vec::new();

    for file in known_files.iter() {
        let file_str = file.as_str();
        if file_str.ends_with(".sln") {
            sln_files.push(file_str.to_string());
        }
    }

    if sln_files.is_empty() {
        return None;
    }

    // Sort for determinism (BTreeSet-like behavior)
    sln_files.sort();

    if sln_files.len() > 1 {
        diagnostics.warn(Warning {
            code: WarningCode::W037MultipleSlnFiles,
            path: CanonicalPath::new(sln_files[0].clone()),
            message: format!(
                "multiple .sln files found ({}), using first alphabetically: {}",
                sln_files.len(),
                sln_files[0]
            ),
            detail: None,
        });
    }

    let sln_path_str = &sln_files[0];
    let full_path = project_root.join(sln_path_str);
    let content = match std::fs::read_to_string(&full_path) {
        Ok(c) => c,
        Err(_) => return None,
    };

    let rel_path = std::path::PathBuf::from(sln_path_str);
    csproj::parse_sln(&content, &rel_path, diagnostics)
}

/// Scan known_files for build.gradle / build.gradle.kts, parse each, and return keyed by directory.
fn discover_gradle_configs(
    project_root: &Path,
    known_files: &FileSet,
    diagnostics: &DiagnosticCollector,
) -> BTreeMap<PathBuf, GradleConfig> {
    let mut configs = BTreeMap::new();

    for file in known_files.iter() {
        let file_name = file.file_name();
        if file_name != "build.gradle" && file_name != "build.gradle.kts" {
            continue;
        }

        let full_path = project_root.join(file.as_str());
        let content = match std::fs::read_to_string(&full_path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let rel_path = PathBuf::from(file.as_str());
        let rel_dir = rel_path.parent().unwrap_or(Path::new("")).to_path_buf();

        if let Some(mut config) = gradle::parse_build_gradle(&content, &rel_dir, diagnostics) {
            // Look for settings.gradle or settings.gradle.kts in same directory
            let settings_names = ["settings.gradle", "settings.gradle.kts"];
            for settings_name in &settings_names {
                let settings_rel = if rel_dir == PathBuf::from("") {
                    PathBuf::from(settings_name)
                } else {
                    rel_dir.join(settings_name)
                };
                let settings_full = project_root.join(&settings_rel);
                if let Ok(settings_content) = std::fs::read_to_string(&settings_full) {
                    let subprojects = gradle::parse_settings_gradle(&settings_content);
                    // Check that declared subproject directories exist
                    for sp in &subprojects {
                        let sp_build_rel = if rel_dir == PathBuf::from("") {
                            sp.path.join("build.gradle")
                        } else {
                            rel_dir.join(&sp.path).join("build.gradle")
                        };
                        let sp_build_kts_rel = if rel_dir == PathBuf::from("") {
                            sp.path.join("build.gradle.kts")
                        } else {
                            rel_dir.join(&sp.path).join("build.gradle.kts")
                        };
                        let sp_build_canon = CanonicalPath::new(
                            sp_build_rel.to_string_lossy().replace('\\', "/"),
                        );
                        let sp_build_kts_canon = CanonicalPath::new(
                            sp_build_kts_rel.to_string_lossy().replace('\\', "/"),
                        );
                        if !known_files.contains(&sp_build_canon)
                            && !known_files.contains(&sp_build_kts_canon)
                        {
                            diagnostics.warn(Warning {
                                code: WarningCode::W041GradleSubprojectNotFound,
                                path: CanonicalPath::new(sp.name.clone()),
                                message: format!(
                                    "Gradle subproject '{}' declared in settings.gradle but no build.gradle found",
                                    sp.name
                                ),
                                detail: None,
                            });
                        }
                    }
                    config.subprojects = subprojects;
                    break;
                }
            }

            configs.insert(rel_dir, config);
        }
    }

    configs
}

/// Scan known_files for pom.xml files, parse each, and return keyed by directory.
fn discover_maven_configs(
    project_root: &Path,
    known_files: &FileSet,
    diagnostics: &DiagnosticCollector,
) -> BTreeMap<PathBuf, MavenConfig> {
    let mut configs = BTreeMap::new();

    for file in known_files.iter() {
        if file.file_name() != "pom.xml" {
            continue;
        }

        let full_path = project_root.join(file.as_str());
        let content = match std::fs::read_to_string(&full_path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let rel_path = PathBuf::from(file.as_str());
        if let Some(config) = maven::parse_pom_xml(&content, &rel_path, diagnostics) {
            let rel_dir = rel_path.parent().unwrap_or(Path::new("")).to_path_buf();

            // Check that declared modules exist
            for module in &config.modules {
                let module_pom_rel = if rel_dir == PathBuf::from("") {
                    PathBuf::from(module).join("pom.xml")
                } else {
                    rel_dir.join(module).join("pom.xml")
                };
                let module_pom_canon = CanonicalPath::new(
                    module_pom_rel.to_string_lossy().replace('\\', "/"),
                );
                if !known_files.contains(&module_pom_canon) {
                    diagnostics.warn(Warning {
                        code: WarningCode::W040MavenModuleNotFound,
                        path: CanonicalPath::new(module.clone()),
                        message: format!(
                            "Maven module '{}' declared in pom.xml but {}/pom.xml not found",
                            module, module
                        ),
                        detail: None,
                    });
                }
            }

            configs.insert(rel_dir, config);
        }
    }

    configs
}

/// Scan known_files for Vite/Webpack config files, parse each.
fn discover_bundler_configs(
    project_root: &Path,
    known_files: &FileSet,
    diagnostics: &DiagnosticCollector,
) -> BTreeMap<PathBuf, BundlerConfig> {
    let mut configs = BTreeMap::new();

    let vite_names = ["vite.config.ts", "vite.config.js", "vite.config.mjs"];
    let webpack_names = ["webpack.config.js", "webpack.config.ts"];

    for file in known_files.iter() {
        let file_name = file.file_name();
        let is_vite = vite_names.contains(&file_name);
        let is_webpack = webpack_names.contains(&file_name);

        if !is_vite && !is_webpack {
            continue;
        }

        let full_path = project_root.join(file.as_str());
        let content = match std::fs::read(&full_path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let rel_path = PathBuf::from(file.as_str());
        let config = if is_vite {
            bundler::parse_vite_config(&content, &rel_path, diagnostics)
        } else {
            bundler::parse_webpack_config(&content, &rel_path, diagnostics)
        };

        if let Some(c) = config {
            let rel_dir = rel_path.parent().unwrap_or(Path::new("")).to_path_buf();
            configs.insert(rel_dir, c);
        }
    }

    configs
}

/// Scan known_files for turbo.json, parse it.
fn discover_turbo_config(
    project_root: &Path,
    known_files: &FileSet,
    diagnostics: &DiagnosticCollector,
) -> Option<TurboConfig> {
    for file in known_files.iter() {
        if file.file_name() == "turbo.json" {
            let full_path = project_root.join(file.as_str());
            let content = match std::fs::read(&full_path) {
                Ok(c) => c,
                Err(_) => continue,
            };
            let rel_path = PathBuf::from(file.as_str());
            return turbo::parse_turbo_config(&content, &rel_path, diagnostics);
        }
    }
    None
}

/// Find the nearest .csproj config for a given file directory.
///
/// Walks up from `file_dir`, checking if each ancestor is a key in `csproj_configs`.
/// Returns the first match (nearest ancestor).
pub fn find_nearest_csproj<'a>(
    file_dir: &Path,
    csproj_configs: &'a BTreeMap<std::path::PathBuf, CsprojConfig>,
) -> Option<&'a CsprojConfig> {
    let mut current = Some(file_dir);
    while let Some(dir) = current {
        if let Some(config) = csproj_configs.get(dir) {
            return Some(config);
        }
        current = dir.parent();
    }
    None
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

/// Find the nearest bundler config (Vite/Webpack) for a given file directory.
///
/// Walks up from `file_dir`, checking if each ancestor is a key in `bundler_configs`.
/// Returns the first match (nearest ancestor).
pub fn find_nearest_bundler<'a>(
    file_dir: &Path,
    bundler_configs: &'a BTreeMap<PathBuf, BundlerConfig>,
) -> Option<&'a BundlerConfig> {
    let mut current = Some(file_dir);
    while let Some(dir) = current {
        if let Some(config) = bundler_configs.get(dir) {
            return Some(config);
        }
        current = dir.parent();
    }
    None
}

/// Find the nearest Gradle build config for a given file directory.
///
/// Walks up from `file_dir`, checking if each ancestor is a key in `gradle_configs`.
/// Returns the first match (nearest ancestor).
pub fn find_nearest_gradle<'a>(
    file_dir: &Path,
    gradle_configs: &'a BTreeMap<PathBuf, GradleConfig>,
) -> Option<&'a GradleConfig> {
    let mut current = Some(file_dir);
    while let Some(dir) = current {
        if let Some(config) = gradle_configs.get(dir) {
            return Some(config);
        }
        current = dir.parent();
    }
    None
}

/// Find the nearest Maven POM config for a given file directory.
///
/// Walks up from `file_dir`, checking if each ancestor is a key in `maven_configs`.
/// Returns the first match (nearest ancestor).
pub fn find_nearest_maven<'a>(
    file_dir: &Path,
    maven_configs: &'a BTreeMap<PathBuf, MavenConfig>,
) -> Option<&'a MavenConfig> {
    let mut current = Some(file_dir);
    while let Some(dir) = current {
        if let Some(config) = maven_configs.get(dir) {
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
        assert!(config.csproj_configs.is_empty());
        assert!(config.dotnet_solution.is_none());
        assert!(config.gradle_configs.is_empty());
        assert!(config.maven_configs.is_empty());
        assert!(config.bundler_configs.is_empty());
        assert!(config.turbo_config.is_none());
        assert!(config.next_routes.is_none());
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

    // --- Phase 8c B1: flattened aliases + discovered config file listing ---

    fn empty_config() -> ProjectConfig {
        ProjectConfig {
            ts_configs: BTreeMap::new(),
            go_config: None,
            py_config: None,
            csproj_configs: BTreeMap::new(),
            dotnet_solution: None,
            gradle_configs: BTreeMap::new(),
            maven_configs: BTreeMap::new(),
            bundler_configs: BTreeMap::new(),
            turbo_config: None,
            next_routes: None,
        }
    }

    #[test]
    fn js_path_aliases_empty_when_no_tsconfig() {
        let cfg = empty_config();
        assert!(cfg.js_path_aliases().is_empty());
    }

    #[test]
    fn js_path_aliases_flattens_single_tsconfig() {
        let mut cfg = empty_config();
        let mut paths = BTreeMap::new();
        paths.insert("@app/*".to_string(), vec!["src/app/*".to_string()]);
        paths.insert(
            "@shared/*".to_string(),
            vec!["src/shared/*".to_string(), "packages/shared/*".to_string()],
        );
        cfg.ts_configs.insert(
            std::path::PathBuf::from(""),
            TsConfig {
                config_dir: std::path::PathBuf::from(""),
                base_url: Some(".".into()),
                paths,
            },
        );
        let flat = cfg.js_path_aliases();
        assert_eq!(flat.len(), 2);
        assert_eq!(flat.get("@app/*").unwrap(), &vec!["src/app/*".to_string()]);
        assert_eq!(flat.get("@shared/*").unwrap().len(), 2);
    }

    #[test]
    fn js_path_aliases_merges_multiple_tsconfigs() {
        let mut cfg = empty_config();
        let mut paths_a = BTreeMap::new();
        paths_a.insert("@a/*".to_string(), vec!["pkgs/a/*".to_string()]);
        let mut paths_b = BTreeMap::new();
        paths_b.insert("@b/*".to_string(), vec!["pkgs/b/*".to_string()]);
        cfg.ts_configs.insert(
            std::path::PathBuf::from("packages/a"),
            TsConfig {
                config_dir: std::path::PathBuf::from("packages/a"),
                base_url: None,
                paths: paths_a,
            },
        );
        cfg.ts_configs.insert(
            std::path::PathBuf::from("packages/b"),
            TsConfig {
                config_dir: std::path::PathBuf::from("packages/b"),
                base_url: None,
                paths: paths_b,
            },
        );
        let flat = cfg.js_path_aliases();
        assert!(flat.contains_key("@a/*"));
        assert!(flat.contains_key("@b/*"));
    }

    #[test]
    fn discovered_config_files_empty_by_default() {
        let cfg = empty_config();
        assert!(cfg.discovered_config_files().is_empty());
    }

    #[test]
    fn discovered_config_files_lists_each_manifest_type() {
        let mut cfg = empty_config();
        cfg.ts_configs.insert(
            std::path::PathBuf::from(""),
            TsConfig {
                config_dir: std::path::PathBuf::from(""),
                base_url: None,
                paths: BTreeMap::new(),
            },
        );
        cfg.go_config = Some(GoModConfig {
            module_path: "x".into(),
        });
        cfg.py_config = Some(PyProjectConfig {
            package_name: Some("x".into()),
            src_layout: false,
        });
        let files = cfg.discovered_config_files();
        assert!(files.iter().any(|p| p.ends_with("tsconfig.json")));
        assert!(files.iter().any(|p| p.ends_with("go.mod")));
        assert!(files.iter().any(|p| p.ends_with("pyproject.toml")));
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
