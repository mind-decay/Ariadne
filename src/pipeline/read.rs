use std::path::PathBuf;

use crate::model::{CanonicalPath, ContentHash};

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
        entry: &super::walk::FileEntry,
    ) -> Result<FileContent, FileSkipReason>;
}
