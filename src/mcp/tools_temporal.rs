use schemars::JsonSchema;
use serde::Deserialize;

/// Parameters for `ariadne_churn` — file change frequency from git history.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ChurnParam {
    /// Time period: "30d", "90d", or "1y" (default: "30d")
    pub period: Option<String>,
    /// Number of top files to return (default: 20)
    pub top: Option<u32>,
}

/// Parameters for `ariadne_coupling` — co-change analysis from git history.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct CouplingParam {
    /// Minimum confidence threshold in [0.0, 1.0] (default: 0.3)
    pub min_confidence: Option<f64>,
}

/// Parameters for `ariadne_hotspots` — high-risk change hotspots.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct HotspotsParam {
    /// Number of top hotspots to return (default: 20)
    pub top: Option<u32>,
}

/// Parameters for `ariadne_ownership` — file ownership from git history.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct OwnershipParam {
    /// Optional file path relative to project root
    pub path: Option<String>,
}

/// Parameters for `ariadne_hidden_deps` — co-changes without structural links.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct HiddenDepsParam {}
