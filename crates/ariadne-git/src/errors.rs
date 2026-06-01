//! Public error type for the Git-history adapter.

use thiserror::Error;

/// Errors raised while walking Git history. `gix`'s own error types are
/// flattened into messages so no `gix` type leaks across the crate boundary
/// (folder-layout rule 4) [src: docs/folder-layout.md].
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum GitError {
    /// The repository could not be opened at the given path.
    #[error("open git repository: {0}")]
    Open(String),

    /// The commit-history walk failed (HEAD resolution, traversal, or object
    /// lookup).
    #[error("walk git history: {0}")]
    Walk(String),

    /// A per-commit tree diff failed.
    #[error("diff git trees: {0}")]
    Diff(String),

    /// A revision spec could not be resolved to an object (unknown ref or oid,
    /// or a missing/unborn HEAD).
    #[error("resolve git revision: {0}")]
    Revspec(String),
}
