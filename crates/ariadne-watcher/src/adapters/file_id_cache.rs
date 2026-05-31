//! Gitignore-aware file-id cache for notify-debouncer-full (tier-01).
//!
//! The default [`notify_debouncer_full::FileIdMap`] populates its map at
//! `watch()` time by walking the whole tree with `walkdir` — no gitignore
//! filter — so it stat-s every entry under `target/`, `node_modules/`,
//! `.ariadne/`, etc. On this repo that is +153 ms for `target/`'s 35,494
//! files. This cache walks with the same [`ignore::WalkBuilder`] the
//! reconciler uses (so ignore semantics match) and applies the watcher's
//! [`Ignore`] matcher per entry, so the initial scan visits only the
//! indexed file set while keeping the rename-pair stitching the debouncer
//! relies on for those files [src:
//! notify-debouncer-full-0.7.0/src/{cache.rs,file_id_map.rs};
//! `crate::adapters::reconcile`].

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use ignore::WalkBuilder;
use notify::RecursiveMode;
use notify_debouncer_full::FileIdCache;
use notify_debouncer_full::file_id::{FileId, get_file_id};

use crate::adapters::ignore::{ARIADNE_IGNORE_FILENAME, Ignore};

/// A [`FileIdCache`] that holds file-ids only for non-ignored paths.
///
/// Construct it from the [`Ignore`] matcher the watcher already builds and
/// hand it to `new_debouncer_opt`; rename stitching for the indexed file
/// set is preserved while ignored directories are never scanned.
#[derive(Debug)]
pub struct GitignoreFileIdCache {
    paths: HashMap<PathBuf, FileId>,
    ignore: Arc<Ignore>,
}

impl GitignoreFileIdCache {
    /// Build an empty cache bound to `ignore`.
    #[must_use]
    pub fn new(ignore: Arc<Ignore>) -> Self {
        Self {
            paths: HashMap::new(),
            ignore,
        }
    }
}

impl FileIdCache for GitignoreFileIdCache {
    fn cached_file_id(&self, path: &Path) -> Option<impl AsRef<FileId>> {
        self.paths.get(path)
    }

    fn add_path(&mut self, path: &Path, recursive_mode: RecursiveMode) {
        if recursive_mode == RecursiveMode::Recursive {
            // Mirror the reconciler's walk so ignore semantics match, but
            // prune ignored directories at the walk level via `filter_entry`
            // keyed on the same `Ignore` matcher: `.git/`, `target/`,
            // `node_modules/`, `.ariadne/` are never descended into, so the
            // scan touches only the indexed file set. (DEFAULT_IGNORES are
            // not in a repo's own `.gitignore`, so without this the walk
            // would still `readdir` every object under `.git/`.)
            let ignore = Arc::clone(&self.ignore);
            let walker = WalkBuilder::new(path)
                .git_global(true)
                .hidden(false)
                .add_custom_ignore_filename(ARIADNE_IGNORE_FILENAME)
                .filter_entry(move |entry| {
                    let is_dir = entry.file_type().is_some_and(|t| t.is_dir());
                    !ignore.is_ignored(entry.path(), is_dir)
                })
                .build();
            for entry in walker.flatten() {
                let p = entry.path();
                if let Ok(file_id) = get_file_id(p) {
                    self.paths.insert(p.to_path_buf(), file_id);
                }
            }
        } else if !self.ignore.is_ignored(path, false) {
            if let Ok(file_id) = get_file_id(path) {
                self.paths.insert(path.to_path_buf(), file_id);
            }
        }
    }

    fn remove_path(&mut self, path: &Path) {
        self.paths.retain(|p, _| !p.starts_with(path));
    }
}
