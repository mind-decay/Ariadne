use schemars::JsonSchema;
use serde::Deserialize;

/// Parameters for `ariadne_boundaries` — list all detected semantic boundaries.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct BoundariesParam {
    /// Filter by boundary kind: "http_route" or "event_channel"
    pub kind: Option<String>,
}

/// Parameters for `ariadne_route_map` — HTTP route map.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct RouteMapParam {}

/// Parameters for `ariadne_event_map` — event channel map.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct EventMapParam {}

/// Parameters for `ariadne_boundary_for` — boundaries in a specific file.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct BoundaryForParam {
    /// File path relative to project root
    pub path: String,
}
