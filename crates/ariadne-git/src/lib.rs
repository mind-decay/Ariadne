//! Ariadne Git-history adapter.
//!
//! Driven adapter that walks bounded commit history with `gix` (pure-Rust)
//! and derives file-level churn + unordered co-change, returning owned
//! `ariadne-core` records. Depends only on `ariadne-core` (folder-layout rule
//! 2); no `gix` type crosses the public API [src: post-v1-roadmap plan.md RD7;
//! docs/adr/0018-git-history-adapter.md].
//!
//! Façade only — re-exports the adapter surface + error type. No logic lives
//! here [src: docs/folder-layout.md rule 3].

#![deny(missing_docs)]

pub mod adapters;
pub mod errors;

pub use adapters::gix::{
    HistoryOptions, HistoryReport, IncrementalWalk, walk_history, walk_line_hunks, walk_since,
};
pub use errors::GitError;
