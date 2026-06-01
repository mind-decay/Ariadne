//! Per-commit new-side changed line-hunk extraction (tier-11b).
//!
//! Walks the most-recent commits and, for each modified blob, diffs the parent
//! blob against the new blob with `gix`'s `blob-diff` (imara-diff) line
//! tokenizer, emitting the *new-side* changed line ranges as
//! [`LineHunk`]s. The adapter stays symbol-agnostic — paths + line ranges
//! only; the symbol join lives in the `ariadne-graph` use-case (ADR-0019)
//! [src: docs.rs/gix/0.84.0/gix/diff/blob/struct.Diff.html `compute`/`hunks`;
//!  docs.rs/gix/0.84.0/gix/object/tree/diff/enum.ChangeDetached.html ;
//!  plan.md RD7 ; tier-11b step 2].

use std::path::Path;

use ariadne_core::LineHunk;
use gix::bstr::ByteSlice;
use gix::diff::blob::sources::byte_lines;
use gix::diff::blob::{Algorithm, Diff, InternedInput};
use gix::object::tree::diff::ChangeDetached;
use gix::revision::walk::Sorting;
use gix::traverse::commit::simple::CommitTimeOrder;

use super::head_oid;
use crate::errors::GitError;

/// Walk at most `depth` most-recent commits (`None` = full history) and collect
/// each commit's new-side changed line hunks. Returns one `Vec<LineHunk>` per
/// commit walked, each sorted by `(path, start_line, end_line)` so output is
/// deterministic across runs. An unborn HEAD yields an empty vector.
///
/// # Errors
/// [`GitError`] on repository open, traversal, object lookup, or tree/blob diff.
pub fn walk_line_hunks(
    repo_root: &Path,
    depth: Option<u32>,
) -> Result<Vec<Vec<LineHunk>>, GitError> {
    let repo = gix::open(repo_root).map_err(|e| GitError::Open(e.to_string()))?;
    let Some(head) = head_oid(&repo)? else {
        return Ok(Vec::new());
    };
    let walk = repo
        .rev_walk(Some(head))
        .sorting(Sorting::ByCommitTime(CommitTimeOrder::NewestFirst))
        .all()
        .map_err(|e| GitError::Walk(e.to_string()))?;
    let limit = depth.map_or(usize::MAX, |d| d as usize);

    let mut out = Vec::new();
    for info in walk.take(limit) {
        let info = info.map_err(|e| GitError::Walk(e.to_string()))?;
        let commit = info.object().map_err(|e| GitError::Walk(e.to_string()))?;
        let tree = commit.tree().map_err(|e| GitError::Walk(e.to_string()))?;
        let parent_tree = match commit.parent_ids().next() {
            Some(pid) => {
                let parent = repo
                    .find_commit(pid.detach())
                    .map_err(|e| GitError::Walk(e.to_string()))?;
                Some(parent.tree().map_err(|e| GitError::Walk(e.to_string()))?)
            }
            None => None,
        };

        let changes = repo
            .diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), None)
            .map_err(|e| GitError::Diff(e.to_string()))?;

        let mut hunks = Vec::new();
        for change in &changes {
            collect_change_hunks(&repo, change, &mut hunks)?;
        }
        hunks.sort_by(|a: &LineHunk, b: &LineHunk| {
            (a.path.as_str(), a.start_line, a.end_line).cmp(&(
                b.path.as_str(),
                b.start_line,
                b.end_line,
            ))
        });
        out.push(hunks);
    }
    Ok(out)
}

/// Append one change's new-side line hunks to `hunks`. Additions diff against an
/// empty old side (the whole file is new); modifications and rewrites diff the
/// previous blob against the new blob; deletions and non-blob entries (trees,
/// submodules) contribute no new-side lines.
fn collect_change_hunks(
    repo: &gix::Repository,
    change: &ChangeDetached,
    hunks: &mut Vec<LineHunk>,
) -> Result<(), GitError> {
    let (entry_mode, location, old_id, new_id) = match change {
        ChangeDetached::Addition {
            entry_mode,
            location,
            id,
            ..
        } => (entry_mode, location, None, *id),
        ChangeDetached::Modification {
            entry_mode,
            location,
            previous_id,
            id,
            ..
        } => (entry_mode, location, Some(*previous_id), *id),
        ChangeDetached::Rewrite {
            entry_mode,
            location,
            source_id,
            id,
            ..
        } => (entry_mode, location, Some(*source_id), *id),
        // A deletion has no new-side line; HEAD holds no symbol there.
        ChangeDetached::Deletion { .. } => return Ok(()),
    };
    if !entry_mode.is_blob_or_symlink() {
        return Ok(());
    }

    let new_bytes = blob_bytes(repo, new_id)?;
    let old_bytes = match old_id {
        Some(id) => blob_bytes(repo, id)?,
        None => Vec::new(),
    };

    let path = location.to_str_lossy().into_owned();
    push_new_side_hunks(&old_bytes, &new_bytes, &path, hunks);
    Ok(())
}

/// Load a blob's raw bytes by object id.
fn blob_bytes(repo: &gix::Repository, id: gix::ObjectId) -> Result<Vec<u8>, GitError> {
    let object = repo
        .find_object(id)
        .map_err(|e| GitError::Diff(e.to_string()))?;
    Ok(object.data.clone())
}

/// Compute the line diff of `old` → `new` and push each non-empty new-side
/// range as a 1-based inclusive [`LineHunk`] on `path`. imara-diff's `after`
/// range is 0-based half-open over the new-side lines; a pure-deletion hunk has
/// an empty `after` range and is skipped (no new-side line to attribute).
fn push_new_side_hunks(old: &[u8], new: &[u8], path: &str, hunks: &mut Vec<LineHunk>) {
    let input = InternedInput::new(byte_lines(old), byte_lines(new));
    let diff = Diff::compute(Algorithm::Histogram, &input);
    for hunk in diff.hunks() {
        if hunk.after.is_empty() {
            continue;
        }
        hunks.push(LineHunk {
            path: path.to_owned(),
            start_line: hunk.after.start + 1,
            end_line: hunk.after.end,
        });
    }
}
