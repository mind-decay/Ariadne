use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use crate::conventions::types::{
    FrameworkInfo, ScriptCategory, ScriptInfo, TechStack,
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
        return parse_package_json(&content, &language, &pkg_json);
    }

    let cargo_toml = project_root.join("Cargo.toml");
    if cargo_toml.exists() {
        let content = fs::read_to_string(&cargo_toml)?;
        return parse_cargo_toml(&content, &cargo_toml);
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
    })
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

    // Parse scripts
    let scripts = if let Some(scripts_obj) = json.get("scripts").and_then(|v| v.as_object()) {
        scripts_obj
            .iter()
            .map(|(name, cmd)| {
                let command = cmd.as_str().unwrap_or("").to_string();
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
    })
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

    Ok(TechStack {
        manifest_path: Some(manifest_path.to_string_lossy().into_owned()),
        language: "rust".to_string(),
        frameworks,
        scripts: Vec::new(), // Rust uses cargo commands, no scripts in manifest
        test_framework,
        linter: Some("clippy".to_string()), // Rust de facto linter
        bundler: None,
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

        let result = parse_package_json(json, "typescript", Path::new("package.json")).unwrap();

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
