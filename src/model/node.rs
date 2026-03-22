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

impl FileType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Source => "source",
            Self::Test => "test",
            Self::Config => "config",
            Self::Style => "style",
            Self::Asset => "asset",
            Self::TypeDef => "type_def",
        }
    }
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

impl ArchLayer {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Api => "api",
            Self::Service => "service",
            Self::Data => "data",
            Self::Util => "util",
            Self::Component => "component",
            Self::Hook => "hook",
            Self::Config => "config",
            Self::Unknown => "unknown",
        }
    }
}

/// Feature-Sliced Design layer classification (D-031).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FsdLayer {
    App,
    Processes,
    Pages,
    Widgets,
    Features,
    Entities,
    Shared,
}

impl FsdLayer {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::App => "app",
            Self::Processes => "processes",
            Self::Pages => "pages",
            Self::Widgets => "widgets",
            Self::Features => "features",
            Self::Entities => "entities",
            Self::Shared => "shared",
        }
    }
}

/// A file node in the dependency graph.
#[derive(Clone, Debug)]
pub struct Node {
    pub file_type: FileType,
    pub layer: ArchLayer,
    pub fsd_layer: Option<FsdLayer>,
    pub arch_depth: u32,
    pub lines: u32,
    pub hash: ContentHash,
    pub exports: Vec<Symbol>,
    pub cluster: ClusterId,
}
