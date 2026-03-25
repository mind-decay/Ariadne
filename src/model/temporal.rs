use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::types::CanonicalPath;

/// Complete temporal analysis state computed from git history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemporalState {
    pub churn: BTreeMap<CanonicalPath, ChurnMetrics>,
    pub co_changes: Vec<CoChange>,
    pub ownership: BTreeMap<CanonicalPath, OwnershipInfo>,
    pub hotspots: Vec<Hotspot>,
    /// Whether this was computed from a shallow clone
    pub shallow: bool,
    /// Number of commits analyzed
    pub commits_analyzed: u32,
    /// Analysis window start (ISO 8601)
    pub window_start: String,
    /// Analysis window end (ISO 8601)
    pub window_end: String,
}

/// Per-file change frequency metrics across time windows.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChurnMetrics {
    pub commits_30d: u32,
    pub commits_90d: u32,
    pub commits_1y: u32,
    pub lines_changed_30d: u32,
    pub lines_changed_90d: u32,
    pub authors_30d: u32,
    /// ISO 8601 date of last modification
    pub last_changed: Option<String>,
    /// Top authors by commit count (sorted descending, max 5)
    pub top_authors: Vec<(String, u32)>,
}

/// A pair of files that change together above threshold.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoChange {
    pub file_a: CanonicalPath,
    pub file_b: CanonicalPath,
    pub co_change_count: u32,
    /// Jaccard index: co_changes / (changes_a + changes_b - co_changes)
    pub confidence: f64,
    /// true if also connected in import graph
    pub has_structural_link: bool,
}

/// Per-file author/contributor information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OwnershipInfo {
    pub last_author: String,
    /// Top contributors by commit count (sorted descending, max 5)
    pub top_contributors: Vec<(String, u32)>,
    /// Total distinct authors in analysis window
    pub author_count: u32,
}

/// A file with high combined churn x size x blast radius.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hotspot {
    pub path: CanonicalPath,
    /// Normalized composite score in [0.0, 1.0]
    pub score: f64,
    pub churn_rank: u32,
    /// Lines of code rank (complexity proxy, D-095)
    pub loc_rank: u32,
    pub blast_radius_rank: u32,
}
