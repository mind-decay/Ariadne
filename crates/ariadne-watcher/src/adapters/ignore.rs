//! `.gitignore` + `.ariadneignore` matcher used to filter notify events
//! and to walk the tree during reconciliation. Wraps `ignore::Gitignore`
//! so notify event paths and reconcile walk paths share one matcher
//! [src: <https://docs.rs/ignore>].

use std::path::{Path, PathBuf};

use ignore::gitignore::{Gitignore, GitignoreBuilder};

use crate::errors::WatcherError;

/// Hard-coded ignore patterns common to every Ariadne workspace. Mirrors
/// the tier-06 plan step 3 list — adapter-internal, not part of any port.
/// `.git/` is included so the watcher never tracks the VCS metadata dir
/// (never indexed; not listed in a repo's own `.gitignore`) — without it
/// the startup file-id scan stat-s every object under `.git/` (tier-01).
const DEFAULT_IGNORES: &[&str] = &["target/", "node_modules/", ".ariadne/", ".git/"];

/// File name of the per-project Ariadne ignore file. Higher precedence
/// than `.gitignore` per the plan letter so contributors can opt files
/// in/out of indexing without touching git tracking semantics.
pub const ARIADNE_IGNORE_FILENAME: &str = ".ariadneignore";

/// Compiled ignore matcher pinned to a workspace root.
///
/// On `macOS`, `FSEvents`-delivered paths arrive under `/private/var/...`
/// while a tempdir handed in by tests can read as `/var/...`. Both
/// spellings are tracked so [`Ignore::is_ignored`] can match either
/// flavour without panicking on the `matched_path_or_any_parents`
/// "path under root" guard.
#[derive(Debug)]
pub struct Ignore {
    matcher: Gitignore,
    roots: Vec<PathBuf>,
}

impl Ignore {
    /// Build the matcher: hard-coded defaults first (lowest precedence),
    /// then the workspace `.gitignore`, then `.ariadneignore` (highest).
    /// Missing files are skipped.
    ///
    /// The root is canonicalized so notify event paths (which `macOS`
    /// `FSEvents` returns under `/private/var/...`) match against the
    /// same shape as the matcher root
    /// [src: <https://docs.rs/ignore/0.4.25/ignore/gitignore/struct.Gitignore.html#method.matched_path_or_any_parents>
    /// — panics when the path is not under the root].
    ///
    /// # Errors
    /// Returns [`WatcherError::IgnoreBuild`] on a malformed pattern.
    pub fn build(root: &Path) -> Result<Self, WatcherError> {
        let roots = collect_roots(root);
        let primary = primary_root(&roots);
        let mut builder = GitignoreBuilder::new(primary);
        for pat in DEFAULT_IGNORES {
            builder
                .add_line(None, pat)
                .map_err(|e| WatcherError::IgnoreBuild(format!("default `{pat}`: {e}")))?;
        }
        let gitignore = primary.join(".gitignore");
        if gitignore.exists() {
            if let Some(err) = builder.add(&gitignore) {
                return Err(WatcherError::IgnoreBuild(format!(".gitignore: {err}")));
            }
        }
        let ariadne = primary.join(ARIADNE_IGNORE_FILENAME);
        if ariadne.exists() {
            if let Some(err) = builder.add(&ariadne) {
                return Err(WatcherError::IgnoreBuild(format!(
                    "{ARIADNE_IGNORE_FILENAME}: {err}"
                )));
            }
        }
        let matcher = builder
            .build()
            .map_err(|e| WatcherError::IgnoreBuild(e.to_string()))?;
        Ok(Self { matcher, roots })
    }

    /// Build a matcher with only the hard-coded defaults. Used by tests
    /// that want a deterministic baseline.
    ///
    /// # Errors
    /// Same shape as [`Self::build`].
    pub fn defaults_only(root: &Path) -> Result<Self, WatcherError> {
        let roots = collect_roots(root);
        let primary = primary_root(&roots);
        let mut builder = GitignoreBuilder::new(primary);
        for pat in DEFAULT_IGNORES {
            builder
                .add_line(None, pat)
                .map_err(|e| WatcherError::IgnoreBuild(format!("default `{pat}`: {e}")))?;
        }
        let matcher = builder
            .build()
            .map_err(|e| WatcherError::IgnoreBuild(e.to_string()))?;
        Ok(Self { matcher, roots })
    }

    /// True if `path` (or any ancestor up to [`Self::root`]) matches an
    /// ignore rule. `is_dir` must be `true` for directory paths so
    /// `target/` style entries match correctly per `ignore` semantics
    /// [src: <https://docs.rs/ignore/0.4/ignore/gitignore/struct.Gitignore.html>].
    ///
    /// Paths outside the matcher root are reported as not-ignored — the
    /// underlying `matched_path_or_any_parents` panics on out-of-tree
    /// paths, so the guard keeps notify event handlers crash-free when a
    /// symlink target lives outside the workspace.
    #[must_use]
    pub fn is_ignored(&self, path: &Path, is_dir: bool) -> bool {
        let candidates = [path.to_path_buf(), canonicalize_or_owned(path)];
        for candidate in &candidates {
            for root in &self.roots {
                if let Ok(rel) = candidate.strip_prefix(root) {
                    return self
                        .matcher
                        .matched_path_or_any_parents(rel, is_dir)
                        .is_ignore();
                }
            }
        }
        false
    }

    /// Primary root the matcher was built against — the canonicalized
    /// workspace root if available, otherwise the input root.
    #[must_use]
    pub fn root(&self) -> &Path {
        primary_root(&self.roots)
    }
}

fn canonicalize_or_owned(path: &Path) -> PathBuf {
    // canonicalize() requires the path to exist on disk. Fall back to the
    // input when the path does not yet exist (notify event for a file
    // that was already removed by the time the handler runs).
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

fn collect_roots(root: &Path) -> Vec<PathBuf> {
    let mut roots = Vec::with_capacity(2);
    roots.push(root.to_path_buf());
    if let Ok(canonical) = std::fs::canonicalize(root) {
        if canonical != root {
            roots.push(canonical);
        }
    }
    roots
}

fn primary_root(roots: &[PathBuf]) -> &Path {
    // Canonical wins when present so the GitignoreBuilder anchors to the
    // shape notify events arrive in. `collect_roots` puts the input first
    // and pushes the canonical second; this helper inverts that order.
    roots.last().map_or_else(|| Path::new(""), PathBuf::as_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn defaults_ignore_target_dir() {
        let tmp = tempdir().unwrap();
        let ig = Ignore::defaults_only(tmp.path()).unwrap();
        assert!(ig.is_ignored(&tmp.path().join("target/debug/foo"), false));
        assert!(!ig.is_ignored(&tmp.path().join("src/lib.rs"), false));
    }

    #[test]
    fn defaults_ignore_git_dir() {
        let tmp = tempdir().unwrap();
        let ig = Ignore::defaults_only(tmp.path()).unwrap();
        assert!(ig.is_ignored(&tmp.path().join(".git/HEAD"), false));
        assert!(ig.is_ignored(&tmp.path().join(".git"), true));
    }

    #[test]
    fn ariadneignore_takes_precedence() {
        let tmp = tempdir().unwrap();
        std::fs::write(tmp.path().join(".ariadneignore"), "*.snap\n").unwrap();
        let ig = Ignore::build(tmp.path()).unwrap();
        assert!(ig.is_ignored(&tmp.path().join("foo.snap"), false));
        assert!(!ig.is_ignored(&tmp.path().join("foo.rs"), false));
    }
}
