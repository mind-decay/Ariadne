use serde::Serialize;

use super::types::{ClusterId, ContentHash, Symbol};

/// File type classification.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FileType {
    Source,
    Test,
    Config,
    Style,
    Asset,
    TypeDef,
}

/// Architectural layer classification.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ArchLayer {
    Api,
    Service,
    Data,
    Util,
    Component,
    Hook,
    Config,
    Unknown,
}

/// A file node in the dependency graph.
#[derive(Clone, Debug)]
pub struct Node {
    pub file_type: FileType,
    pub layer: ArchLayer,
    pub arch_depth: u32,
    pub lines: u32,
    pub hash: ContentHash,
    pub exports: Vec<Symbol>,
    pub cluster: ClusterId,
}
