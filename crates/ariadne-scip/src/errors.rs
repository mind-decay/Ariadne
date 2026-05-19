//! Crate error type. Mirrors the variants the SCIP adapter can surface;
//! everything downstream sees `ScipError` and never the underlying
//! `prost::DecodeError` / `std::io::Error` directly.

use std::path::PathBuf;

use thiserror::Error;

/// Errors raised by the SCIP adapter.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ScipError {
    /// The configured indexer binary is not on PATH. The driver returns this
    /// rather than crashing so `IngestPlan` can fall back to syntactic-only
    /// indexing for that language (plan §scope: "missing indexers degrade …
    /// never crash").
    #[error("indexer `{binary}` not found on PATH ({install_hint})")]
    IndexerMissing {
        /// CLI binary the driver tried to invoke.
        binary: String,
        /// One-line install hint surfaced in `IngestReport`.
        install_hint: String,
    },

    /// The indexer subprocess exited non-zero.
    #[error("indexer `{binary}` exited with status {status}: {stderr}")]
    SubprocessFailed {
        /// CLI binary that ran.
        binary: String,
        /// Process exit status as reported by the OS.
        status: i32,
        /// Captured stderr, truncated to a sensible length by the driver.
        stderr: String,
    },

    /// Could not read or write a path the driver controls (temp dir, output
    /// scip file, etc.).
    #[error("io error at {path}: {source}")]
    Io {
        /// Path that triggered the failure.
        path: PathBuf,
        /// Backing OS error.
        #[source]
        source: std::io::Error,
    },

    /// Decoding the raw SCIP bytes failed.
    #[error("scip decode failed: {0}")]
    Decode(#[from] prost::DecodeError),

    /// Symbol string failed normalization. The grammar lives in the proto
    /// comments [src: <https://github.com/sourcegraph/scip/blob/main/scip.proto>].
    #[error("malformed scip symbol `{symbol}`: {reason}")]
    MalformedSymbol {
        /// Raw symbol text the driver tried to parse.
        symbol: String,
        /// Short human reason.
        reason: &'static str,
    },
}
