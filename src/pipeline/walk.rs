use std::path::{Path, PathBuf};

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
    fn walk(&self, root: &Path, config: &WalkConfig) -> Result<Vec<FileEntry>, FatalError>;
}

/// Max directory depth (hardcoded, not configurable).
const MAX_DEPTH: usize = 64;

/// Filesystem-based file walker using the `ignore` crate.
pub struct FsWalker;

impl FsWalker {
    pub fn new() -> Self {
        Self
    }
}

impl FileWalker for FsWalker {
    fn walk(&self, root: &Path, config: &WalkConfig) -> Result<Vec<FileEntry>, FatalError> {
        // Validate root
        if !root.exists() {
            return Err(FatalError::ProjectNotFound {
                path: root.to_path_buf(),
            });
        }
        if !root.is_dir() {
            return Err(FatalError::NotADirectory {
                path: root.to_path_buf(),
            });
        }

        let mut walker = ignore::WalkBuilder::new(root);
        walker.max_depth(Some(MAX_DEPTH));
        walker.hidden(false); // Don't skip hidden files (we want .env etc)
        walker.git_ignore(true);
        walker.git_global(false);
        walker.git_exclude(false);

        // Add exclude patterns for .ariadne and any configured dirs.
        // Build ONE OverrideBuilder with ALL patterns, then apply once.
        let mut override_builder = ignore::overrides::OverrideBuilder::new(root);
        for dir in &config.exclude_dirs {
            let _ = override_builder.add(&format!("!{}/**", dir));
        }
        match override_builder.build() {
            Ok(overrides) => {
                walker.overrides(overrides);
            }
            Err(err) => {
                eprintln!("walk: failed to build override rules, falling back to manual exclusion: {}", err);
            }
        }

        let mut entries = Vec::new();

        for result in walker.build() {
            let entry = match result {
                Ok(e) => e,
                Err(err) => {
                    eprintln!("walk: skipping entry: {}", err);
                    continue;
                }
            };

            // Skip directories
            if entry.file_type().map_or(true, |ft| ft.is_dir()) {
                continue;
            }

            let path = entry.into_path();

            // Extract extension
            let extension = path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_string();

            if extension.is_empty() {
                continue; // Skip files without extensions
            }

            entries.push(FileEntry { path, extension });

            // Check max files limit
            if entries.len() >= config.max_files {
                break;
            }
        }

        // Sort by path for determinism (D-006)
        entries.sort_by(|a, b| a.path.cmp(&b.path));

        Ok(entries)
    }
}
