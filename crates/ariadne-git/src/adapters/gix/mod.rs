//! `gix`-backed commit-history walk: per-file churn + unordered co-change.
//!
//! `head_commit` → `rev_walk([head]).all()` ancestors (uses the commit-graph
//! file when present, R-C1); per commit, `diff_tree_to_tree(parent, tree)`
//! yields the changed blob paths (root commits diff against an empty tree via
//! a `None` parent); `commit.author()` + `info.commit_time` give identity and
//! last-changed time [src: docs.rs/gix/0.84.0/gix/struct.Repository.html ;
//! plan.md RD7 ; docs/adr/0018-git-history-adapter.md].

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use ariadne_core::{CoChangePair, FileChurn};
use gix::bstr::ByteSlice;
use gix::object::tree::diff::ChangeDetached;
use gix::revision::walk::Sorting;
use gix::traverse::commit::simple::CommitTimeOrder;

use crate::errors::GitError;

mod incremental;

pub use incremental::{IncrementalWalk, walk_since};

/// Bounds on the history walk, read from `config.toml` `[history]`.
#[derive(Debug, Clone)]
pub struct HistoryOptions {
    /// Walk at most this many most-recent commits; `None` walks full history.
    pub depth: Option<u32>,
    /// Commits touching more than this many files are excluded from co-change
    /// (their O(n²) pair set is coupling noise) but still counted for churn
    /// [src: Tornhill, "Your Code as a Crime Scene", 2015].
    pub max_files_per_commit: u32,
}

/// Derived file-level history: per-file churn + unordered co-change pairs.
/// Both vectors are sorted (churn by path, pairs by `(a, b)`) so output is
/// deterministic across runs.
#[derive(Debug, Clone, Default)]
pub struct HistoryReport {
    /// Per-file churn records.
    pub churn: Vec<FileChurn>,
    /// Unordered file-pair co-change counts.
    pub pairs: Vec<CoChangePair>,
}

/// Per-file accumulator collapsed into a [`FileChurn`] once the walk ends.
struct ChurnAccum {
    commits: u32,
    authors: BTreeSet<[u8; 8]>,
    last_changed_ns: i128,
}

const NANOS_PER_SEC: i128 = 1_000_000_000;

/// Walk bounded commit history rooted at HEAD and derive file churn +
/// co-change. An unborn HEAD (no commits) yields an empty report.
///
/// # Errors
/// [`GitError`] on repository open, traversal, object lookup, or tree diff.
pub fn walk_history(repo_root: &Path, opts: &HistoryOptions) -> Result<HistoryReport, GitError> {
    let repo = gix::open(repo_root).map_err(|e| GitError::Open(e.to_string()))?;
    let Some(head) = head_oid(&repo)? else {
        // Unborn HEAD: a repository with no commits has no history to ingest.
        return Ok(HistoryReport::default());
    };
    accumulate(&repo, head, None, opts)
}

/// Resolve HEAD to its commit oid, or `None` for an unborn HEAD (no commits).
/// Shared by [`walk_history`] and the incremental [`walk_since`].
pub(super) fn head_oid(repo: &gix::Repository) -> Result<Option<gix::ObjectId>, GitError> {
    let head = repo.head().map_err(|e| GitError::Walk(e.to_string()))?;
    Ok(head.id().map(gix::Id::detach))
}

/// Accumulate file churn + co-change over the ancestors of `head`. When `hide`
/// is `Some(oid)`, that commit and all its ancestors are excluded from the walk
/// (`with_hidden`), so only commits newer than the watermark are visited — the
/// tier-11a incremental walk; `None` walks the full reachable history. The
/// commit-graph file is used when present (R-C1)
/// [src: docs.rs/gix/0.84.0/gix/revision/walk/struct.Platform.html `with_hidden`].
///
/// # Errors
/// [`GitError`] on traversal, object lookup, or tree diff.
pub(super) fn accumulate(
    repo: &gix::Repository,
    head: gix::ObjectId,
    hide: Option<gix::ObjectId>,
    opts: &HistoryOptions,
) -> Result<HistoryReport, GitError> {
    let walk = repo
        .rev_walk(Some(head))
        .sorting(Sorting::ByCommitTime(CommitTimeOrder::NewestFirst))
        .with_hidden(hide)
        .all()
        .map_err(|e| GitError::Walk(e.to_string()))?;

    let mut churn: BTreeMap<String, ChurnAccum> = BTreeMap::new();
    let mut pairs: BTreeMap<(String, String), u32> = BTreeMap::new();
    // Bounded depth: walk at most `depth` of the most-recent commits.
    let limit = opts.depth.map_or(usize::MAX, |d| d as usize);

    for info in walk.take(limit) {
        let info = info.map_err(|e| GitError::Walk(e.to_string()))?;

        let commit = info.object().map_err(|e| GitError::Walk(e.to_string()))?;
        let last_changed_ns = i128::from(info.commit_time.unwrap_or(0)) * NANOS_PER_SEC;

        let author = commit.author().map_err(|e| GitError::Walk(e.to_string()))?;
        // `&BStr` derefs to `&[u8]` for the identity hash.
        let identity = if author.email.is_empty() {
            author.name
        } else {
            author.email
        };
        let akey = author_key(identity);

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

        let mut touched: Vec<String> = changes.iter().filter_map(change_path).collect();
        touched.sort();
        touched.dedup();

        for path in &touched {
            let acc = churn.entry(path.clone()).or_insert_with(|| ChurnAccum {
                commits: 0,
                authors: BTreeSet::new(),
                last_changed_ns: i128::MIN,
            });
            acc.commits += 1;
            acc.authors.insert(akey);
            acc.last_changed_ns = acc.last_changed_ns.max(last_changed_ns);
        }

        // Skip co-change for oversized commits: the pair set is O(n²) and a
        // sweeping refactor is coupling noise, not real co-change signal.
        if touched.len() <= opts.max_files_per_commit as usize {
            for i in 0..touched.len() {
                for j in (i + 1)..touched.len() {
                    *pairs
                        .entry((touched[i].clone(), touched[j].clone()))
                        .or_insert(0) += 1;
                }
            }
        }
    }

    Ok(HistoryReport {
        churn: churn
            .into_iter()
            .map(|(path, acc)| FileChurn {
                path,
                commits: acc.commits,
                author_keys: acc.authors.into_iter().collect(),
                last_changed_ns: acc.last_changed_ns,
            })
            .collect(),
        pairs: pairs
            .into_iter()
            .map(|((a, b), count)| CoChangePair { a, b, count })
            .collect(),
    })
}

/// Changed-path extractor. A recursive tree diff also emits an entry for each
/// changed *directory* (tree) and submodule (commit); file-level churn counts
/// only blobs/symlinks, so non-file entries are skipped via `entry_mode`. Each
/// variant carries the destination `location` (the new path for a rewrite).
fn change_path(change: &ChangeDetached) -> Option<String> {
    let (entry_mode, location) = match change {
        ChangeDetached::Addition {
            entry_mode,
            location,
            ..
        }
        | ChangeDetached::Deletion {
            entry_mode,
            location,
            ..
        }
        | ChangeDetached::Modification {
            entry_mode,
            location,
            ..
        }
        | ChangeDetached::Rewrite {
            entry_mode,
            location,
            ..
        } => (entry_mode, location),
    };
    entry_mode
        .is_blob_or_symlink()
        .then(|| location.to_str_lossy().into_owned())
}

/// 8-byte author identity key: FNV-1a/64 over the author's email (or name
/// when the email is empty). Dependency-free and deterministic across runs,
/// so tier-11a can merge incremental author sets by union.
fn author_key(identity: &[u8]) -> [u8; 8] {
    const OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut hash = OFFSET_BASIS;
    for &byte in identity {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(PRIME);
    }
    hash.to_be_bytes()
}
