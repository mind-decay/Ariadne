use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use crate::conventions::types::{
    FrameworkInfo, ScriptCategory, ScriptInfo, TechStack, TestConfig,
};
use crate::model::{FileType, ProjectGraph};

/// Detect the project's technology stack from manifest files and graph analysis.
///
/// 1. Detect primary language from file extensions in the graph
/// 2. Find and parse manifest (package.json > Cargo.toml > go.mod)
/// 3. Extract frameworks, scripts, test framework, linter, bundler
pub fn tech_stack(
    project_root: &Path,
    graph: &ProjectGraph,
) -> Result<TechStack, std::io::Error> {
    let language = detect_language(graph);

    let pkg_json = project_root.join("package.json");
    if pkg_json.exists() {
        let content = fs::read_to_string(&pkg_json)?;
        let mut ts = parse_package_json(&content, &language, &pkg_json, project_root)?;
        ts.test_config = discover_test_config(project_root, ts.test_framework.as_deref());
        return Ok(ts);
    }

    let cargo_toml = project_root.join("Cargo.toml");
    if cargo_toml.exists() {
        let content = fs::read_to_string(&cargo_toml)?;
        let mut ts = parse_cargo_toml(&content, &cargo_toml)?;
        ts.test_config = discover_test_config(project_root, ts.test_framework.as_deref());
        return Ok(ts);
    }

    let go_mod = project_root.join("go.mod");
    if go_mod.exists() {
        return Ok(TechStack {
            manifest_path: Some(go_mod.to_string_lossy().into_owned()),
            language: "go".to_string(),
            frameworks: Vec::new(),
            scripts: Vec::new(),
            test_framework: Some("go test".to_string()),
            linter: None,
            bundler: None,
            test_config: None,
        });
    }

    Ok(TechStack {
        manifest_path: None,
        language,
        frameworks: Vec::new(),
        scripts: Vec::new(),
        test_framework: None,
        linter: None,
        bundler: None,
        test_config: discover_test_config(project_root, None),
    })
}

/// Phase 8c B2: discover a test framework config file on disk.
///
/// **Stack-agnostic by design.** Instead of matching a closed allow-list of
/// known frameworks (vitest/jest/mocha/…), candidate filenames are derived
/// *by convention* from the `test_framework` string itself:
///
/// - `{stem}.config.{ts,js,mts,cts,mjs,cjs,json,yaml,yml}`
/// - `.{stem}rc` and `.{stem}rc.{js,cjs,json,yml,yaml,toml}`
///
/// where `stem` is the normalized framework name (e.g. `@playwright/test` →
/// `playwright`, `go test` → `go`). This works for any framework that follows
/// the common `<name>.config.<ext>` / `.<name>rc.<ext>` convention, including
/// ones Ariadne has never heard of (karma, ava, uvu, supertest, bun test,
/// deno test, storybook, etc.).
///
/// The Python ecosystem is the single explicit exception because its convention
/// is different: `pytest.ini`, `pyproject.toml` (`[tool.pytest.ini_options]`),
/// `setup.cfg`, `tox.ini`. These are tried when `test_framework` is absent or
/// when the primary convention-based lookup fails.
///
/// Does NOT parse the discovered file's include/exclude globs — those are left
/// empty and populated best-effort in a future phase if telemetry shows LLMs
/// still searching for them.
pub fn discover_test_config(
    project_root: &Path,
    test_framework: Option<&str>,
) -> Option<TestConfig> {
    // 1. Convention-based probe: derive candidate names from the framework string.
    if let Some(raw) = test_framework {
        if let Some(stem) = normalize_framework_stem(raw) {
            if let Some(cfg) = probe_config_stem(project_root, &stem) {
                return Some(cfg);
            }
        }
    }

    // 2. Language-neutral fallback for Python-style config conventions.
    //    These do not follow the `<name>.config.<ext>` pattern above, so they
    //    need an explicit probe — but the list is fixed by the Python
    //    ecosystem, not by a Theseus-managed allow-list of frameworks.
    for name in ["pytest.ini", "pyproject.toml", "setup.cfg", "tox.ini"] {
        if project_root.join(name).exists() {
            // Only accept pyproject.toml / setup.cfg / tox.ini if they
            // actually mention pytest — otherwise they belong to unrelated
            // tooling and returning them would be misleading.
            if name == "pytest.ini" || file_mentions(project_root, name, "pytest") {
                return Some(TestConfig {
                    config_file_path: Some(name.to_string()),
                    ..TestConfig::default()
                });
            }
        }
    }

    None
}

/// Normalize a framework identifier from tech_stack (e.g. `@playwright/test`,
/// `@nestjs/testing`, `go test`, `cargo-test`) into a filesystem-safe stem
/// suitable for convention-based config lookup.
///
/// Rules: strip leading scope `@owner/`, split on whitespace/slash/hyphen, keep
/// the first non-empty token. Returns `None` for empty input.
fn normalize_framework_stem(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    // Strip leading "@scope/" — e.g. "@playwright/test" → "playwright/test"
    let without_scope = trimmed.strip_prefix('@').unwrap_or(trimmed);
    // Take the first token split by any of / , space, hyphen, underscore
    let stem = without_scope
        .split(|c: char| matches!(c, '/' | ' ' | '-' | '_'))
        .find(|s| !s.is_empty())?
        .to_lowercase();
    if stem.is_empty() { None } else { Some(stem) }
}

/// Probe the project root for any `{stem}.config.<ext>` or `.{stem}rc[.<ext>]`
/// file. Returns the first match that exists on disk.
fn probe_config_stem(project_root: &Path, stem: &str) -> Option<TestConfig> {
    // `<stem>.config.<ext>` — ordered by how common the extension is in
    // the JS/TS ecosystem, then other ecosystems.
    for ext in [
        "ts", "mts", "cts", "js", "mjs", "cjs", "json", "yaml", "yml", "toml",
    ] {
        let name = format!("{stem}.config.{ext}");
        if project_root.join(&name).exists() {
            return Some(TestConfig {
                config_file_path: Some(name),
                ..TestConfig::default()
            });
        }
    }
    // Bare `.<stem>rc`
    let bare = format!(".{stem}rc");
    if project_root.join(&bare).exists() {
        return Some(TestConfig {
            config_file_path: Some(bare),
            ..TestConfig::default()
        });
    }
    // `.<stem>rc.<ext>`
    for ext in ["js", "cjs", "mjs", "json", "yml", "yaml", "toml"] {
        let name = format!(".{stem}rc.{ext}");
        if project_root.join(&name).exists() {
            return Some(TestConfig {
                config_file_path: Some(name),
                ..TestConfig::default()
            });
        }
    }
    None
}

/// Cheap text-search guard used by Python-config detection: returns true if
/// `path` (read as UTF-8) contains `needle`. Used to keep pyproject.toml /
/// setup.cfg / tox.ini from being returned as a "test config" when they
/// actually belong to unrelated tooling.
fn file_mentions(project_root: &Path, name: &str, needle: &str) -> bool {
    match fs::read_to_string(project_root.join(name)) {
        Ok(content) => content.contains(needle),
        Err(_) => false,
    }
}

/// Detect primary language from file extension frequency in the graph.
fn detect_language(graph: &ProjectGraph) -> String {
    let mut ext_counts: BTreeMap<&str, usize> = BTreeMap::new();

    for (path, node) in &graph.nodes {
        if node.file_type != FileType::Source {
            continue;
        }
        let ext = match path.extension() {
            Some(e) => e,
            None => continue,
        };
        *ext_counts.entry(ext).or_default() += 1;
    }

    let dominant_ext = ext_counts
        .into_iter()
        .max_by_key(|(_, count)| *count)
        .map(|(ext, _)| ext);

    match dominant_ext {
        Some("ts" | "tsx") => "typescript".to_string(),
        Some("js" | "jsx" | "mjs" | "cjs") => "javascript".to_string(),
        Some("rs") => "rust".to_string(),
        Some("go") => "go".to_string(),
        Some("py" | "pyi") => "python".to_string(),
        Some("cs") => "csharp".to_string(),
        Some("java") => "java".to_string(),
        Some(other) => other.to_string(),
        None => "unknown".to_string(),
    }
}

fn categorize_script(name: &str) -> ScriptCategory {
    let name_lower = name.to_lowercase();
    if name_lower.contains("test") || name_lower.contains("jest")
        || name_lower.contains("vitest") || name_lower.contains("mocha")
    {
        ScriptCategory::Test
    } else if name_lower.contains("lint") || name_lower.contains("eslint")
        || name_lower.contains("check") || name_lower.contains("format")
    {
        ScriptCategory::Lint
    } else if name_lower.contains("build") || name_lower.contains("compile") {
        ScriptCategory::Build
    } else if name_lower.contains("dev") || name_lower.contains("start")
        || name_lower.contains("serve") || name_lower.contains("watch")
    {
        ScriptCategory::Dev
    } else {
        ScriptCategory::Other
    }
}

/// Known JS/TS framework patterns: (package_name, category)
fn js_framework_category(name: &str) -> Option<&'static str> {
    match name {
        "react" | "react-dom" | "next" | "vue" | "nuxt" | "svelte"
        | "@sveltejs/kit" | "angular" | "@angular/core" | "express"
        | "fastify" | "koa" | "hono" | "nest" | "@nestjs/core"
        | "remix" | "@remix-run/react" | "solid-js" | "astro" => Some("framework"),

        "jest" | "vitest" | "mocha" | "playwright" | "cypress"
        | "@playwright/test" | "ava" | "tap" => Some("testing"),

        "eslint" | "biome" | "@biomejs/biome" | "prettier"
        | "oxlint" => Some("linting"),

        "webpack" | "vite" | "esbuild" | "rollup" | "turbopack"
        | "parcel" | "@rspack/core" | "rspack" => Some("bundler"),

        "typescript" => Some("runtime"),
        _ => None,
    }
}

fn parse_package_json(
    content: &str,
    language: &str,
    manifest_path: &Path,
    project_root: &Path,
) -> Result<TechStack, std::io::Error> {
    let json: serde_json::Value = serde_json::from_str(content)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

    let mut frameworks = Vec::new();
    let mut test_framework: Option<String> = None;
    let mut linter: Option<String> = None;
    let mut bundler: Option<String> = None;

    // Scan dependencies + devDependencies
    for deps_key in &["dependencies", "devDependencies"] {
        if let Some(deps) = json.get(deps_key).and_then(|v| v.as_object()) {
            for (name, version_val) in deps {
                let version = version_val.as_str().unwrap_or("*").to_string();
                if let Some(category) = js_framework_category(name) {
                    frameworks.push(FrameworkInfo {
                        name: name.clone(),
                        version,
                        category: category.to_string(),
                    });
                    match category {
                        "testing" if test_framework.is_none() => {
                            test_framework = Some(name.clone());
                        }
                        "linting" if linter.is_none() => {
                            linter = Some(name.clone());
                        }
                        "bundler" if bundler.is_none() => {
                            bundler = Some(name.clone());
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    // Parse scripts — command is the ready-to-run shell command (e.g. "npm run build"),
    // not the raw inner value from package.json. Runner detected from lock files.
    let runner = detect_js_runner(project_root);
    let scripts = if let Some(scripts_obj) = json.get("scripts").and_then(|v| v.as_object()) {
        scripts_obj
            .iter()
            .map(|(name, _cmd)| {
                let command = format!("{runner} run {name}");
                let category = categorize_script(name);
                ScriptInfo {
                    name: name.clone(),
                    command,
                    category,
                }
            })
            .collect()
    } else {
        Vec::new()
    };

    Ok(TechStack {
        manifest_path: Some(manifest_path.to_string_lossy().into_owned()),
        language: language.to_string(),
        frameworks,
        scripts,
        test_framework,
        linter,
        bundler,
        test_config: None,
    })
}

/// Detect JS package runner from lock files in the project directory.
/// Falls back to "npm" if no lock file is found.
fn detect_js_runner(project_root: &Path) -> &'static str {
    if project_root.join("bun.lockb").exists() || project_root.join("bun.lock").exists() {
        "bun"
    } else if project_root.join("pnpm-lock.yaml").exists() {
        "pnpm"
    } else if project_root.join("yarn.lock").exists() {
        "yarn"
    } else {
        "npm"
    }
}

/// Known Rust crate patterns: (crate_name, category)
fn rust_crate_category(name: &str) -> Option<&'static str> {
    match name {
        "actix-web" | "axum" | "rocket" | "warp" | "poem"
        | "tide" | "gotham" => Some("framework"),
        "tokio" | "async-std" => Some("runtime"),
        "serde" | "serde_json" | "serde_yaml" => Some("serialization"),
        "clap" | "structopt" | "argh" => Some("cli"),
        "tracing" | "log" | "env_logger" => Some("logging"),
        "diesel" | "sqlx" | "sea-orm" => Some("database"),
        _ => None,
    }
}

fn parse_cargo_toml(
    content: &str,
    manifest_path: &Path,
) -> Result<TechStack, std::io::Error> {
    let toml_val: toml::Value = toml::from_str(content)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

    let mut frameworks = Vec::new();
    let mut test_framework: Option<String> = None;

    // Scan [dependencies]
    for section in &["dependencies", "dev-dependencies"] {
        if let Some(deps) = toml_val.get(section).and_then(|v| v.as_table()) {
            for (name, val) in deps {
                let version = extract_toml_version(val);
                if let Some(category) = rust_crate_category(name) {
                    frameworks.push(FrameworkInfo {
                        name: name.clone(),
                        version,
                        category: category.to_string(),
                    });
                }
                // Detect test frameworks in dev-dependencies
                if *section == "dev-dependencies" && test_framework.is_none() {
                    if matches!(name.as_str(), "insta" | "proptest" | "criterion"
                        | "rstest" | "test-case" | "quickcheck")
                    {
                        test_framework = Some(name.clone());
                    }
                }
            }
        }
    }

    // Rust always has cargo commands — synthesize scripts from toolchain
    let mut scripts = vec![
        ScriptInfo {
            name: "build".to_string(),
            command: "cargo build".to_string(),
            category: ScriptCategory::Build,
        },
        ScriptInfo {
            name: "test".to_string(),
            command: "cargo test".to_string(),
            category: ScriptCategory::Test,
        },
    ];
    if true {
        // clippy is Rust's de facto linter
        scripts.push(ScriptInfo {
            name: "lint".to_string(),
            command: "cargo clippy -- -D warnings".to_string(),
            category: ScriptCategory::Lint,
        });
    }

    Ok(TechStack {
        manifest_path: Some(manifest_path.to_string_lossy().into_owned()),
        language: "rust".to_string(),
        frameworks,
        scripts,
        test_framework,
        linter: Some("clippy".to_string()),
        bundler: None,
        test_config: None,
    })
}

/// Extract version string from a TOML dependency value.
/// Handles both `"1.0"` and `{ version = "1.0", ... }` forms.
fn extract_toml_version(val: &toml::Value) -> String {
    match val {
        toml::Value::String(s) => s.clone(),
        toml::Value::Table(t) => t
            .get("version")
            .and_then(|v| v.as_str())
            .unwrap_or("*")
            .to_string(),
        _ => "*".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{
        ArchLayer, CanonicalPath, ClusterId, ContentHash, Node,
    };

    fn make_source_node() -> Node {
        Node {
            file_type: FileType::Source,
            layer: ArchLayer::Unknown,
            fsd_layer: None,
            arch_depth: 0,
            lines: 10,
            hash: ContentHash::new("0000000000000000".to_string()),
            exports: vec![],
            cluster: ClusterId::new("default"),
            symbols: vec![],
        }
    }

    fn make_graph_with_extensions(exts: &[&str]) -> ProjectGraph {
        let mut nodes = BTreeMap::new();
        for (i, ext) in exts.iter().enumerate() {
            let path = format!("src/file{i}.{ext}");
            nodes.insert(CanonicalPath::new(&path), make_source_node());
        }
        ProjectGraph { nodes, edges: vec![] }
    }

    #[test]
    fn detect_language_typescript() {
        let graph = make_graph_with_extensions(&["ts", "ts", "ts", "js"]);
        assert_eq!(detect_language(&graph), "typescript");
    }

    #[test]
    fn detect_language_rust() {
        let graph = make_graph_with_extensions(&["rs", "rs", "rs"]);
        assert_eq!(detect_language(&graph), "rust");
    }

    #[test]
    fn detect_language_empty_graph() {
        let graph = ProjectGraph {
            nodes: BTreeMap::new(),
            edges: vec![],
        };
        assert_eq!(detect_language(&graph), "unknown");
    }

    #[test]
    fn script_categorization() {
        assert_eq!(categorize_script("test"), ScriptCategory::Test);
        assert_eq!(categorize_script("test:unit"), ScriptCategory::Test);
        assert_eq!(categorize_script("lint"), ScriptCategory::Lint);
        assert_eq!(categorize_script("lint:fix"), ScriptCategory::Lint);
        assert_eq!(categorize_script("build"), ScriptCategory::Build);
        assert_eq!(categorize_script("build:prod"), ScriptCategory::Build);
        assert_eq!(categorize_script("dev"), ScriptCategory::Dev);
        assert_eq!(categorize_script("start"), ScriptCategory::Dev);
        assert_eq!(categorize_script("deploy"), ScriptCategory::Other);
    }

    #[test]
    fn parse_package_json_full() {
        let json = r#"{
            "name": "my-app",
            "scripts": {
                "dev": "vite",
                "build": "vite build",
                "test": "vitest",
                "lint": "eslint ."
            },
            "dependencies": {
                "react": "^18.2.0",
                "react-dom": "^18.2.0",
                "express": "^4.18.0"
            },
            "devDependencies": {
                "vitest": "^1.0.0",
                "eslint": "^8.0.0",
                "vite": "^5.0.0",
                "typescript": "^5.0.0"
            }
        }"#;

        let tmp = tempfile::tempdir().expect("temp dir");
        let result = parse_package_json(json, "typescript", Path::new("package.json"), tmp.path()).unwrap();

        assert_eq!(result.language, "typescript");

        // Frameworks detected
        let framework_names: Vec<&str> = result.frameworks.iter().map(|f| f.name.as_str()).collect();
        assert!(framework_names.contains(&"react"));
        assert!(framework_names.contains(&"express"));
        assert!(framework_names.contains(&"vite"));

        // Scripts parsed with correct categories
        let test_script = result.scripts.iter().find(|s| s.name == "test").unwrap();
        assert_eq!(test_script.category, ScriptCategory::Test);
        let dev_script = result.scripts.iter().find(|s| s.name == "dev").unwrap();
        assert_eq!(dev_script.category, ScriptCategory::Dev);

        // Test framework / linter / bundler detected
        assert_eq!(result.test_framework.as_deref(), Some("vitest"));
        assert_eq!(result.linter.as_deref(), Some("eslint"));
        assert_eq!(result.bundler.as_deref(), Some("vite"));
    }

    #[test]
    fn parse_cargo_toml_full() {
        let toml_content = r#"
[package]
name = "my-app"
version = "0.1.0"
edition = "2021"

[dependencies]
axum = "0.7"
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tracing = "0.1"

[dev-dependencies]
insta = { version = "1", features = ["yaml"] }
"#;

        let result = parse_cargo_toml(toml_content, Path::new("Cargo.toml")).unwrap();

        assert_eq!(result.language, "rust");

        let framework_names: Vec<&str> = result.frameworks.iter().map(|f| f.name.as_str()).collect();
        assert!(framework_names.contains(&"axum"));
        assert!(framework_names.contains(&"tokio"));
        assert!(framework_names.contains(&"serde"));

        // Version extraction from inline tables
        let tokio = result.frameworks.iter().find(|f| f.name == "tokio").unwrap();
        assert_eq!(tokio.version, "1");

        assert_eq!(result.test_framework.as_deref(), Some("insta"));
        assert_eq!(result.linter.as_deref(), Some("clippy"));
    }

    #[test]
    fn tech_stack_no_manifest() {
        let dir = tempfile::tempdir().unwrap();
        let graph = make_graph_with_extensions(&["ts", "ts"]);

        let result = tech_stack(dir.path(), &graph).unwrap();
        assert!(result.manifest_path.is_none());
        assert_eq!(result.language, "typescript");
        assert!(result.frameworks.is_empty());
    }

    #[test]
    fn tech_stack_with_package_json() {
        let dir = tempfile::tempdir().unwrap();
        let pkg = r#"{"dependencies":{"react":"^18.0.0"},"scripts":{"test":"jest"}}"#;
        fs::write(dir.path().join("package.json"), pkg).unwrap();

        let graph = make_graph_with_extensions(&["tsx", "tsx"]);
        let result = tech_stack(dir.path(), &graph).unwrap();

        assert!(result.manifest_path.is_some());
        assert_eq!(result.language, "typescript");
        assert!(result.frameworks.iter().any(|f| f.name == "react"));
    }

    // --- Phase 8c B2: stack-agnostic TestConfig discovery ---

    #[test]
    fn normalize_framework_stem_strips_npm_scope() {
        assert_eq!(
            normalize_framework_stem("@playwright/test").as_deref(),
            Some("playwright")
        );
        assert_eq!(
            normalize_framework_stem("@nestjs/testing").as_deref(),
            Some("nestjs")
        );
    }

    #[test]
    fn normalize_framework_stem_splits_on_whitespace_and_hyphen() {
        assert_eq!(normalize_framework_stem("go test").as_deref(), Some("go"));
        assert_eq!(
            normalize_framework_stem("cargo-test").as_deref(),
            Some("cargo")
        );
        assert_eq!(
            normalize_framework_stem("bun test").as_deref(),
            Some("bun")
        );
    }

    #[test]
    fn normalize_framework_stem_lowercases_and_rejects_empty() {
        assert_eq!(normalize_framework_stem("Vitest").as_deref(), Some("vitest"));
        assert!(normalize_framework_stem("").is_none());
        assert!(normalize_framework_stem("  ").is_none());
    }

    #[test]
    fn discover_test_config_vitest_picks_ts_first() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("vitest.config.ts"), "export default {}").unwrap();
        let cfg = discover_test_config(dir.path(), Some("vitest")).unwrap();
        assert_eq!(cfg.config_file_path.as_deref(), Some("vitest.config.ts"));
    }

    #[test]
    fn discover_test_config_jest_js_fallback() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("jest.config.js"), "module.exports = {}").unwrap();
        let cfg = discover_test_config(dir.path(), Some("jest")).unwrap();
        assert_eq!(cfg.config_file_path.as_deref(), Some("jest.config.js"));
    }

    #[test]
    fn discover_test_config_none_when_no_files() {
        let dir = tempfile::tempdir().unwrap();
        assert!(discover_test_config(dir.path(), Some("vitest")).is_none());
    }

    #[test]
    fn discover_test_config_works_for_unknown_framework() {
        // karma is not in any hardcoded list — convention derivation must handle it.
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("karma.config.js"), "module.exports = {}").unwrap();
        let cfg = discover_test_config(dir.path(), Some("karma")).unwrap();
        assert_eq!(cfg.config_file_path.as_deref(), Some("karma.config.js"));
    }

    #[test]
    fn discover_test_config_works_for_rc_style_mocha() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join(".mocharc.json"), "{}").unwrap();
        let cfg = discover_test_config(dir.path(), Some("mocha")).unwrap();
        assert_eq!(cfg.config_file_path.as_deref(), Some(".mocharc.json"));
    }

    #[test]
    fn discover_test_config_works_for_scoped_playwright() {
        // npm scope normalization must not break the common playwright case.
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("playwright.config.ts"),
            "export default {}",
        )
        .unwrap();
        let cfg = discover_test_config(dir.path(), Some("@playwright/test")).unwrap();
        assert_eq!(cfg.config_file_path.as_deref(), Some("playwright.config.ts"));
    }

    #[test]
    fn discover_test_config_pytest_pyproject_with_marker() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("pyproject.toml"),
            "[tool.pytest.ini_options]\n",
        )
        .unwrap();
        let cfg = discover_test_config(dir.path(), Some("pytest")).unwrap();
        assert_eq!(cfg.config_file_path.as_deref(), Some("pyproject.toml"));
    }

    #[test]
    fn discover_test_config_pyproject_without_marker_is_skipped() {
        // pyproject.toml exists but never mentions pytest — do not falsely
        // claim it as a test config.
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("pyproject.toml"),
            "[project]\nname = \"x\"\n",
        )
        .unwrap();
        assert!(discover_test_config(dir.path(), None).is_none());
    }

    #[test]
    fn tech_stack_populates_test_config_for_vitest() {
        let dir = tempfile::tempdir().unwrap();
        let pkg = r#"{"devDependencies":{"vitest":"^1.0.0"}}"#;
        fs::write(dir.path().join("package.json"), pkg).unwrap();
        fs::write(dir.path().join("vitest.config.ts"), "export default {}").unwrap();

        let graph = make_graph_with_extensions(&["ts", "ts"]);
        let result = tech_stack(dir.path(), &graph).unwrap();
        assert_eq!(result.test_framework.as_deref(), Some("vitest"));
        let tc = result.test_config.expect("test_config populated");
        assert_eq!(tc.config_file_path.as_deref(), Some("vitest.config.ts"));
    }

    #[test]
    fn tech_stack_with_cargo_toml() {
        let dir = tempfile::tempdir().unwrap();
        let cargo = r#"
[package]
name = "test"
version = "0.1.0"

[dependencies]
axum = "0.7"
"#;
        fs::write(dir.path().join("Cargo.toml"), cargo).unwrap();

        let graph = make_graph_with_extensions(&["rs", "rs"]);
        let result = tech_stack(dir.path(), &graph).unwrap();

        assert!(result.manifest_path.is_some());
        assert_eq!(result.language, "rust");
        assert!(result.frameworks.iter().any(|f| f.name == "axum"));
    }
}
