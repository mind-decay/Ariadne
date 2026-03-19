use std::path::{Path, PathBuf};

use crate::diagnostic::{FatalError, Warning, WarningCode};
use crate::model::CanonicalPath;

/// Output of the walk stage.
#[derive(Clone, Debug)]
pub struct FileEntry {
    pub path: PathBuf,
    pub extension: String,
}

/// Result of a walk operation — entries found plus any warnings.
pub struct WalkResult {
    pub entries: Vec<FileEntry>,
    pub warnings: Vec<Warning>,
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
    fn walk(&self, root: &Path, config: &WalkConfig) -> Result<WalkResult, FatalError>;
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
    fn walk(&self, root: &Path, config: &WalkConfig) -> Result<WalkResult, FatalError> {
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
        let mut override_builder = ignore::overrides::OverrideBuilder::new(root);
        for dir in &config.exclude_dirs {
            let _ = override_builder.add(&format!("!{}/**", dir));
        }
        let mut manual_exclude = Vec::new();
        match override_builder.build() {
            Ok(overrides) => {
                walker.overrides(overrides);
            }
            Err(_) => {
                // Fallback: manually exclude these dirs during iteration
                manual_exclude = config.exclude_dirs.clone();
            }
        }

        let mut entries = Vec::new();
        let mut warnings = Vec::new();

        for result in walker.build() {
            let entry = match result {
                Ok(e) => e,
                Err(err) => {
                    // Structured warning instead of eprintln! (S1 fix)
                    warnings.push(Warning {
                        code: WarningCode::W002ReadFailed,
                        path: CanonicalPath::new(format!("{}", err)),
                        message: "walk error".to_string(),
                        detail: Some(format!("{}", err)),
                    });
                    continue;
                }
            };

            // Skip directories
            if entry.file_type().map_or(true, |ft| ft.is_dir()) {
                continue;
            }

            let path = entry.into_path();

            // Manual exclusion fallback (S3 fix)
            if !manual_exclude.is_empty() {
                let rel = path.strip_prefix(root).unwrap_or(&path);
                let should_exclude = manual_exclude.iter().any(|dir| {
                    rel.components()
                        .next()
                        .map_or(false, |c| c.as_os_str() == dir.as_str())
                });
                if should_exclude {
                    continue;
                }
            }

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

            // Check max files limit (S2 fix — emit warning)
            if entries.len() >= config.max_files {
                warnings.push(Warning {
                    code: WarningCode::W005MaxFilesReached,
                    path: CanonicalPath::new(format!("(limit: {})", config.max_files)),
                    message: format!(
                        "file limit reached ({}), graph may be partial",
                        config.max_files
                    ),
                    detail: None,
                });
                break;
            }
        }

        // Sort by path for determinism (D-006)
        entries.sort_by(|a, b| a.path.cmp(&b.path));

        Ok(WalkResult { entries, warnings })
    }
}
