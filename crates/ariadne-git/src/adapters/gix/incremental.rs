//! Incremental commit-history walk (tier-11a): visit only commits newer than a
//! HEAD-oid watermark, falling back to a full walk when the watermark is not an
//! ancestor of HEAD (rebase / force-push).
//!
//! The watermark crosses the port boundary as raw oid bytes, never a `gix`
//! type, so no `gix` type leaks through the public API (folder-layout rule 2)
//! [src: docs.rs/gix/0.84.0/gix/struct.Repository.html `merge_base` ;
//! plan.md RD7 ; .claude/plans/post-v1-roadmap/tier-11a-incremental-history.md].

use std::path::Path;

use super::{HistoryOptions, HistoryReport, accumulate, head_oid};
use crate::errors::GitError;

/// Outcome of an incremental history walk.
#[derive(Debug, Clone, Default)]
pub struct IncrementalWalk {
    /// `true` when the watermark was a valid ancestor of HEAD and `report`
    /// therefore covers only the commits newer than it (the caller merges);
    /// `false` when there was no usable watermark and `report` is a full walk
    /// (the caller replaces).
    pub incremental: bool,
    /// Derived churn + co-change — a delta when `incremental`, else the full
    /// history.
    pub report: HistoryReport,
    /// Current HEAD commit oid as raw bytes, to persist as the new watermark.
    /// `None` for an unborn HEAD (no commits to record).
    pub head_oid: Option<Vec<u8>>,
}

/// Walk history since `watermark`. With a watermark that is a valid ancestor of
/// HEAD, only newer commits are visited (incremental); with no watermark, an
/// unparseable one, or one unreachable from HEAD (rebase / force-push), the
/// full history is walked and `incremental` is `false`.
///
/// # Errors
/// [`GitError`] on repository open, traversal, object lookup, or tree diff.
pub fn walk_since(
    repo_root: &Path,
    opts: &HistoryOptions,
    watermark: Option<&[u8]>,
) -> Result<IncrementalWalk, GitError> {
    let repo = gix::open(repo_root).map_err(|e| GitError::Open(e.to_string()))?;
    let Some(head) = head_oid(&repo)? else {
        return Ok(IncrementalWalk::default());
    };
    let head_bytes = head.as_slice().to_vec();

    // Hide the watermark only when it parses and is an ancestor of HEAD; an
    // unreachable watermark (rewritten history) drops to a full walk so the
    // caller replaces rather than merging onto a now-invalid base.
    let hide = match watermark.and_then(|bytes| gix::ObjectId::try_from(bytes).ok()) {
        Some(wm) if is_ancestor(&repo, wm, head) => Some(wm),
        _ => None,
    };

    let report = accumulate(&repo, head, hide, opts)?;
    Ok(IncrementalWalk {
        incremental: hide.is_some(),
        report,
        head_oid: Some(head_bytes),
    })
}

/// Whether `ancestor` is an ancestor of (or equal to) `descendant`: their merge
/// base is `ancestor` itself. A `merge_base` error (missing object, unrelated
/// histories) is treated as "not an ancestor" so the caller falls back to a
/// full walk [src: docs.rs/gix/0.84.0/gix/struct.Repository.html `merge_base`].
fn is_ancestor(repo: &gix::Repository, ancestor: gix::ObjectId, descendant: gix::ObjectId) -> bool {
    repo.merge_base(ancestor, descendant)
        .is_ok_and(|base| base == ancestor)
}
