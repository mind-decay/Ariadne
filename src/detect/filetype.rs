use crate::model::{CanonicalPath, FileType};

/// Detect file type using 6-level priority matching.
pub fn detect_file_type(path: &CanonicalPath) -> FileType {
    let filename = path.file_name();
    let path_str = path.as_str();

    // Priority 1 — Known config filenames (exact or pattern match)
    if is_config_filename(filename) {
        return FileType::Config;
    }

    // Priority 2 — Per-language test patterns
    if is_test_file(filename, path_str) {
        return FileType::Test;
    }

    // Priority 3 — Type definition extensions
    if filename.ends_with(".d.ts") || filename.ends_with(".d.mts") || filename.ends_with(".pyi") {
        return FileType::TypeDef;
    }

    // Priority 4 — Style extensions
    if let Some("css" | "scss" | "sass" | "less") = path.extension() {
        return FileType::Style;
    }

    // Priority 5 — Asset extensions
    if is_asset_file(filename, path.extension()) {
        return FileType::Asset;
    }

    // Priority 6 — Default
    FileType::Source
}

/// Check if a filename matches known config file patterns.
fn is_config_filename(filename: &str) -> bool {
    // Exact matches
    matches!(
        filename,
        "package.json"
            | "package-lock.json"
            | "go.mod"
            | "go.sum"
            | "Cargo.toml"
            | "Cargo.lock"
            | "pom.xml"
            | "build.gradle"
            | "build.gradle.kts"
            | ".eslintrc.json"
            | ".prettierrc"
            | "webpack.config.js"
            | "webpack.config.ts"
            | "vite.config.ts"
            | "vite.config.js"
            | "jest.config.js"
            | "jest.config.ts"
            | "rollup.config.js"
            | "next.config.js"
            | "next.config.mjs"
            | "tailwind.config.js"
            | "tailwind.config.ts"
            | "postcss.config.js"
            | "babel.config.js"
            | ".babelrc"
            | "Makefile"
            | "Dockerfile"
            | "docker-compose.yml"
            | "docker-compose.yaml"
            | ".gitignore"
            | ".dockerignore"
            | ".env"
            | "pyproject.toml"
            | "setup.py"
            | "setup.cfg"
            | "requirements.txt"
            | "Pipfile"
            | "build.rs"
    ) || is_tsconfig(filename)
        || is_env_variant(filename)
}

/// Match `tsconfig.json` and `tsconfig.*.json` variants.
fn is_tsconfig(filename: &str) -> bool {
    if filename == "tsconfig.json" {
        return true;
    }
    filename.starts_with("tsconfig.") && filename.ends_with(".json")
}

/// Match `.env.*` variants (e.g., `.env.local`, `.env.production`).
fn is_env_variant(filename: &str) -> bool {
    filename.starts_with(".env.")
}

/// Check per-language test file patterns.
fn is_test_file(filename: &str, path_str: &str) -> bool {
    // TypeScript/JavaScript test patterns
    if filename.ends_with(".test.ts")
        || filename.ends_with(".spec.ts")
        || filename.ends_with(".test.js")
        || filename.ends_with(".spec.js")
        || filename.ends_with(".test.tsx")
        || filename.ends_with(".spec.tsx")
        || filename.ends_with(".test.jsx")
        || filename.ends_with(".spec.jsx")
    {
        return true;
    }

    // Files in __tests__/ directory
    if path_contains_segment(path_str, "__tests__") {
        return true;
    }

    // Go test files
    if filename.ends_with("_test.go") {
        return true;
    }

    // Python test patterns
    if filename == "conftest.py" {
        return true;
    }
    if filename.ends_with(".py")
        && (filename.starts_with("test_") || filename.ends_with("_test.py"))
    {
        return true;
    }
    if filename.ends_with(".py") && path_contains_segment(path_str, "tests") {
        return true;
    }

    // Rust integration tests: files in tests/ directory with .rs extension
    if filename.ends_with(".rs") && path_contains_segment(path_str, "tests") {
        return true;
    }

    // C# test patterns
    if filename.ends_with("Tests.cs") || filename.ends_with("Test.cs") {
        return true;
    }
    // Files in *.Tests/ directory
    if filename.ends_with(".cs") && path_has_tests_project_dir(path_str) {
        return true;
    }

    // Java test patterns
    if filename.ends_with("Test.java")
        || filename.ends_with("Tests.java")
        || filename.ends_with("IT.java")
    {
        return true;
    }
    // Files in src/test/ directory
    if filename.ends_with(".java") && path_str.contains("src/test/") {
        return true;
    }

    false
}

/// Check if path contains a specific directory segment.
fn path_contains_segment(path: &str, segment: &str) -> bool {
    for part in path.split('/') {
        if part == segment {
            return true;
        }
    }
    false
}

/// Check if path contains a directory ending with `.Tests` (C# convention).
fn path_has_tests_project_dir(path: &str) -> bool {
    for part in path.split('/') {
        if part.ends_with(".Tests") {
            return true;
        }
    }
    false
}

/// Check if a file is an asset based on extension.
fn is_asset_file(_filename: &str, ext: Option<&str>) -> bool {
    match ext {
        Some(ext) => matches!(
            ext,
            "png"
                | "jpg"
                | "jpeg"
                | "gif"
                | "svg"
                | "ico"
                | "woff"
                | "woff2"
                | "ttf"
                | "eot"
                | "json"
                | "yaml"
                | "yml"
        ),
        // Files without extensions that weren't caught earlier are not assets
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ft(path: &str) -> FileType {
        detect_file_type(&CanonicalPath::new(path))
    }

    // Priority 1 — Config files
    #[test]
    fn config_exact_matches() {
        assert_eq!(ft("Cargo.toml"), FileType::Config);
        assert_eq!(ft("package.json"), FileType::Config);
        assert_eq!(ft("src/build.rs"), FileType::Config);
        assert_eq!(ft(".env"), FileType::Config);
        assert_eq!(ft("Makefile"), FileType::Config);
        assert_eq!(ft("Dockerfile"), FileType::Config);
    }

    #[test]
    fn config_tsconfig_variants() {
        assert_eq!(ft("tsconfig.json"), FileType::Config);
        assert_eq!(ft("tsconfig.build.json"), FileType::Config);
        assert_eq!(ft("tsconfig.app.json"), FileType::Config);
    }

    #[test]
    fn config_env_variants() {
        assert_eq!(ft(".env.local"), FileType::Config);
        assert_eq!(ft(".env.production"), FileType::Config);
    }

    // Priority 2 — Test files
    #[test]
    fn test_typescript_js() {
        assert_eq!(ft("src/auth/login.test.ts"), FileType::Test);
        assert_eq!(ft("src/auth/login.spec.js"), FileType::Test);
        assert_eq!(ft("src/__tests__/util.ts"), FileType::Test);
    }

    #[test]
    fn test_go() {
        assert_eq!(ft("pkg/auth/handler_test.go"), FileType::Test);
    }

    #[test]
    fn test_python() {
        assert_eq!(ft("tests/test_auth.py"), FileType::Test);
        assert_eq!(ft("src/auth_test.py"), FileType::Test);
        assert_eq!(ft("tests/conftest.py"), FileType::Test);
        assert_eq!(ft("tests/helpers.py"), FileType::Test);
    }

    #[test]
    fn test_rust_integration() {
        assert_eq!(ft("tests/integration.rs"), FileType::Test);
    }

    #[test]
    fn test_csharp() {
        assert_eq!(ft("src/AuthTests.cs"), FileType::Test);
        assert_eq!(ft("MyApp.Tests/AuthController.cs"), FileType::Test);
    }

    #[test]
    fn test_java() {
        assert_eq!(ft("src/test/java/AuthTest.java"), FileType::Test);
        assert_eq!(ft("src/AuthIT.java"), FileType::Test);
    }

    // Priority 3 — Type definitions
    #[test]
    fn typedef_files() {
        assert_eq!(ft("src/types/global.d.ts"), FileType::TypeDef);
        assert_eq!(ft("src/types/index.d.mts"), FileType::TypeDef);
        assert_eq!(ft("stubs/module.pyi"), FileType::TypeDef);
    }

    // Priority 4 — Style files
    #[test]
    fn style_files() {
        assert_eq!(ft("src/app.css"), FileType::Style);
        assert_eq!(ft("src/theme.scss"), FileType::Style);
        assert_eq!(ft("src/old.less"), FileType::Style);
    }

    // Priority 5 — Asset files
    #[test]
    fn asset_files() {
        assert_eq!(ft("public/logo.png"), FileType::Asset);
        assert_eq!(ft("assets/font.woff2"), FileType::Asset);
        assert_eq!(ft("data/config.yaml"), FileType::Asset);
        // json not caught by config → asset
        assert_eq!(ft("data/schema.json"), FileType::Asset);
    }

    // Priority 6 — Source (default)
    #[test]
    fn source_default() {
        assert_eq!(ft("src/auth/login.ts"), FileType::Source);
        assert_eq!(ft("src/main.rs"), FileType::Source);
        assert_eq!(ft("cmd/server/main.go"), FileType::Source);
    }

    // Priority ordering: config > test
    #[test]
    fn config_beats_test() {
        // jest.config.js is config, not source
        assert_eq!(ft("jest.config.js"), FileType::Config);
        // setup.py is config, not test
        assert_eq!(ft("setup.py"), FileType::Config);
    }

    // Priority ordering: test > typedef
    #[test]
    fn test_beats_typedef() {
        // A .d.ts in __tests__ is a test
        assert_eq!(ft("__tests__/mock.d.ts"), FileType::Test);
    }
}
