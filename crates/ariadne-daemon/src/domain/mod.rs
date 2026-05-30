//! Daemon domain interior.
//!
//! Pure lifecycle policy (`lifecycle`) plus the warm-graph read model: the
//! in-RAM snapshot mirror (`snapshot`), the derived `catalog`, and the
//! query `dispatch` that maps each [`ariadne_core::DaemonQuery`] to the
//! matching `ariadne-graph` use case. The `interprocess` transport and the
//! storage IO that builds the catalog live in `crate::adapters`
//! [src: docs/folder-layout.md rule 1;
//!  .claude/plans/post-v1-roadmap/tier-07-daemon-warm-graph.md].

pub mod lifecycle;

pub(crate) mod catalog;
pub(crate) mod dispatch;
pub(crate) mod dump;
pub(crate) mod facts;
pub(crate) mod live;
pub(crate) mod queries;
pub(crate) mod snapshot;
