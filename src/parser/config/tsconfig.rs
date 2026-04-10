use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

use crate::diagnostic::{DiagnosticCollector, Warning, WarningCode};
use crate::model::CanonicalPath;

use super::jsonc::strip_jsonc_comments;

/// Parsed tsconfig.json configuration.
#[derive(Clone, Debug)]
pub struct TsConfig {
    /// Directory containing this tsconfig.json (for relative path resolution).
    pub config_dir: PathBuf,
    /// compilerOptions.baseUrl (relative to config_dir).
    pub base_url: Option<String>,
    /// compilerOptions.paths — alias pattern -> list of target patterns.
    /// e.g., "@/*" -> ["src/*"]
    pub paths: BTreeMap<String, Vec<String>>,
}

/// Parse a tsconfig.json content string.
///
/// Uses `strip_jsonc_comments()` to handle JSONC, then extracts
/// `compilerOptions.baseUrl` and `compilerOptions.paths`.
pub fn parse_tsconfig(
    content: &str,
    config_dir: &Path,
    diagnostics: &DiagnosticCollector,
) -> Option<TsConfig> {
    let stripped = strip_jsonc_comments(content);

    let value: serde_json::Value = match serde_json::from_str(&stripped) {
        Ok(v) => v,
        Err(e) => {
            diagnostics.warn(Warning {
                code: WarningCode::W030ConfigParseError,
                path: CanonicalPath::new(config_dir.join("tsconfig.json").to_string_lossy().to_string()),
                message: format!("failed to parse tsconfig.json: {e}"),
                detail: None,
            });
            return None;
        }
    };

    let compiler_options = value.get("compilerOptions");

    let base_url = compiler_options
        .and_then(|co| co.get("baseUrl"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let paths = extract_paths(compiler_options, diagnostics, config_dir);

    Some(TsConfig {
        config_dir: config_dir.to_path_buf(),
        base_url,
        paths,
    })
}

/// Extract the `compilerOptions.paths` mapping.
fn extract_paths(
    compiler_options: Option<&serde_json::Value>,
    diagnostics: &DiagnosticCollector,
    config_dir: &Path,
) -> BTreeMap<String, Vec<String>> {
    let mut result = BTreeMap::new();

    let paths_obj = match compiler_options.and_then(|co| co.get("paths")).and_then(|p| p.as_object())
    {
        Some(obj) => obj,
        None => return result,
    };

    for (pattern, targets) in paths_obj {
        let target_array = match targets.as_array() {
            Some(arr) => arr,
            None => {
                diagnostics.warn(Warning {
                    code: WarningCode::W032InvalidPathPattern,
                    path: CanonicalPath::new(
                        config_dir.join("tsconfig.json").to_string_lossy().to_string(),
                    ),
                    message: format!("paths pattern '{pattern}' has non-array value"),
                    detail: None,
                });
                continue;
            }
        };

        let targets: Vec<String> = target_array
            .iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect();

        if !targets.is_empty() {
            result.insert(pattern.clone(), targets);
        }
    }

    result
}

/// Parse a tsconfig.json file from disk with `extends` resolution.
///
/// Recursively resolves the `extends` chain (max depth 10) and merges
/// parent configs. Child values override parent values.
pub fn parse_tsconfig_with_extends(
    path: &Path,
    diagnostics: &DiagnosticCollector,
) -> Option<TsConfig> {
    let mut visited = HashSet::new();
    parse_tsconfig_recursive(path, diagnostics, &mut visited, 0)
}

/// Maximum depth for extends chain resolution.
const MAX_EXTENDS_DEPTH: u32 = 10;

fn parse_tsconfig_recursive(
    path: &Path,
    diagnostics: &DiagnosticCollector,
    visited: &mut HashSet<PathBuf>,
    depth: u32,
) -> Option<TsConfig> {
    if depth >= MAX_EXTENDS_DEPTH {
        diagnostics.warn(Warning {
            code: WarningCode::W031CircularExtends,
            path: CanonicalPath::new(path.to_string_lossy().to_string()),
            message: format!("tsconfig extends chain exceeds max depth {MAX_EXTENDS_DEPTH}"),
            detail: None,
        });
        return None;
    }

    let canonical = match std::fs::canonicalize(path) {
        Ok(p) => p,
        Err(_) => {
            diagnostics.warn(Warning {
                code: WarningCode::W033ExtendsNotFound,
                path: CanonicalPath::new(path.to_string_lossy().to_string()),
                message: format!("tsconfig not found: {}", path.display()),
                detail: None,
            });
            return None;
        }
    };

    if !visited.insert(canonical.clone()) {
        diagnostics.warn(Warning {
            code: WarningCode::W031CircularExtends,
            path: CanonicalPath::new(path.to_string_lossy().to_string()),
            message: format!("circular tsconfig extends detected: {}", path.display()),
            detail: None,
        });
        return None;
    }

    let content = match std::fs::read_to_string(&canonical) {
        Ok(c) => c,
        Err(_) => {
            diagnostics.warn(Warning {
                code: WarningCode::W033ExtendsNotFound,
                path: CanonicalPath::new(path.to_string_lossy().to_string()),
                message: format!("cannot read tsconfig: {}", path.display()),
                detail: None,
            });
            return None;
        }
    };

    let config_dir = canonical.parent().unwrap_or(Path::new("."));

    // Check for extends before full parse
    let stripped = strip_jsonc_comments(&content);
    let raw_value: serde_json::Value = match serde_json::from_str(&stripped) {
        Ok(v) => v,
        Err(e) => {
            diagnostics.warn(Warning {
                code: WarningCode::W030ConfigParseError,
                path: CanonicalPath::new(path.to_string_lossy().to_string()),
                message: format!("failed to parse tsconfig.json: {e}"),
                detail: None,
            });
            return None;
        }
    };

    let extends_path = raw_value
        .get("extends")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    // Parse current config
    let mut current = parse_tsconfig(&content, config_dir, diagnostics)?;

    // Resolve extends
    if let Some(extends_str) = extends_path {
        let parent_path = resolve_extends_path(config_dir, &extends_str);
        if let Some(parent) = parse_tsconfig_recursive(&parent_path, diagnostics, visited, depth + 1)
        {
            // Merge: parent first, then child overrides
            current = merge_configs(parent, current);
        }
        // If parent fails to parse, continue with current config
    }

    Some(current)
}

/// Resolve the extends path relative to the config directory.
fn resolve_extends_path(config_dir: &Path, extends: &str) -> PathBuf {
    let mut target = config_dir.join(extends);

    // If the target doesn't end with .json, try appending it
    if target.extension().is_none() {
        target.set_extension("json");
    }

    target
}

/// Merge parent and child TsConfig. Child values override parent.
fn merge_configs(parent: TsConfig, child: TsConfig) -> TsConfig {
    let base_url = child.base_url.or(parent.base_url);

    // For paths: start with parent, then child overrides per-key
    let mut paths = parent.paths;
    for (key, value) in child.paths {
        paths.insert(key, value);
    }

    TsConfig {
        config_dir: child.config_dir,
        base_url,
        paths,
    }
}

/// Resolve a path alias using the tsconfig paths configuration.
///
/// For each entry in config.paths (most specific first — longer patterns first):
/// - If pattern contains `*`: match the wildcard, capture the segment
/// - If pattern is exact match: use the target directly
/// - For each target: substitute `*` with captured segment
/// - Resolve relative to config_dir + baseUrl (if set)
///
/// If no pattern matches but baseUrl is set: returns `[config_dir/baseUrl/specifier]`.
/// Returns empty vec if nothing matches.
pub fn resolve_path_alias(specifier: &str, config: &TsConfig) -> Vec<PathBuf> {
    // Sort patterns by length (longest first) for most-specific matching
    let mut patterns: Vec<(&String, &Vec<String>)> = config.paths.iter().collect();
    patterns.sort_by(|a, b| b.0.len().cmp(&a.0.len()));

    let base_dir = match &config.base_url {
        Some(base) => config.config_dir.join(base),
        None => config.config_dir.clone(),
    };

    for (pattern, targets) in &patterns {
        if let Some(captured) = match_wildcard(pattern, specifier) {
            let mut results = Vec::new();
            for target in *targets {
                let resolved = target.replace('*', &captured);
                results.push(base_dir.join(resolved));
            }
            return results;
        }
    }

    // No pattern matched — fall back to baseUrl if set
    if config.base_url.is_some() {
        return vec![base_dir.join(specifier)];
    }

    Vec::new()
}

/// Match a wildcard pattern (single `*`) against a specifier.
/// Returns the captured segment if matched, None otherwise.
fn match_wildcard(pattern: &str, specifier: &str) -> Option<String> {
    if let Some(star_pos) = pattern.find('*') {
        let prefix = &pattern[..star_pos];
        let suffix = &pattern[star_pos + 1..];

        if specifier.starts_with(prefix) && specifier.ends_with(suffix) {
            let captured_end = specifier.len() - suffix.len();
            if star_pos <= captured_end {
                return Some(specifier[star_pos..captured_end].to_string());
            }
        }
    } else {
        // Exact match (no wildcard)
        if pattern == specifier {
            return Some(String::new());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_diagnostics() -> DiagnosticCollector {
        DiagnosticCollector::new()
    }

    // --- parse_tsconfig ---

    #[test]
    fn basic_tsconfig_with_paths() {
        let diag = make_diagnostics();
        let content = r#"{
            "compilerOptions": {
                "baseUrl": ".",
                "paths": {
                    "@/*": ["src/*"],
                    "@utils/*": ["src/utils/*"]
                }
            }
        }"#;
        let config = parse_tsconfig(content, Path::new("/project"), &diag).unwrap();
        assert_eq!(config.base_url.as_deref(), Some("."));
        assert_eq!(config.paths.len(), 2);
        assert_eq!(
            config.paths.get("@/*").unwrap(),
            &vec!["src/*".to_string()]
        );
    }

    #[test]
    fn paths_with_wildcard_substitution() {
        let config = TsConfig {
            config_dir: PathBuf::from("/project"),
            base_url: Some(".".to_string()),
            paths: BTreeMap::from([
                ("@/*".to_string(), vec!["src/*".to_string()]),
            ]),
        };
        let results = resolve_path_alias("@/components/Button", &config);
        assert_eq!(results, vec![PathBuf::from("/project/./src/components/Button")]);
    }

    #[test]
    fn base_url_resolution() {
        let config = TsConfig {
            config_dir: PathBuf::from("/project"),
            base_url: Some("src".to_string()),
            paths: BTreeMap::new(),
        };
        let results = resolve_path_alias("utils/helper", &config);
        assert_eq!(results, vec![PathBuf::from("/project/src/utils/helper")]);
    }

    #[test]
    fn base_url_without_paths_fallback() {
        let config = TsConfig {
            config_dir: PathBuf::from("/project"),
            base_url: Some(".".to_string()),
            paths: BTreeMap::new(),
        };
        let results = resolve_path_alias("components/Button", &config);
        assert_eq!(
            results,
            vec![PathBuf::from("/project/./components/Button")]
        );
    }

    #[test]
    fn no_base_url_no_paths_returns_empty() {
        let config = TsConfig {
            config_dir: PathBuf::from("/project"),
            base_url: None,
            paths: BTreeMap::new(),
        };
        let results = resolve_path_alias("something", &config);
        assert!(results.is_empty());
    }

    #[test]
    fn jsonc_comments_in_tsconfig() {
        let diag = make_diagnostics();
        let content = r#"{
            // TypeScript config
            "compilerOptions": {
                "baseUrl": ".", /* root-relative */
                "paths": {
                    "@/*": ["src/*"],
                }
            }
        }"#;
        let config = parse_tsconfig(content, Path::new("/project"), &diag).unwrap();
        assert_eq!(config.base_url.as_deref(), Some("."));
        assert_eq!(config.paths.len(), 1);
    }

    #[test]
    fn empty_paths_no_aliases() {
        let diag = make_diagnostics();
        let content = r#"{
            "compilerOptions": {
                "baseUrl": ".",
                "paths": {}
            }
        }"#;
        let config = parse_tsconfig(content, Path::new("/project"), &diag).unwrap();
        assert!(config.paths.is_empty());
    }

    #[test]
    fn multiple_targets_for_same_alias() {
        let config = TsConfig {
            config_dir: PathBuf::from("/project"),
            base_url: Some(".".to_string()),
            paths: BTreeMap::from([(
                "@lib/*".to_string(),
                vec!["lib/*".to_string(), "vendor/*".to_string()],
            )]),
        };
        let results = resolve_path_alias("@lib/foo", &config);
        assert_eq!(
            results,
            vec![
                PathBuf::from("/project/./lib/foo"),
                PathBuf::from("/project/./vendor/foo"),
            ]
        );
    }

    #[test]
    fn missing_tsconfig_returns_none() {
        let diag = make_diagnostics();
        let result =
            parse_tsconfig_with_extends(Path::new("/nonexistent/tsconfig.json"), &diag);
        assert!(result.is_none());
        let report = diag.drain();
        assert!(report.warnings.iter().any(|w| w.code == WarningCode::W033ExtendsNotFound));
    }

    #[test]
    fn parse_error_returns_none() {
        let diag = make_diagnostics();
        let content = "this is not json at all";
        let result = parse_tsconfig(content, Path::new("/project"), &diag);
        assert!(result.is_none());
        let report = diag.drain();
        assert!(report.warnings.iter().any(|w| w.code == WarningCode::W030ConfigParseError));
    }

    #[test]
    fn exact_match_pattern() {
        let config = TsConfig {
            config_dir: PathBuf::from("/project"),
            base_url: Some(".".to_string()),
            paths: BTreeMap::from([(
                "jquery".to_string(),
                vec!["vendor/jquery/dist/jquery.js".to_string()],
            )]),
        };
        let results = resolve_path_alias("jquery", &config);
        assert_eq!(
            results,
            vec![PathBuf::from(
                "/project/./vendor/jquery/dist/jquery.js"
            )]
        );
    }

    #[test]
    fn longer_pattern_takes_priority() {
        let config = TsConfig {
            config_dir: PathBuf::from("/project"),
            base_url: Some(".".to_string()),
            paths: BTreeMap::from([
                ("@/*".to_string(), vec!["src/*".to_string()]),
                ("@/utils/*".to_string(), vec!["src/shared/utils/*".to_string()]),
            ]),
        };
        // "@/utils/foo" should match "@/utils/*" (longer) not "@/*"
        let results = resolve_path_alias("@/utils/foo", &config);
        assert_eq!(
            results,
            vec![PathBuf::from("/project/./src/shared/utils/foo")]
        );
    }

    #[test]
    fn extends_chain_with_tempdir() {
        let dir = tempfile::tempdir().unwrap();

        // Parent config
        let parent_content = r#"{
            "compilerOptions": {
                "baseUrl": ".",
                "paths": {
                    "@parent/*": ["parent-src/*"]
                }
            }
        }"#;
        std::fs::write(dir.path().join("tsconfig.base.json"), parent_content).unwrap();

        // Child config extends parent
        let child_content = r#"{
            "extends": "./tsconfig.base.json",
            "compilerOptions": {
                "paths": {
                    "@child/*": ["child-src/*"]
                }
            }
        }"#;
        std::fs::write(dir.path().join("tsconfig.json"), child_content).unwrap();

        let diag = make_diagnostics();
        let config =
            parse_tsconfig_with_extends(&dir.path().join("tsconfig.json"), &diag).unwrap();

        // Should have both parent and child paths
        assert!(config.paths.contains_key("@parent/*"));
        assert!(config.paths.contains_key("@child/*"));
        // baseUrl from parent inherited
        assert_eq!(config.base_url.as_deref(), Some("."));
    }

    #[test]
    fn circular_extends_detection() {
        let dir = tempfile::tempdir().unwrap();

        // A extends B, B extends A
        let a_content = r#"{
            "extends": "./b.json",
            "compilerOptions": {
                "paths": { "@a/*": ["a/*"] }
            }
        }"#;
        let b_content = r#"{
            "extends": "./a.json",
            "compilerOptions": {
                "paths": { "@b/*": ["b/*"] }
            }
        }"#;
        std::fs::write(dir.path().join("a.json"), a_content).unwrap();
        std::fs::write(dir.path().join("b.json"), b_content).unwrap();

        let diag = make_diagnostics();
        let config = parse_tsconfig_with_extends(&dir.path().join("a.json"), &diag);

        // Should still return a config (parsed so far), not panic
        assert!(config.is_some());
        let report = diag.drain();
        assert!(report
            .warnings
            .iter()
            .any(|w| w.code == WarningCode::W031CircularExtends));
    }

    #[test]
    fn no_compiler_options() {
        let diag = make_diagnostics();
        let content = r#"{ "include": ["src"] }"#;
        let config = parse_tsconfig(content, Path::new("/project"), &diag).unwrap();
        assert!(config.base_url.is_none());
        assert!(config.paths.is_empty());
    }

    // --- match_wildcard ---

    #[test]
    fn wildcard_match_captures_segment() {
        assert_eq!(
            match_wildcard("@/*", "@/foo/bar"),
            Some("foo/bar".to_string())
        );
    }

    #[test]
    fn wildcard_no_match() {
        assert_eq!(match_wildcard("@/*", "~/foo"), None);
    }

    #[test]
    fn exact_match_returns_empty_capture() {
        assert_eq!(match_wildcard("jquery", "jquery"), Some(String::new()));
    }

    #[test]
    fn exact_match_no_match() {
        assert_eq!(match_wildcard("jquery", "lodash"), None);
    }
}
