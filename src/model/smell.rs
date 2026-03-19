use serde::Serialize;

use super::types::CanonicalPath;

/// A detected architectural anti-pattern.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ArchSmell {
    pub smell_type: SmellType,
    pub files: Vec<CanonicalPath>,
    pub severity: SmellSeverity,
    pub explanation: String,
    pub metrics: SmellMetrics,
}

/// Quantitative evidence for a detected smell.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct SmellMetrics {
    pub primary_value: f64,
    pub threshold: f64,
}

/// Severity classification for architectural smells.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, Hash)]
pub enum SmellSeverity {
    High,
    Medium,
    Low,
}

impl SmellSeverity {
    /// Numeric severity level for comparison (High=2, Medium=1, Low=0).
    pub fn level(&self) -> u8 {
        match self {
            Self::High => 2,
            Self::Medium => 1,
            Self::Low => 0,
        }
    }

    /// Parse from string (case-insensitive). Defaults to Low for unrecognized input.
    pub fn from_str_loose(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "high" => Self::High,
            "medium" => Self::Medium,
            _ => Self::Low,
        }
    }
}

/// Types of architectural anti-patterns.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, Hash)]
pub enum SmellType {
    GodFile,
    CircularDependency,
    LayerViolation,
    HubAndSpoke,
    UnstableFoundation,
    DeadCluster,
    ShotgunSurgery,
}
