//! Tier-06 step 8: proptest the ignore matcher.

use ariadne_watcher::adapters::ignore::Ignore;
use proptest::prelude::*;
use tempfile::tempdir;

#[test]
fn target_dir_is_always_ignored() {
    let tmp = tempdir().unwrap();
    let ig = Ignore::defaults_only(tmp.path()).unwrap();
    assert!(ig.is_ignored(&tmp.path().join("target/debug/x"), false));
    assert!(ig.is_ignored(&tmp.path().join("target/release/y"), false));
    assert!(ig.is_ignored(&tmp.path().join("target"), true));
}

#[test]
fn ariadne_dir_is_always_ignored() {
    let tmp = tempdir().unwrap();
    let ig = Ignore::defaults_only(tmp.path()).unwrap();
    assert!(ig.is_ignored(&tmp.path().join(".ariadne/index.redb"), false));
}

#[test]
fn node_modules_is_always_ignored() {
    let tmp = tempdir().unwrap();
    let ig = Ignore::defaults_only(tmp.path()).unwrap();
    assert!(ig.is_ignored(&tmp.path().join("node_modules/foo/index.js"), false));
}

#[test]
fn ariadneignore_pattern_is_applied() {
    let tmp = tempdir().unwrap();
    std::fs::write(tmp.path().join(".ariadneignore"), "*.snap\n").unwrap();
    let ig = Ignore::build(tmp.path()).unwrap();
    assert!(ig.is_ignored(&tmp.path().join("foo.snap"), false));
    assert!(!ig.is_ignored(&tmp.path().join("foo.rs"), false));
}

proptest! {
    // Random segment names that never overlap with the ignored dirs above.
    #[test]
    fn random_source_paths_are_tracked(
        seg in "[a-z][a-z0-9_]{0,12}",
        depth in 0_usize..4,
        ext in prop::sample::select(vec!["rs", "py", "ts", "go", "java", "kt"]),
    ) {
        let tmp = tempdir().unwrap();
        let ig = Ignore::defaults_only(tmp.path()).unwrap();
        let mut path = tmp.path().to_path_buf();
        for _ in 0..depth {
            path.push(&seg);
        }
        path.push(format!("{seg}.{ext}"));
        let is_in_target = path
            .components()
            .any(|c| matches!(c.as_os_str().to_str(), Some("target" | "node_modules" | ".ariadne")));
        prop_assert!(!is_in_target, "test path must not collide with default ignores: {path:?}");
        prop_assert!(!ig.is_ignored(&path, false), "tracked path mis-ignored: {path:?}");
    }

    #[test]
    fn random_paths_under_target_are_ignored(
        seg in "[a-z][a-z0-9_]{0,12}",
        depth in 1_usize..4,
    ) {
        let tmp = tempdir().unwrap();
        let ig = Ignore::defaults_only(tmp.path()).unwrap();
        let mut path = tmp.path().join("target");
        for _ in 0..depth {
            path.push(&seg);
        }
        prop_assert!(ig.is_ignored(&path, false), "target/-rooted path leaked: {path:?}");
    }
}
