use std::collections::BTreeSet;
use std::path::Path;

use crate::diagnostic::{DiagnosticCollector, Warning, WarningCode};
use crate::model::workspace::{WorkspaceInfo, WorkspaceKind, WorkspaceMember};
use crate::model::CanonicalPath;

/// Detect workspace configuration in the project root.
///
/// Checks for npm/yarn workspaces (package.json) and pnpm workspaces
/// (pnpm-workspace.yaml). Returns `None` if no workspace is detected
/// or if configuration cannot be parsed.
pub fn detect_workspace(root: &Path, diagnostics: &DiagnosticCollector) -> Option<WorkspaceInfo> {
    // Try package.json workspaces first (npm/yarn)
    let pkg_json_path = root.join("package.json");
    if pkg_json_path.is_file() {
        match std::fs::read_to_string(&pkg_json_path) {
            Ok(contents) => {
                match serde_json::from_str::<serde_json::Value>(&contents) {
                    Ok(json) => {
                        if let Some(patterns) = extract_workspace_patterns(&json) {
                            let kind = detect_npm_or_yarn(root);
                            return resolve_workspace(root, kind, &patterns, diagnostics);
                        }
                    }
                    Err(e) => {
                        diagnostics.warn(Warning {
                            code: WarningCode::W008ConfigParseFailed,
                            path: CanonicalPath::new("package.json"),
                            message: format!("failed to parse package.json: {e}"),
                            detail: None,
                        });
                        // Fall through to pnpm check instead of returning None
                    }
                }
            }
            Err(e) => {
                diagnostics.warn(Warning {
                    code: WarningCode::W008ConfigParseFailed,
                    path: CanonicalPath::new("package.json"),
                    message: format!("failed to read package.json: {e}"),
                    detail: None,
                });
                // Fall through to pnpm check instead of returning None
            }
        }
    }

    // Try pnpm-workspace.yaml
    let pnpm_path = root.join("pnpm-workspace.yaml");
    if pnpm_path.is_file() {
        match std::fs::read_to_string(&pnpm_path) {
            Ok(contents) => {
                let patterns = parse_pnpm_workspace_yaml(&contents);
                if !patterns.is_empty() {
                    return resolve_workspace(root, WorkspaceKind::Pnpm, &patterns, diagnostics);
                }
            }
            Err(e) => {
                diagnostics.warn(Warning {
                    code: WarningCode::W008ConfigParseFailed,
                    path: CanonicalPath::new("pnpm-workspace.yaml"),
                    message: format!("failed to read pnpm-workspace.yaml: {e}"),
                    detail: None,
                });
                return None;
            }
        }
    }

    None
}

/// Extract workspace glob patterns from a parsed package.json.
fn extract_workspace_patterns(json: &serde_json::Value) -> Option<Vec<String>> {
    let workspaces = json.get("workspaces")?;

    // Format 1: "workspaces": ["packages/*", "apps/*"]
    if let Some(arr) = workspaces.as_array() {
        let patterns: Vec<String> = arr
            .iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect();
        if !patterns.is_empty() {
            return Some(patterns);
        }
    }

    // Format 2: "workspaces": { "packages": ["packages/*"] }
    if let Some(obj) = workspaces.as_object() {
        if let Some(pkgs) = obj.get("packages").and_then(|v| v.as_array()) {
            let patterns: Vec<String> = pkgs
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect();
            if !patterns.is_empty() {
                return Some(patterns);
            }
        }
    }

    None
}

/// Determine if this is an npm or yarn workspace based on lock file presence.
fn detect_npm_or_yarn(root: &Path) -> WorkspaceKind {
    if root.join("yarn.lock").is_file() {
        WorkspaceKind::Yarn
    } else {
        WorkspaceKind::Npm
    }
}

/// Parse pnpm-workspace.yaml manually.
///
/// Expected format:
/// ```yaml
/// packages:
///   - 'packages/*'
///   - "apps/*"
///   - libs/*
/// ```
fn parse_pnpm_workspace_yaml(contents: &str) -> Vec<String> {
    let mut patterns = Vec::new();
    let mut in_packages = false;

    for line in contents.lines() {
        let trimmed = line.trim();

        if trimmed == "packages:" {
            in_packages = true;
            continue;
        }

        if in_packages {
            // Stop if we hit another top-level key (no leading whitespace + ends with ':')
            if !trimmed.is_empty() && !line.starts_with(' ') && !line.starts_with('\t') {
                break;
            }

            if let Some(item) = trimmed.strip_prefix("- ") {
                let item = item.trim();
                // Strip surrounding quotes
                let item = item
                    .strip_prefix('\'')
                    .and_then(|s| s.strip_suffix('\''))
                    .or_else(|| item.strip_prefix('"').and_then(|s| s.strip_suffix('"')))
                    .unwrap_or(item);
                if !item.is_empty() {
                    patterns.push(item.to_string());
                }
            }
        }
    }

    patterns
}

/// Resolve workspace member directories from glob patterns, read their
/// package.json files, and build the WorkspaceInfo.
fn resolve_workspace(
    root: &Path,
    kind: WorkspaceKind,
    patterns: &[String],
    diagnostics: &DiagnosticCollector,
) -> Option<WorkspaceInfo> {
    let mut members = Vec::new();
    let mut seen_names = BTreeSet::new();

    for pattern in patterns {
        // Construct absolute glob pattern
        let abs_pattern = root.join(pattern).to_string_lossy().to_string();

        let paths = match glob::glob(&abs_pattern) {
            Ok(paths) => paths,
            Err(e) => {
                diagnostics.warn(Warning {
                    code: WarningCode::W008ConfigParseFailed,
                    path: CanonicalPath::new(pattern.as_str()),
                    message: format!("invalid workspace glob pattern '{pattern}': {e}"),
                    detail: None,
                });
                continue;
            }
        };

        // Collect and sort for deterministic order
        let mut matched: Vec<_> = paths.filter_map(|r| r.ok()).collect();
        matched.sort();

        for member_dir in matched {
            if !member_dir.is_dir() {
                continue;
            }

            let member_pkg = member_dir.join("package.json");
            if !member_pkg.is_file() {
                continue;
            }

            let pkg_contents = match std::fs::read_to_string(&member_pkg) {
                Ok(c) => c,
                Err(e) => {
                    let rel = member_dir
                        .strip_prefix(root)
                        .unwrap_or(&member_dir)
                        .to_string_lossy();
                    diagnostics.warn(Warning {
                        code: WarningCode::W008ConfigParseFailed,
                        path: CanonicalPath::new(format!("{rel}/package.json")),
                        message: format!("failed to read member package.json: {e}"),
                        detail: None,
                    });
                    continue;
                }
            };

            let pkg_json: serde_json::Value = match serde_json::from_str(&pkg_contents) {
                Ok(v) => v,
                Err(e) => {
                    let rel = member_dir
                        .strip_prefix(root)
                        .unwrap_or(&member_dir)
                        .to_string_lossy();
                    diagnostics.warn(Warning {
                        code: WarningCode::W008ConfigParseFailed,
                        path: CanonicalPath::new(format!("{rel}/package.json")),
                        message: format!("failed to parse member package.json: {e}"),
                        detail: None,
                    });
                    continue;
                }
            };

            let name = match pkg_json.get("name").and_then(|v| v.as_str()) {
                Some(n) => n.to_string(),
                None => {
                    // Use directory name as fallback
                    member_dir
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_default()
                }
            };

            // Name collision check: first-found wins, emit W008
            if !seen_names.insert(name.clone()) {
                let rel = member_dir
                    .strip_prefix(root)
                    .unwrap_or(&member_dir)
                    .to_string_lossy();
                diagnostics.warn(Warning {
                    code: WarningCode::W008ConfigParseFailed,
                    path: CanonicalPath::new(rel.to_string()),
                    message: format!(
                        "duplicate workspace member name '{name}', skipping (first-found wins)"
                    ),
                    detail: None,
                });
                continue;
            }

            let entry_point = resolve_entry_point(&member_dir, &pkg_json);

            members.push(WorkspaceMember {
                name,
                path: member_dir,
                entry_point,
            });
        }
    }

    if members.is_empty() {
        return None;
    }

    Some(WorkspaceInfo { kind, members })
}

/// Resolve the entry point for a workspace member (D-027).
///
/// Priority: main field → module field → default probe
/// (src/index.ts, src/index.js, index.ts, index.js)
fn resolve_entry_point(member_dir: &Path, pkg_json: &serde_json::Value) -> std::path::PathBuf {
    // Try "main" field
    if let Some(main) = pkg_json.get("main").and_then(|v| v.as_str()) {
        let path = member_dir.join(main);
        if path.is_file() {
            return path;
        }
    }

    // Try "module" field
    if let Some(module) = pkg_json.get("module").and_then(|v| v.as_str()) {
        let path = member_dir.join(module);
        if path.is_file() {
            return path;
        }
    }

    // Default probe order
    let probes = [
        "src/index.ts",
        "src/index.tsx",
        "src/index.js",
        "src/index.jsx",
        "index.ts",
        "index.tsx",
        "index.js",
        "index.jsx",
    ];

    for probe in &probes {
        let path = member_dir.join(probe);
        if path.is_file() {
            return path;
        }
    }

    // Fallback: return src/index.ts even if it doesn't exist
    member_dir.join("src/index.ts")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    /// Helper: create a directory and write a file with the given contents.
    fn write_file(base: &Path, relative: &str, contents: &str) {
        let path = base.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, contents).unwrap();
    }

    #[test]
    fn npm_workspace_detection() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        // Root package.json with workspaces
        write_file(
            root,
            "package.json",
            r#"{ "name": "my-monorepo", "workspaces": ["packages/*"] }"#,
        );

        // Member package
        write_file(
            root,
            "packages/auth/package.json",
            r#"{ "name": "@myapp/auth" }"#,
        );
        write_file(root, "packages/auth/src/index.ts", "export {}");

        // Another member
        write_file(
            root,
            "packages/utils/package.json",
            r#"{ "name": "@myapp/utils", "main": "dist/index.js" }"#,
        );
        write_file(root, "packages/utils/dist/index.js", "module.exports = {}");

        let diag = DiagnosticCollector::new();
        let info = detect_workspace(root, &diag).expect("should detect workspace");

        assert_eq!(info.kind, WorkspaceKind::Npm);
        assert_eq!(info.members.len(), 2);
        assert_eq!(info.members[0].name, "@myapp/auth");
        assert_eq!(info.members[1].name, "@myapp/utils");

        // auth uses default probe (src/index.ts)
        assert!(info.members[0].entry_point.ends_with("src/index.ts"));
        // utils uses "main" field
        assert!(info.members[1].entry_point.ends_with("dist/index.js"));

        let report = diag.drain();
        assert!(report.warnings.is_empty());
    }

    #[test]
    fn yarn_workspace_detection() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        write_file(
            root,
            "package.json",
            r#"{ "name": "yarn-mono", "workspaces": { "packages": ["packages/*"] } }"#,
        );
        write_file(root, "yarn.lock", "");
        write_file(root, "packages/core/package.json", r#"{ "name": "core" }"#);
        write_file(root, "packages/core/index.ts", "export {}");

        let diag = DiagnosticCollector::new();
        let info = detect_workspace(root, &diag).expect("should detect yarn workspace");

        assert_eq!(info.kind, WorkspaceKind::Yarn);
        assert_eq!(info.members.len(), 1);
        assert_eq!(info.members[0].name, "core");
        assert!(info.members[0].entry_point.ends_with("index.ts"));
    }

    #[test]
    fn pnpm_workspace_detection() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        write_file(
            root,
            "pnpm-workspace.yaml",
            "packages:\n  - 'packages/*'\n  - \"apps/*\"\n",
        );
        write_file(
            root,
            "packages/lib-a/package.json",
            r#"{ "name": "lib-a" }"#,
        );
        write_file(root, "packages/lib-a/src/index.ts", "export {}");
        write_file(root, "apps/web/package.json", r#"{ "name": "web-app" }"#);
        write_file(root, "apps/web/src/index.ts", "export {}");

        let diag = DiagnosticCollector::new();
        let info = detect_workspace(root, &diag).expect("should detect pnpm workspace");

        assert_eq!(info.kind, WorkspaceKind::Pnpm);
        assert_eq!(info.members.len(), 2);

        let names: Vec<&str> = info.members.iter().map(|m| m.name.as_str()).collect();
        assert!(names.contains(&"lib-a"));
        assert!(names.contains(&"web-app"));
    }

    #[test]
    fn entry_point_preference_order() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        write_file(
            root,
            "package.json",
            r#"{ "name": "mono", "workspaces": ["packages/*"] }"#,
        );

        // Member with both "main" and "module" — main wins
        write_file(
            root,
            "packages/both/package.json",
            r#"{ "name": "both", "main": "lib/main.js", "module": "lib/module.js" }"#,
        );
        write_file(root, "packages/both/lib/main.js", "");
        write_file(root, "packages/both/lib/module.js", "");

        // Member with only "module"
        write_file(
            root,
            "packages/esm/package.json",
            r#"{ "name": "esm", "module": "dist/esm.js" }"#,
        );
        write_file(root, "packages/esm/dist/esm.js", "");

        // Member with "main" pointing to non-existent file, falls through to "module"
        write_file(
            root,
            "packages/fallback/package.json",
            r#"{ "name": "fallback", "main": "missing.js", "module": "lib/mod.js" }"#,
        );
        write_file(root, "packages/fallback/lib/mod.js", "");

        let diag = DiagnosticCollector::new();
        let info = detect_workspace(root, &diag).unwrap();

        assert_eq!(info.members.len(), 3);

        let both = info.members.iter().find(|m| m.name == "both").unwrap();
        assert!(both.entry_point.ends_with("lib/main.js"));

        let esm = info.members.iter().find(|m| m.name == "esm").unwrap();
        assert!(esm.entry_point.ends_with("dist/esm.js"));

        let fallback = info.members.iter().find(|m| m.name == "fallback").unwrap();
        assert!(fallback.entry_point.ends_with("lib/mod.js"));
    }

    #[test]
    fn name_collision_emits_w008() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        write_file(
            root,
            "package.json",
            r#"{ "name": "mono", "workspaces": ["packages/*", "libs/*"] }"#,
        );

        // Two members with the same name
        write_file(root, "packages/dup/package.json", r#"{ "name": "shared" }"#);
        write_file(root, "packages/dup/src/index.ts", "");
        write_file(root, "libs/dup/package.json", r#"{ "name": "shared" }"#);
        write_file(root, "libs/dup/src/index.ts", "");

        let diag = DiagnosticCollector::new();
        let info = detect_workspace(root, &diag).unwrap();

        // First-found wins, second is skipped
        assert_eq!(info.members.len(), 1);
        assert_eq!(info.members[0].name, "shared");

        let report = diag.drain();
        assert_eq!(report.warnings.len(), 1);
        assert_eq!(report.warnings[0].code, WarningCode::W008ConfigParseFailed);
        assert!(report.warnings[0].message.contains("duplicate"));
    }

    #[test]
    fn malformed_package_json_returns_none() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        write_file(root, "package.json", "{ this is not valid json }}}");

        let diag = DiagnosticCollector::new();
        let result = detect_workspace(root, &diag);

        assert!(result.is_none());

        let report = diag.drain();
        assert_eq!(report.warnings.len(), 1);
        assert_eq!(report.warnings[0].code, WarningCode::W008ConfigParseFailed);
    }

    #[test]
    fn no_workspace_indicators_returns_none() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        // package.json without workspaces field
        write_file(root, "package.json", r#"{ "name": "simple-project" }"#);

        let diag = DiagnosticCollector::new();
        let result = detect_workspace(root, &diag);

        assert!(result.is_none());
        let report = diag.drain();
        assert!(report.warnings.is_empty());
    }

    #[test]
    fn empty_directory_returns_none() {
        let tmp = TempDir::new().unwrap();

        let diag = DiagnosticCollector::new();
        let result = detect_workspace(tmp.path(), &diag);

        assert!(result.is_none());
        let report = diag.drain();
        assert!(report.warnings.is_empty());
    }

    #[test]
    fn pnpm_yaml_parsing() {
        let yaml = "packages:\n  - 'packages/*'\n  - \"apps/*\"\n  - tools/*\n";
        let patterns = parse_pnpm_workspace_yaml(yaml);
        assert_eq!(patterns, vec!["packages/*", "apps/*", "tools/*"]);
    }

    #[test]
    fn pnpm_yaml_stops_at_next_key() {
        let yaml = "packages:\n  - 'pkg/*'\nother:\n  - 'ignored'\n";
        let patterns = parse_pnpm_workspace_yaml(yaml);
        assert_eq!(patterns, vec!["pkg/*"]);
    }
}
