use schemars::JsonSchema;
use serde::Deserialize;

/// Parameters for `ariadne_context` — smart context assembly for a task.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ContextParam {
    /// File paths relative to project root (the focus files for context assembly)
    pub files: Vec<String>,
    /// Task type hint: "add_field", "refactor", "fix_bug", "add_feature", "understand"
    pub task: Option<String>,
    /// Maximum token budget for the assembled context (default: 8000)
    pub budget_tokens: Option<u32>,
    /// BFS expansion depth from seed files (default: 3)
    pub depth: Option<u32>,
    /// Categories to include: "tests", "interfaces", "configs" (default: all)
    pub include: Option<Vec<String>>,
}

/// Parameters for `ariadne_tests_for` — find test files covering given paths.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct TestsForParam {
    /// File paths relative to project root
    pub paths: Vec<String>,
}

/// Parameters for `ariadne_reading_order` — topologically sorted reading order.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReadingOrderParam {
    /// Seed file paths relative to project root
    pub paths: Vec<String>,
    /// BFS expansion depth (default: 3)
    pub depth: Option<u32>,
}

/// A single change entry for impact analysis.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ChangeEntry {
    /// File path relative to project root
    pub path: String,
    /// Change type: "modify" or "add"
    #[serde(rename = "type", default = "default_change_type")]
    pub change_type: String,
}

fn default_change_type() -> String {
    "modify".to_string()
}

/// Parameters for `ariadne_plan_impact` — impact analysis for planned changes.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct PlanImpactParam {
    /// Planned changes with file paths and change types
    pub changes: Vec<ChangeEntry>,
}
