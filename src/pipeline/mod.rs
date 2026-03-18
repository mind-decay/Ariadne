pub mod build;
pub mod read;
pub mod resolve;
pub mod walk;

use std::path::PathBuf;

use crate::diagnostic::Warning;
use crate::model::CanonicalPath;
use crate::parser::{RawExport, RawImport};

pub use read::{FileContent, FileReader, FileSkipReason};
pub use walk::{FileEntry, FileWalker, WalkConfig};

/// Output of the parse stage.
#[derive(Clone, Debug)]
pub struct ParsedFile {
    pub path: CanonicalPath,
    pub imports: Vec<RawImport>,
    pub exports: Vec<RawExport>,
}

/// Result of a successful pipeline run.
pub struct BuildOutput {
    pub graph_path: PathBuf,
    pub clusters_path: PathBuf,
    pub file_count: usize,
    pub edge_count: usize,
    pub cluster_count: usize,
    pub warnings: Vec<Warning>,
}
