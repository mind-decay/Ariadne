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
