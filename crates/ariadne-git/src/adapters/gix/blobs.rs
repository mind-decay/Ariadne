//! Base-blob reader (block A, A2): the exact bytes of given paths at an
//! arbitrary revision.
//!
//! Generalizes the `head_blob_bytes` idiom in `diff.rs` to any revspec so the
//! API-surface diff can reconstruct a changed file's base surface by
//! re-parsing its blob at the base ref. No `gix` type crosses the boundary:
//! the revspec resolves here and only owned `(path, bytes)` pairs return
//! [src: crates/ariadne-git/src/adapters/gix/diff.rs:72-78,148-160 ;
//!  .claude/plans/intelligence-platform/block-a/plan.md D4 ; docs.rs/gix/0.84.0].

use std::path::Path;

use super::line_hunks::blob_bytes;
use crate::errors::GitError;

/// Read the exact blob bytes of each path in `paths` at revision `rev` in the
/// repository rooted at `repo_root`, returning `(path, bytes)` pairs sorted by
/// path so the output is deterministic across runs.
///
/// A path absent from `rev`'s tree is **skipped, not an error** — only changed
/// files are queried, and a file added since `rev` simply has no base blob, so
/// the caller reads its absence as "new on the head side".
///
/// No `gix` type crosses the boundary (folder-layout rule 4): the revspec
/// inside `rev` is resolved here and only owned `ariadne-core`-compatible
/// `(String, Vec<u8>)` pairs are returned.
///
/// # Errors
/// [`GitError`] on repository open, revspec resolution, tree lookup, or blob
/// read.
pub fn read_blobs_at(
    repo_root: &Path,
    rev: &str,
    paths: &[String],
) -> Result<Vec<(String, Vec<u8>)>, GitError> {
    let repo = gix::open(repo_root).map_err(|e| GitError::Open(e.to_string()))?;
    // Resolve the revspec → commit → tree, reusing the `diff.rs` rev/tree idiom
    // so an unknown ref surfaces as `Revspec` and a corrupt tree as `Diff`.
    let id = repo
        .rev_parse_single(rev)
        .map_err(|e| GitError::Revspec(e.to_string()))?;
    let tree = repo
        .find_commit(id)
        .map_err(|e| GitError::Revspec(e.to_string()))?
        .tree()
        .map_err(|e| GitError::Diff(e.to_string()))?;

    let mut out: Vec<(String, Vec<u8>)> = Vec::with_capacity(paths.len());
    for path in paths {
        // A path absent at `rev` (added since the base) yields no entry: it is
        // skipped, never an error — the head side reads its absence as "new".
        if let Some(entry) = tree
            .lookup_entry_by_path(Path::new(path))
            .map_err(|e| GitError::Diff(e.to_string()))?
        {
            out.push((path.clone(), blob_bytes(&repo, entry.object_id())?));
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(out)
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::process::Command;

    use tempfile::TempDir;

    use super::read_blobs_at;

    /// Run `git` in `repo`, isolated from any ambient user/global config so the
    /// fixture is reproducible on any host. Mirrors the temp-repo style of
    /// `tests/diff.rs`. Panics on non-zero exit.
    fn git(repo: &Path, args: &[&str]) {
        let output = Command::new("git")
            .current_dir(repo)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_SYSTEM", "/dev/null")
            .env("GIT_AUTHOR_NAME", "t")
            .env("GIT_AUTHOR_EMAIL", "t@x")
            .env("GIT_COMMITTER_NAME", "t")
            .env("GIT_COMMITTER_EMAIL", "t@x")
            .args(args)
            .output()
            .expect("spawn git");
        assert!(
            output.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&output.stderr),
        );
    }

    fn commit(repo: &Path, path: &str, content: &str) {
        std::fs::write(repo.join(path), content).expect("write fixture");
        git(repo, &["add", path]);
        git(repo, &["commit", "-m", "c", "--no-gpg-sign"]);
    }

    /// A two-commit fixture: c1 (HEAD~1) writes the base bytes of `a.txt`; c2
    /// (HEAD) overwrites them with the head bytes.
    fn two_commit_repo() -> TempDir {
        let tmp = tempfile::tempdir().expect("tempdir");
        let p = tmp.path();
        git(p, &["init", "-b", "main"]);
        commit(p, "a.txt", "base bytes\n");
        commit(p, "a.txt", "head bytes\n");
        tmp
    }

    #[test]
    fn reads_exact_base_blob_at_prior_revision() {
        let repo = two_commit_repo();
        let got = read_blobs_at(repo.path(), "HEAD~1", &["a.txt".to_owned()]).expect("read blobs");
        assert_eq!(
            got,
            vec![("a.txt".to_owned(), b"base bytes\n".to_vec())],
            "HEAD~1 holds the base bytes, byte-exact",
        );
    }

    #[test]
    fn skips_absent_path_without_error() {
        let repo = two_commit_repo();
        let got = read_blobs_at(
            repo.path(),
            "HEAD~1",
            &["missing.txt".to_owned(), "a.txt".to_owned()],
        )
        .expect("read blobs");
        // `missing.txt` does not exist at HEAD~1: skipped, not errored. The
        // surviving `a.txt` proves the absence is silent and output is sorted.
        assert_eq!(got, vec![("a.txt".to_owned(), b"base bytes\n".to_vec())]);
    }
}
