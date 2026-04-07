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
