use std::path::PathBuf;

use crate::hash::hash_content;
use crate::model::{CanonicalPath, ContentHash};

use super::walk::FileEntry;

/// Output of the read stage.
#[derive(Clone, Debug)]
pub struct FileContent {
    pub path: CanonicalPath,
    pub bytes: Vec<u8>,
    pub hash: ContentHash,
    pub lines: u32,
}

/// Why a file was skipped during reading.
#[derive(Debug)]
pub enum FileSkipReason {
    ReadError { path: PathBuf, reason: String },
    TooLarge { path: PathBuf, size: u64 },
    BinaryFile { path: PathBuf },
    EncodingError { path: PathBuf },
}

/// File reading + filtering abstraction.
pub trait FileReader: Send + Sync {
    fn read(
        &self,
        entry: &FileEntry,
        project_root: &std::path::Path,
        max_file_size: u64,
    ) -> Result<FileContent, FileSkipReason>;
}

/// Filesystem-based file reader.
pub struct FsReader;

impl FsReader {
    pub fn new() -> Self {
        Self
    }
}

impl Default for FsReader {
    fn default() -> Self {
        Self::new()
    }
}

impl FileReader for FsReader {
    fn read(
        &self,
        entry: &FileEntry,
        project_root: &std::path::Path,
        max_file_size: u64,
    ) -> Result<FileContent, FileSkipReason> {
        // Check file size first
        let metadata = std::fs::metadata(&entry.path).map_err(|e| FileSkipReason::ReadError {
            path: entry.path.clone(),
            reason: e.to_string(),
        })?;

        if metadata.len() > max_file_size {
            return Err(FileSkipReason::TooLarge {
                path: entry.path.clone(),
                size: metadata.len(),
            });
        }

        // Read bytes
        let bytes = std::fs::read(&entry.path).map_err(|e| FileSkipReason::ReadError {
            path: entry.path.clone(),
            reason: e.to_string(),
        })?;

        // Check for binary content (null bytes in first 8KB)
        let check_len = bytes.len().min(8192);
        if bytes[..check_len].contains(&0) {
            return Err(FileSkipReason::BinaryFile {
                path: entry.path.clone(),
            });
        }

        // Check UTF-8
        if std::str::from_utf8(&bytes).is_err() {
            return Err(FileSkipReason::EncodingError {
                path: entry.path.clone(),
            });
        }

        // Compute hash
        let hash = hash_content(&bytes);

        // Count lines
        let lines = bytes.iter().filter(|&&b| b == b'\n').count() as u32;

        // Canonicalize path relative to project root
        let relative = entry.path.strip_prefix(project_root).unwrap_or(&entry.path);
        let canonical = CanonicalPath::new(relative.to_string_lossy().to_string());

        Ok(FileContent {
            path: canonical,
            bytes,
            hash,
            lines,
        })
    }
}
