//! Diff-source reader (tier-14): a [`DiffSpec`] → new-side line hunks + the
//! changed-path list.
//!
//! All three kinds reduce to (old, new) blob pairs. `WorkingTree` enumerates the
//! uncommitted changed paths with `Repository::status` (index-vs-worktree +
//! head-vs-index) and diffs each path's `HEAD` blob against its current worktree
//! bytes; `Commit` diffs a commit's tree against its first-parent tree;
//! `RefRange` diffs the two resolved trees. The new-side line ranges reuse the
//! tier-11b `blob-diff` emitter, so the adapter stays symbol-agnostic — paths +
//! line ranges only; the symbol join lives in the `ariadne-graph` `diff_blast`
//! use-case (ADR-0022) [src: docs.rs/gix/0.84.0/gix/struct.Repository.html ;
//!  docs.rs/gix/0.84.0/gix/status/index.html ; plan.md RD7 ; tier-14 D2/D3].

use std::collections::BTreeSet;
use std::path::Path;

use ariadne_core::{DiffSpec, LineHunk};
use gix::bstr::{BString, ByteSlice};

use super::change_path;
use super::line_hunks::{blob_bytes, collect_change_hunks, push_new_side_hunks};
use crate::errors::GitError;

/// Resolve a [`DiffSpec`] over the repository at `repo_root` to its changeset:
/// the new-side changed line hunks and the full changed-path list. Both are
/// sorted (hunks by `(path, start_line, end_line)`, paths lexicographically and
/// deduplicated) so output is deterministic across runs. New / binary / deleted
/// files contribute a path but no new-side hunk — surfaced downstream as
/// unresolved impact.
///
/// No `gix` type crosses the boundary: the revspec strings inside [`DiffSpec`]
/// are resolved here, and only `ariadne-core` [`LineHunk`]s + path strings are
/// returned (folder-layout rule 4).
///
/// # Errors
/// [`GitError`] on repository open, revspec resolution, working-tree status, or
/// tree/blob diff.
pub fn diff(repo_root: &Path, spec: &DiffSpec) -> Result<(Vec<LineHunk>, Vec<String>), GitError> {
    let repo = gix::open(repo_root).map_err(|e| GitError::Open(e.to_string()))?;
    match spec {
        DiffSpec::WorkingTree => working_tree_changeset(&repo),
        DiffSpec::Commit(rev) => {
            let commit = resolve_commit(&repo, rev)?;
            let tree = commit.tree().map_err(|e| GitError::Diff(e.to_string()))?;
            // A commit diffs against its first parent; a root commit (no parent)
            // diffs against an empty tree, so the whole commit is the new side.
            let parent_tree = match commit.parent_ids().next() {
                Some(pid) => Some(
                    repo.find_commit(pid.detach())
                        .map_err(|e| GitError::Revspec(e.to_string()))?
                        .tree()
                        .map_err(|e| GitError::Diff(e.to_string()))?,
                ),
                None => None,
            };
            tree_changeset(&repo, parent_tree.as_ref(), &tree)
        }
        DiffSpec::RefRange { from, to } => {
            let from_tree = resolve_commit(&repo, from)?
                .tree()
                .map_err(|e| GitError::Diff(e.to_string()))?;
            let to_tree = resolve_commit(&repo, to)?
                .tree()
                .map_err(|e| GitError::Diff(e.to_string()))?;
            tree_changeset(&repo, Some(&from_tree), &to_tree)
        }
    }
}

/// Resolve a revspec string to its commit. A spec that names no object, or a
/// missing HEAD, surfaces as [`GitError::Revspec`].
fn resolve_commit<'r>(repo: &'r gix::Repository, rev: &str) -> Result<gix::Commit<'r>, GitError> {
    let id = repo
        .rev_parse_single(rev)
        .map_err(|e| GitError::Revspec(e.to_string()))?;
    repo.find_commit(id)
        .map_err(|e| GitError::Revspec(e.to_string()))
}

/// Diff `old_tree` (or an empty tree when `None`) against `new_tree`, collecting
/// new-side line hunks + changed paths via the shared tree-diff helpers.
fn tree_changeset(
    repo: &gix::Repository,
    old_tree: Option<&gix::Tree<'_>>,
    new_tree: &gix::Tree<'_>,
) -> Result<(Vec<LineHunk>, Vec<String>), GitError> {
    let changes = repo
        .diff_tree_to_tree(old_tree, Some(new_tree), None)
        .map_err(|e| GitError::Diff(e.to_string()))?;
    let mut hunks = Vec::new();
    let mut paths = Vec::new();
    for change in &changes {
        if let Some(path) = change_path(change) {
            paths.push(path);
        }
        collect_change_hunks(repo, change, &mut hunks)?;
    }
    Ok(finish(hunks, paths))
}

/// Uncommitted changeset: index-vs-worktree + head-vs-index changed paths, with
/// each path's new-side hunks computed from its `HEAD` blob against its current
/// worktree bytes. Untracked files are excluded — no indexed symbol covers them,
/// and including them would make the result depend on incidental worktree state.
fn working_tree_changeset(
    repo: &gix::Repository,
) -> Result<(Vec<LineHunk>, Vec<String>), GitError> {
    let head_tree = repo
        .head_tree()
        .map_err(|e| GitError::Revspec(e.to_string()))?;
    let workdir = repo
        .workdir()
        .ok_or_else(|| GitError::Diff("bare repository has no worktree".to_owned()))?
        .to_owned();

    // `status` enumerates the uncommitted changes; `head_tree` enables the
    // head-vs-index leg, `UntrackedFiles::None` drops the dirwalk over untracked
    // files. An empty `None` pattern set inspects every path.
    let iter = repo
        .status(gix::progress::Discard)
        .map_err(|e| GitError::Diff(e.to_string()))?
        .untracked_files(gix::status::UntrackedFiles::None)
        .head_tree(head_tree.id)
        .into_iter(None::<BString>)
        .map_err(|e| GitError::Diff(e.to_string()))?;

    let mut paths: BTreeSet<String> = BTreeSet::new();
    for item in iter {
        let item = item.map_err(|e| GitError::Diff(e.to_string()))?;
        paths.insert(item.location().to_str_lossy().into_owned());
    }

    // New-side hunks per changed path: `HEAD` blob (old) vs current worktree
    // bytes (new). A deleted file reads as empty new bytes → no hunk; an added
    // file reads as empty old bytes → a whole-file hunk.
    let mut hunks = Vec::new();
    for path in &paths {
        let old_bytes = head_blob_bytes(repo, &head_tree, path)?;
        let new_bytes = std::fs::read(workdir.join(path)).unwrap_or_default();
        push_new_side_hunks(&old_bytes, &new_bytes, path, &mut hunks);
    }

    Ok(finish(hunks, paths.into_iter().collect()))
}

/// The `HEAD` blob bytes for `path`, or empty when `path` is absent at `HEAD`
/// (a newly added file).
fn head_blob_bytes(
    repo: &gix::Repository,
    head_tree: &gix::Tree<'_>,
    path: &str,
) -> Result<Vec<u8>, GitError> {
    match head_tree
        .lookup_entry_by_path(Path::new(path))
        .map_err(|e| GitError::Diff(e.to_string()))?
    {
        Some(entry) => blob_bytes(repo, entry.object_id()),
        None => Ok(Vec::new()),
    }
}

/// Sort + dedup the changeset so the adapter's output is byte-stable: hunks by
/// `(path, start_line, end_line)`, paths lexicographically.
fn finish(mut hunks: Vec<LineHunk>, mut paths: Vec<String>) -> (Vec<LineHunk>, Vec<String>) {
    hunks.sort_by(|a: &LineHunk, b: &LineHunk| {
        (a.path.as_str(), a.start_line, a.end_line).cmp(&(
            b.path.as_str(),
            b.start_line,
            b.end_line,
        ))
    });
    paths.sort();
    paths.dedup();
    (hunks, paths)
}
