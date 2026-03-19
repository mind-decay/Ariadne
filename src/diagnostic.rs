use std::fmt;
use std::path::PathBuf;
use std::sync::Mutex;

use crate::model::CanonicalPath;

/// Fatal errors that stop the pipeline (exit code 1).
#[non_exhaustive]
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
    #[error("E006: graph not found in {path}. Run 'ariadne build' first.")]
    GraphNotFound { path: PathBuf },
    #[error("E007: stats not found in {path}. Run 'ariadne build' first.")]
    StatsNotFound { path: PathBuf },
    #[error("E008: corrupted file {path}: {reason}")]
    GraphCorrupted { path: PathBuf, reason: String },
    #[error("E009: file not found in graph: {path}")]
    FileNotInGraph { path: String },
    #[error("E010: failed to start MCP server: {reason}")]
    McpServerFailed { reason: String },
    #[error("E011: another ariadne server is running (PID {pid}). Stop it first or remove {}", lock_path.display())]
    LockFileHeld { pid: u32, lock_path: PathBuf },
    #[error("E012: MCP protocol error: {reason}")]
    McpProtocolError { reason: String },
    #[error("E013: invalid argument: {reason}")]
    InvalidArgument { reason: String },
}

/// Warning codes for recoverable errors.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum WarningCode {
    W001ParseFailed,
    W002ReadFailed,
    W003FileTooLarge,
    W004BinaryFile,
    W005MaxFilesReached,
    W006ImportUnresolved,
    W007PartialParse,
    W008ConfigParseFailed,
    W009EncodingError,
    W010GraphVersionMismatch,
    W011GraphCorrupted,
    W012AlgorithmFailed,
    W013StaleStats,
    W014FsWatcherFailed,
    W015IncrementalRebuildFailed,
    W016StaleLockRemoved,
    W017SmellDetectionSkipped,
    W018BlastRadiusTimeout,
}

impl WarningCode {
    pub fn code(&self) -> &'static str {
        match self {
            Self::W001ParseFailed => "W001",
            Self::W002ReadFailed => "W002",
            Self::W003FileTooLarge => "W003",
            Self::W004BinaryFile => "W004",
            Self::W005MaxFilesReached => "W005",
            Self::W006ImportUnresolved => "W006",
            Self::W007PartialParse => "W007",
            Self::W008ConfigParseFailed => "W008",
            Self::W009EncodingError => "W009",
            Self::W010GraphVersionMismatch => "W010",
            Self::W011GraphCorrupted => "W011",
            Self::W012AlgorithmFailed => "W012",
            Self::W013StaleStats => "W013",
            Self::W014FsWatcherFailed => "W014",
            Self::W015IncrementalRebuildFailed => "W015",
            Self::W016StaleLockRemoved => "W016",
            Self::W017SmellDetectionSkipped => "W017",
            Self::W018BlastRadiusTimeout => "W018",
        }
    }
}

impl fmt::Display for WarningCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}

/// Warning output format.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WarningFormat {
    Human,
    Json,
}

/// Format warnings from a diagnostic report.
///
/// Filters W006 (ImportUnresolved) unless `verbose` is true.
pub fn format_warnings(report: &DiagnosticReport, format: WarningFormat, verbose: bool) -> String {
    let mut lines: Vec<String> = Vec::new();

    for w in &report.warnings {
        if w.code == WarningCode::W006ImportUnresolved && !verbose {
            continue;
        }

        match format {
            WarningFormat::Human => {
                let detail_part = match &w.detail {
                    Some(d) => format!(": {}", d),
                    None => String::new(),
                };
                lines.push(format!("warn[{}]: {}: {}{}", w.code, w.path, w.message, detail_part));
            }
            WarningFormat::Json => {
                let detail_json = match &w.detail {
                    Some(d) => format!(",\"detail\":\"{}\"", json_escape(d)),
                    None => String::new(),
                };
                lines.push(format!(
                    "{{\"level\":\"warn\",\"code\":\"{}\",\"file\":\"{}\",\"message\":\"{}\"{}}}",
                    w.code,
                    json_escape(w.path.as_str()),
                    json_escape(&w.message),
                    detail_json,
                ));
            }
        }
    }

    lines.join("\n")
}

/// Format the build summary line.
pub fn format_summary(
    report: &DiagnosticReport,
    file_count: usize,
    edge_count: usize,
    cluster_count: usize,
    elapsed: std::time::Duration,
) -> String {
    let mut result = format!(
        "Built graph: {} files, {} edges, {} clusters in {:.1}s",
        file_count,
        edge_count,
        cluster_count,
        elapsed.as_secs_f64(),
    );

    if report.counts.files_skipped > 0 {
        let mut reasons = Vec::new();
        if report.counts.parse_errors > 0 {
            reasons.push(format!("{} parse error", report.counts.parse_errors));
        }
        if report.counts.read_errors > 0 {
            reasons.push(format!("{} read error", report.counts.read_errors));
        }
        if report.counts.too_large > 0 {
            reasons.push(format!("{} too large", report.counts.too_large));
        }
        if report.counts.binary_files > 0 {
            reasons.push(format!("{} binary", report.counts.binary_files));
        }
        if report.counts.encoding_errors > 0 {
            reasons.push(format!("{} encoding error", report.counts.encoding_errors));
        }
        if reasons.is_empty() {
            result.push_str(&format!("\n  {} files skipped", report.counts.files_skipped));
        } else {
            result.push_str(&format!(
                "\n  {} files skipped ({})",
                report.counts.files_skipped,
                reasons.join(", ")
            ));
        }
    }

    if report.counts.imports_unresolved > 0 {
        result.push_str(&format!(
            "\n  {} imports unresolved (external packages)",
            report.counts.imports_unresolved
        ));
    }

    result
}

/// Minimal JSON string escaping for JSONL output.
fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c < '\x20' => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out
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
    pub parse_errors: u32,
    pub read_errors: u32,
    pub too_large: u32,
    pub binary_files: u32,
    pub encoding_errors: u32,
    pub imports_unresolved: u32,
    pub partial_parses: u32,
    pub graph_load_warnings: u32,
    pub algorithm_failures: u32,
    pub stale_stats: u32,
}

/// Final diagnostic report after pipeline completion.
pub struct DiagnosticReport {
    pub warnings: Vec<Warning>,
    pub counts: DiagnosticCounts,
}

/// Thread-safe warning collector for use during parallel pipeline stages.
pub struct DiagnosticCollector {
    inner: Mutex<(Vec<Warning>, DiagnosticCounts)>,
}

impl DiagnosticCollector {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new((Vec::new(), DiagnosticCounts::default())),
        }
    }

    /// Record a warning.
    pub fn warn(&self, warning: Warning) {
        let mut guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        match warning.code {
            WarningCode::W001ParseFailed => {
                guard.1.files_skipped += 1;
                guard.1.parse_errors += 1;
            }
            WarningCode::W002ReadFailed => {
                guard.1.files_skipped += 1;
                guard.1.read_errors += 1;
            }
            WarningCode::W003FileTooLarge => {
                guard.1.files_skipped += 1;
                guard.1.too_large += 1;
            }
            WarningCode::W004BinaryFile => {
                guard.1.files_skipped += 1;
                guard.1.binary_files += 1;
            }
            WarningCode::W005MaxFilesReached => {
                // Not a file skip — a walk-level limit warning
            }
            WarningCode::W009EncodingError => {
                guard.1.files_skipped += 1;
                guard.1.encoding_errors += 1;
            }
            WarningCode::W006ImportUnresolved => {
                guard.1.imports_unresolved += 1;
            }
            WarningCode::W007PartialParse => {
                guard.1.partial_parses += 1;
            }
            WarningCode::W008ConfigParseFailed => {}
            WarningCode::W010GraphVersionMismatch => {
                guard.1.graph_load_warnings += 1;
            }
            WarningCode::W011GraphCorrupted => {
                guard.1.graph_load_warnings += 1;
            }
            WarningCode::W012AlgorithmFailed => {
                guard.1.algorithm_failures += 1;
            }
            WarningCode::W013StaleStats => {
                guard.1.stale_stats += 1;
            }
            WarningCode::W014FsWatcherFailed => {}
            WarningCode::W015IncrementalRebuildFailed => {}
            WarningCode::W016StaleLockRemoved => {}
            WarningCode::W017SmellDetectionSkipped => {}
            WarningCode::W018BlastRadiusTimeout => {}
        }
        guard.0.push(warning);
    }

    /// Increment unresolved import count without recording a warning
    /// (used when not in verbose mode).
    pub fn increment_unresolved(&self) {
        let mut guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        guard.1.imports_unresolved += 1;
    }

    /// Consume the collector and return a sorted diagnostic report.
    pub fn drain(self) -> DiagnosticReport {
        let (mut warnings, counts) = self.inner.into_inner().unwrap_or_else(|e| e.into_inner());
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

#[cfg(test)]
mod tests {
    use super::*;

    fn make_warning(code: WarningCode, path: &str, message: &str, detail: Option<&str>) -> Warning {
        Warning {
            code,
            path: CanonicalPath::new(path.to_string()),
            message: message.to_string(),
            detail: detail.map(|s| s.to_string()),
        }
    }

    fn make_report(warnings: Vec<Warning>, counts: DiagnosticCounts) -> DiagnosticReport {
        DiagnosticReport { warnings, counts }
    }

    #[test]
    fn human_format_with_detail() {
        let report = make_report(
            vec![make_warning(
                WarningCode::W001ParseFailed,
                "src/foo.ts",
                "failed to parse",
                Some("unexpected token at line 42"),
            )],
            DiagnosticCounts::default(),
        );
        let output = format_warnings(&report, WarningFormat::Human, false);
        assert_eq!(
            output,
            "warn[W001]: src/foo.ts: failed to parse: unexpected token at line 42"
        );
    }

    #[test]
    fn human_format_without_detail() {
        let report = make_report(
            vec![make_warning(
                WarningCode::W002ReadFailed,
                "src/bar.ts",
                "cannot read file",
                None,
            )],
            DiagnosticCounts::default(),
        );
        let output = format_warnings(&report, WarningFormat::Human, false);
        assert_eq!(output, "warn[W002]: src/bar.ts: cannot read file");
    }

    #[test]
    fn json_format_valid() {
        let report = make_report(
            vec![make_warning(
                WarningCode::W001ParseFailed,
                "src/foo.ts",
                "parse failed",
                Some("unexpected token at line 42"),
            )],
            DiagnosticCounts::default(),
        );
        let output = format_warnings(&report, WarningFormat::Json, false);
        // Verify it's valid JSON
        let parsed: serde_json::Value = serde_json::from_str(&output).expect("should be valid JSON");
        assert_eq!(parsed["level"], "warn");
        assert_eq!(parsed["code"], "W001");
        assert_eq!(parsed["file"], "src/foo.ts");
        assert_eq!(parsed["message"], "parse failed");
        assert_eq!(parsed["detail"], "unexpected token at line 42");
    }

    #[test]
    fn json_format_without_detail() {
        let report = make_report(
            vec![make_warning(
                WarningCode::W003FileTooLarge,
                "src/big.ts",
                "file too large",
                None,
            )],
            DiagnosticCounts::default(),
        );
        let output = format_warnings(&report, WarningFormat::Json, false);
        let parsed: serde_json::Value = serde_json::from_str(&output).expect("should be valid JSON");
        assert_eq!(parsed["level"], "warn");
        assert_eq!(parsed["code"], "W003");
        assert!(parsed.get("detail").is_none());
    }

    #[test]
    fn summary_with_skipped_and_unresolved() {
        let report = make_report(
            vec![],
            DiagnosticCounts {
                files_skipped: 3,
                parse_errors: 1,
                read_errors: 1,
                too_large: 1,
                imports_unresolved: 42,
                ..DiagnosticCounts::default()
            },
        );
        let output = format_summary(&report, 847, 2341, 12, std::time::Duration::from_secs_f64(1.23));
        assert!(output.starts_with("Built graph: 847 files, 2341 edges, 12 clusters in 1.2s"));
        assert!(output.contains("3 files skipped (1 parse error, 1 read error, 1 too large)"));
        assert!(output.contains("42 imports unresolved (external packages)"));
    }

    #[test]
    fn summary_no_skipped() {
        let report = make_report(vec![], DiagnosticCounts::default());
        let output = format_summary(&report, 10, 5, 2, std::time::Duration::from_secs_f64(0.5));
        assert_eq!(output, "Built graph: 10 files, 5 edges, 2 clusters in 0.5s");
        assert!(!output.contains("skipped"));
        assert!(!output.contains("unresolved"));
    }

    #[test]
    fn w006_filtered_without_verbose() {
        let report = make_report(
            vec![
                make_warning(WarningCode::W001ParseFailed, "a.ts", "parse failed", None),
                make_warning(WarningCode::W006ImportUnresolved, "b.ts", "unresolved import", None),
            ],
            DiagnosticCounts::default(),
        );
        let output = format_warnings(&report, WarningFormat::Human, false);
        assert!(output.contains("W001"));
        assert!(!output.contains("W006"));
    }

    #[test]
    fn w006_shown_with_verbose() {
        let report = make_report(
            vec![
                make_warning(WarningCode::W001ParseFailed, "a.ts", "parse failed", None),
                make_warning(WarningCode::W006ImportUnresolved, "b.ts", "unresolved import", None),
            ],
            DiagnosticCounts::default(),
        );
        let output = format_warnings(&report, WarningFormat::Human, true);
        assert!(output.contains("W001"));
        assert!(output.contains("W006"));
    }

    #[test]
    fn warning_code_display() {
        assert_eq!(format!("{}", WarningCode::W001ParseFailed), "W001");
        assert_eq!(format!("{}", WarningCode::W006ImportUnresolved), "W006");
        assert_eq!(format!("{}", WarningCode::W009EncodingError), "W009");
    }
}
