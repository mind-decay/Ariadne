use std::path::PathBuf;

use crate::diagnostic::FatalError;

/// Output of the walk stage.
#[derive(Clone, Debug)]
pub struct FileEntry {
    pub path: PathBuf,
    pub extension: String,
}

/// Configuration for the file walking stage.
pub struct WalkConfig {
    pub max_files: usize,
    pub max_file_size: u64,
    pub exclude_dirs: Vec<String>,
}

impl Default for WalkConfig {
    fn default() -> Self {
        Self {
            max_files: 50_000,
            max_file_size: 1_048_576, // 1MB
            exclude_dirs: vec![".ariadne".to_string()],
        }
    }
}

/// Directory traversal abstraction.
pub trait FileWalker: Send + Sync {
    fn walk(&self, root: &std::path::Path, config: &WalkConfig) -> Result<Vec<FileEntry>, FatalError>;
}
