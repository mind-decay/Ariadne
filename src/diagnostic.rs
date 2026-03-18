use std::path::PathBuf;
use std::sync::Mutex;

use crate::model::CanonicalPath;

/// Fatal errors that stop the pipeline (exit code 1).
#[derive(Debug, thiserror::Error)]
pub enum FatalError {
    #[error("E001: project root not found: {path}")]
    ProjectNotFound { path: PathBuf },
    #[error("E002: not a directory: {path}")]
    NotADirectory { path: PathBuf },
    #[error("E003: cannot write to output directory: {path}: {reason}")]
    OutputNotWritable { path: PathBuf, reason: String },
    #[error("E004: no parseable files found in {path}")]
    NoParseableFiles { path: PathBuf },
    #[error("E005: cannot read project directory: {path}: {reason}")]
    WalkFailed { path: PathBuf, reason: String },
}

/// Warning codes for recoverable errors.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum WarningCode {
    W001ParseFailed,
    W002ReadFailed,
    W003FileTooLarge,
    W004BinaryFile,
    W006ImportUnresolved,
    W007PartialParse,
    W008ConfigParseFailed,
    W009EncodingError,
}

impl WarningCode {
    pub fn code(&self) -> &'static str {
        match self {
            Self::W001ParseFailed => "W001",
            Self::W002ReadFailed => "W002",
            Self::W003FileTooLarge => "W003",
            Self::W004BinaryFile => "W004",
            Self::W006ImportUnresolved => "W006",
            Self::W007PartialParse => "W007",
            Self::W008ConfigParseFailed => "W008",
            Self::W009EncodingError => "W009",
        }
    }
}

/// A recoverable warning about a specific file.
#[derive(Clone, Debug)]
pub struct Warning {
    pub code: WarningCode,
    pub path: CanonicalPath,
    pub message: String,
    pub detail: Option<String>,
}

/// Aggregate counts for the summary report.
#[derive(Clone, Debug, Default)]
pub struct DiagnosticCounts {
    pub files_skipped: u32,
    pub imports_unresolved: u32,
    pub partial_parses: u32,
}

/// Final diagnostic report after pipeline completion.
pub struct DiagnosticReport {
    pub warnings: Vec<Warning>,
    pub counts: DiagnosticCounts,
}

/// Thread-safe warning collector for use during parallel pipeline stages.
pub struct DiagnosticCollector {
    warnings: Mutex<Vec<Warning>>,
    counts: Mutex<DiagnosticCounts>,
}

impl DiagnosticCollector {
    pub fn new() -> Self {
        Self {
            warnings: Mutex::new(Vec::new()),
            counts: Mutex::new(DiagnosticCounts::default()),
        }
    }

    /// Record a warning.
    pub fn warn(&self, warning: Warning) {
        let mut warnings = self.warnings.lock().unwrap();
        // Update counts based on warning type
        let mut counts = self.counts.lock().unwrap();
        match warning.code {
            WarningCode::W001ParseFailed
            | WarningCode::W002ReadFailed
            | WarningCode::W003FileTooLarge
            | WarningCode::W004BinaryFile
            | WarningCode::W009EncodingError => {
                counts.files_skipped += 1;
            }
            WarningCode::W006ImportUnresolved => {
                counts.imports_unresolved += 1;
            }
            WarningCode::W007PartialParse => {
                counts.partial_parses += 1;
            }
            WarningCode::W008ConfigParseFailed => {}
        }
        warnings.push(warning);
    }

    /// Increment unresolved import count without recording a warning
    /// (used when not in verbose mode).
    pub fn increment_unresolved(&self) {
        let mut counts = self.counts.lock().unwrap();
        counts.imports_unresolved += 1;
    }

    /// Consume the collector and return a sorted diagnostic report.
    pub fn drain(self) -> DiagnosticReport {
        let mut warnings = self.warnings.into_inner().unwrap();
        let counts = self.counts.into_inner().unwrap();
        // Sort by (path, code) for deterministic output (D-006)
        warnings.sort_by(|a, b| a.path.cmp(&b.path).then(a.code.cmp(&b.code)));
        DiagnosticReport { warnings, counts }
    }
}

impl Default for DiagnosticCollector {
    fn default() -> Self {
        Self::new()
    }
}
