//! Tier-01 invariant: the gitignore-aware file-id cache holds file-ids for
//! indexed source files but never for files under an ignored directory.
//!
//! Functional assertion only (map contents) — wall-clock lives in benches
//! [src: plan.md `<constraints>`].

use std::fs;

use ariadne_watcher::adapters::file_id_cache::GitignoreFileIdCache;
use ariadne_watcher::adapters::ignore::Ignore;
use notify::RecursiveMode;
use notify_debouncer_full::FileIdCache;

#[test]
fn caches_source_file_but_not_ignored_dir() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();

    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(root.join("src/a.rs"), b"fn a() {}\n").unwrap();
    fs::create_dir_all(root.join("target/debug")).unwrap();
    fs::write(root.join("target/debug/big.rs"), b"fn big() {}\n").unwrap();
    fs::create_dir_all(root.join(".git")).unwrap();
    fs::write(root.join(".git/HEAD"), b"ref: refs/heads/main\n").unwrap();

    let ignore = Ignore::build(root).unwrap();
    let mut cache = GitignoreFileIdCache::new(std::sync::Arc::new(ignore));
    cache.add_path(root, RecursiveMode::Recursive);

    assert!(
        cache.cached_file_id(&root.join("src/a.rs")).is_some(),
        "source file must hold a file-id for rename stitching"
    );
    assert!(
        cache
            .cached_file_id(&root.join("target/debug/big.rs"))
            .is_none(),
        "file under ignored target/ must never be cached"
    );
    assert!(
        cache.cached_file_id(&root.join(".git/HEAD")).is_none(),
        "file under the VCS .git/ dir must never be cached"
    );
}
