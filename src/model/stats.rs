use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Output model for stats.json.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StatsOutput {
    pub version: u32,
    pub centrality: BTreeMap<String, f64>,
    pub sccs: Vec<Vec<String>>,
    pub layers: BTreeMap<String, Vec<String>>,
    pub summary: StatsSummary,
}

/// Summary statistics for the project graph.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StatsSummary {
    pub max_depth: u32,
    pub avg_in_degree: f64,
    pub avg_out_degree: f64,
    pub bottleneck_files: Vec<String>,
    pub orphan_files: Vec<String>,
}
