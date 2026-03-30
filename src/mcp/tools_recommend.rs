//! Parameter structs for recommendation MCP tools.

use schemars::JsonSchema;
use serde::Deserialize;

/// Parameters for the `ariadne_suggest_split` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SuggestSplitParam {
    /// File path to analyze for potential splitting (relative to project root)
    pub path: String,
}

/// Parameters for the `ariadne_refactor_opportunities` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct RefactorOpportunitiesParam {
    /// Optional directory path to limit analysis scope (e.g., "src/algo")
    pub scope: Option<String>,
    /// Optional minimum impact filter: "low", "medium", or "high"
    pub min_impact: Option<String>,
}

/// Parameters for the `ariadne_suggest_placement` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SuggestPlacementParam {
    /// Description of the proposed new file's purpose
    pub description: String,
    /// File paths that the new file will depend on (import from)
    pub depends_on: Vec<String>,
    /// File paths that will depend on (import) the new file
    #[serde(default)]
    pub depended_by: Vec<String>,
}
