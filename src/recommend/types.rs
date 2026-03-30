use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Effort {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Impact {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DataQuality {
    Full,
    Structural,
    Minimal,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RefactorType {
    ExtractInterface,
    BreakCycle,
    MergeModules,
    SplitFile,
    ReduceCoupling,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SplitGroup {
    pub name: String,
    pub symbols: BTreeSet<String>,
    pub estimated_lines: u32,
    pub rationale: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SplitAnalysis {
    pub path: String,
    pub should_split: bool,
    pub reason: String,
    pub suggested_splits: Vec<SplitGroup>,
    pub cut_weight: f64,
    pub impact: SplitImpact,
    pub data_quality: DataQuality,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SplitImpact {
    pub blast_radius_before: u32,
    pub blast_radius_after_estimate: u32,
    pub centrality_before: f64,
    pub centrality_reduction_estimate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlacementSuggestion {
    pub suggested_path: String,
    pub cluster: String,
    pub layer: String,
    pub arch_depth: u32,
    pub reasoning: Vec<String>,
    pub alternatives: Vec<PlacementAlternative>,
    pub data_quality: DataQuality,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlacementAlternative {
    pub path: String,
    pub cluster: String,
    pub risk: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefactorOpportunity {
    pub refactor_type: RefactorType,
    pub target: Vec<String>,
    pub symbols: BTreeSet<String>,
    pub benefit: String,
    pub effort: Effort,
    pub impact: Impact,
    pub effort_score: f64,
    pub impact_score: f64,
    pub pareto: bool,
    pub dominated_by: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefactorAnalysis {
    pub scope: String,
    pub opportunities: Vec<RefactorOpportunity>,
    pub pareto_count: usize,
    pub data_quality: DataQuality,
}

#[derive(Debug, Clone)]
pub struct SymbolGraph {
    pub nodes: Vec<String>,
    pub weights: Vec<Vec<f64>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MinCutResult {
    pub cut_weight: f64,
    pub partition_a: BTreeSet<usize>,
    pub partition_b: BTreeSet<usize>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effort_serde_round_trip() {
        for (variant, expected) in [
            (Effort::Low, "\"low\""),
            (Effort::Medium, "\"medium\""),
            (Effort::High, "\"high\""),
        ] {
            let json = serde_json::to_string(&variant).unwrap();
            assert_eq!(json, expected);
            let back: Effort = serde_json::from_str(&json).unwrap();
            assert_eq!(back, variant);
        }
    }

    #[test]
    fn impact_serde_round_trip() {
        for (variant, expected) in [
            (Impact::Low, "\"low\""),
            (Impact::Medium, "\"medium\""),
            (Impact::High, "\"high\""),
        ] {
            let json = serde_json::to_string(&variant).unwrap();
            assert_eq!(json, expected);
            let back: Impact = serde_json::from_str(&json).unwrap();
            assert_eq!(back, variant);
        }
    }

    #[test]
    fn data_quality_serde_round_trip() {
        for (variant, expected) in [
            (DataQuality::Full, "\"full\""),
            (DataQuality::Structural, "\"structural\""),
            (DataQuality::Minimal, "\"minimal\""),
        ] {
            let json = serde_json::to_string(&variant).unwrap();
            assert_eq!(json, expected);
            let back: DataQuality = serde_json::from_str(&json).unwrap();
            assert_eq!(back, variant);
        }
    }

    #[test]
    fn refactor_type_serde_round_trip() {
        for (variant, expected) in [
            (RefactorType::ExtractInterface, "\"extract_interface\""),
            (RefactorType::BreakCycle, "\"break_cycle\""),
            (RefactorType::MergeModules, "\"merge_modules\""),
            (RefactorType::SplitFile, "\"split_file\""),
            (RefactorType::ReduceCoupling, "\"reduce_coupling\""),
        ] {
            let json = serde_json::to_string(&variant).unwrap();
            assert_eq!(json, expected);
            let back: RefactorType = serde_json::from_str(&json).unwrap();
            assert_eq!(back, variant);
        }
    }
}
