//! Tier-06 step 9: reconciliation catches files the notify stream missed.
//!
//! Strategy: build a `Reconciler` directly (no notify thread) and step it
//! twice. First pass seeds the hash store. We then mutate the file behind
//! the reconciler's back; the second pass must emit a `HashDrift`.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use ariadne_core::{Invalidation, WatcherSink};
use ariadne_watcher::adapters::ignore::Ignore;
use ariadne_watcher::adapters::reconcile::Reconciler;
use tempfile::tempdir;

#[derive(Debug, Default)]
struct CapturingSink {
    events: Arc<Mutex<Vec<Invalidation>>>,
}

impl CapturingSink {
    fn new() -> (Self, Arc<Mutex<Vec<Invalidation>>>) {
        let events = Arc::new(Mutex::new(Vec::new()));
        (
            Self {
                events: Arc::clone(&events),
            },
            events,
        )
    }
}

impl WatcherSink for CapturingSink {
    fn apply_invalidation(&mut self, inv: &Invalidation) {
        self.events.lock().unwrap().push(inv.clone());
    }
}

#[test]
fn second_pass_emits_hash_drift_for_modified_file() {
    let tmp = tempdir().unwrap();
    let root: PathBuf = tmp.path().to_path_buf();
    let target = root.join("a.rs");
    std::fs::write(&target, b"v1").unwrap();

    let ignore = Arc::new(Ignore::build(&root).unwrap());
    let (sink, events) = CapturingSink::new();
    let mut reconciler = Reconciler::new(root.clone(), Arc::clone(&ignore));

    // Pass 1: seed hashes. No drift yet — sink should observe zero
    // HashDrift events because everything is "new".
    let report1 = reconciler.run_pass(&mut { sink });
    assert!(
        report1.errors.is_empty(),
        "pass 1 errors: {:?}",
        report1.errors
    );
    assert!(report1.files_checked >= 1);
    // First-seen files emit an initial HashDrift from all-zero → real
    // hash so downstream sinks bootstrap their state.
    let initial = events.lock().unwrap().clone();
    assert!(
        initial
            .iter()
            .any(|e| matches!(e, Invalidation::HashDrift { path, .. } if path == &target)),
        "expected initial HashDrift seeding for {target:?}, got {initial:?}",
    );
    events.lock().unwrap().clear();

    // Mutate without telling the watcher.
    std::fs::write(&target, b"v2-different-length").unwrap();

    let (sink2, events2) = CapturingSink::new();
    let report2 = reconciler.run_pass(&mut { sink2 });
    assert!(
        report2.errors.is_empty(),
        "pass 2 errors: {:?}",
        report2.errors
    );

    let drifts: Vec<_> = events2.lock().unwrap().clone();
    assert!(
        drifts
            .iter()
            .any(|e| matches!(e, Invalidation::HashDrift { path, .. } if path == &target)),
        "expected HashDrift for {target:?} on second pass, got {drifts:?}",
    );
    assert_eq!(report2.drifts_emitted, drifts.len());
}

#[test]
fn unchanged_file_emits_no_drift_on_subsequent_pass() {
    let tmp = tempdir().unwrap();
    let root = tmp.path().to_path_buf();
    let target = root.join("stable.rs");
    std::fs::write(&target, b"contents").unwrap();
    let ignore = Arc::new(Ignore::build(&root).unwrap());
    let mut reconciler = Reconciler::new(root, Arc::clone(&ignore));

    let mut seeding = CapturingSink::new().0;
    let _ = reconciler.run_pass(&mut seeding);

    let (mut sink, events) = {
        let (s, e) = CapturingSink::new();
        (s, e)
    };
    let report = reconciler.run_pass(&mut sink);
    assert_eq!(report.drifts_emitted, 0, "drift on unchanged file");
    assert!(events.lock().unwrap().is_empty());
}

#[test]
fn ignored_paths_are_skipped() {
    let tmp = tempdir().unwrap();
    let root = tmp.path().to_path_buf();
    std::fs::create_dir_all(root.join("target")).unwrap();
    std::fs::write(root.join("target/skipme"), b"x").unwrap();
    std::fs::write(root.join("tracked.rs"), b"y").unwrap();
    let ignore = Arc::new(Ignore::build(&root).unwrap());
    let mut reconciler = Reconciler::new(root.clone(), Arc::clone(&ignore));

    let (mut sink, events) = CapturingSink::new();
    let _ = reconciler.run_pass(&mut sink);
    let seen: Vec<_> = events.lock().unwrap().clone();
    assert!(
        seen.iter()
            .all(|e| !e.path().starts_with(root.join("target")))
    );
    assert!(seen.iter().any(|e| e.path() == &root.join("tracked.rs")));
}
