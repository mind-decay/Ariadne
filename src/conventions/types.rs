use serde::Serialize;

/// Trend direction for temporal analysis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Trend {
    Growing,
    Stable,
    Declining,
}

/// Category of import pattern.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Ord, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ImportCategory {
    /// Imports of style files (.css, .scss, .sass, .less, styled-components)
    Style,
    /// Imports within the project (resolved internal edges)
    Internal,
    /// Imports from test files
    Test,
}

/// A detected import pattern in the codebase.
#[derive(Debug, Clone, Serialize)]
pub struct ImportPattern {
    pub category: ImportCategory,
    /// Descriptive identifier (e.g., "relative-imports", "style-imports")
    pub identifier: String,
    /// Number of files that use this pattern
    pub file_count: usize,
    /// Percentage of total source files in scope (0.0 to 100.0)
    pub percentage: f64,
    /// Trend from temporal data, None if unavailable
    pub trend: Option<Trend>,
    /// Up to 3 representative source file paths
    pub example_files: Vec<String>,
}

/// Detected naming case for a symbol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NamingCase {
    PascalCase,
    CamelCase,
    SnakeCase,
    ScreamingSnakeCase,
    /// Name doesn't definitively match any single convention
    Ambiguous,
}

/// A naming convention detected per symbol kind.
#[derive(Debug, Clone, Serialize)]
pub struct NamingConvention {
    /// Symbol kind label: "function", "class", "constant", etc.
    pub symbol_kind: String,
    /// The dominant naming case for this kind
    pub dominant_case: NamingCase,
    /// Count of symbols conforming to the dominant case
    pub conforming: usize,
    /// Total symbols of this kind (including ambiguous)
    pub total: usize,
    /// Up to 5 symbols that actively violate the dominant case
    pub exceptions: Vec<String>,
}

/// Category for a script command in a manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ScriptCategory {
    Dev,
    Test,
    Lint,
    Build,
    Other,
}

/// A framework or notable dependency.
#[derive(Debug, Clone, Serialize)]
pub struct FrameworkInfo {
    pub name: String,
    pub version: String,
    /// Category: "framework", "testing", "linting", "bundler", "runtime"
    pub category: String,
}

/// A build/dev script from the manifest.
#[derive(Debug, Clone, Serialize)]
pub struct ScriptInfo {
    pub name: String,
    pub command: String,
    pub category: ScriptCategory,
}

/// Test framework configuration, populated when Ariadne can discover a
/// framework-specific config file (vitest.config.ts, jest.config.js, pytest.ini, etc.).
///
/// Phase 8c B2: exists so downstream consumers (e.g. Theseus test_gen prompts)
/// can name the config file explicitly instead of forcing the LLM to search.
/// Fields beyond `config_file_path` are populated best-effort: `include_glob`,
/// `exclude_glob`, `setup_files`, `environment` are left empty/None when the
/// config file exists but parsing the full shape is not implemented for the
/// framework in question.
#[derive(Debug, Clone, Default, Serialize)]
pub struct TestConfig {
    /// Path to the discovered config file, relative to project root.
    pub config_file_path: Option<String>,
    /// Glob patterns listing test files.
    pub include_glob: Vec<String>,
    /// Glob patterns excluding files from test discovery.
    pub exclude_glob: Vec<String>,
    /// Setup files that run before the test suite.
    pub setup_files: Vec<String>,
    /// Test environment name (e.g. "node", "jsdom").
    pub environment: Option<String>,
}

/// Technology stack derived from manifest files and graph analysis.
#[derive(Debug, Clone, Serialize)]
pub struct TechStack {
    /// Path to the manifest file, if found
    pub manifest_path: Option<String>,
    /// Primary language detected from file extensions
    pub language: String,
    /// Detected frameworks and notable dependencies
    pub frameworks: Vec<FrameworkInfo>,
    /// Build/dev scripts from manifest
    pub scripts: Vec<ScriptInfo>,
    /// Detected test framework name
    pub test_framework: Option<String>,
    /// Detected linter name
    pub linter: Option<String>,
    /// Detected bundler name
    pub bundler: Option<String>,
    /// Test framework configuration — populated when Ariadne discovers a
    /// framework-specific config file (Phase 8c B2).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub test_config: Option<TestConfig>,
}

/// A temporal trend detected across the codebase.
#[derive(Debug, Clone, Serialize)]
pub struct TemporalTrend {
    /// Human-readable description of the trend
    pub pattern: String,
    /// Direction of the trend
    pub trend: Trend,
    /// Evidence string (e.g., "8/12 new files use .tsx, 3/45 old files use .tsx")
    pub evidence: String,
}
