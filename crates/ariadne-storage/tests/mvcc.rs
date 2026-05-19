//! MVCC stress: 16 reader threads + 1 writer thread for 5s. Asserts both
//! ends make progress and that the cached revision counter never regresses
//! across observed reads
//! [src: .claude/plans/ariadne-core/tier-02-storage.md step 9].

mod support;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread;
use std::time::Duration;

use ariadne_core::{Changeset, FileId, FileRecord, Lang, ReadSnapshot, Storage, WriteTxn};
use ariadne_storage::RedbStorage;

const READERS: usize = 16;
const DURATION: Duration = Duration::from_secs(5);

#[test]
fn mvcc_readers_and_writer_make_progress() {
    let dir = tempfile::tempdir().expect("tempdir");
    let storage = RedbStorage::open(&dir.path().join("index.redb")).expect("open");

    let stop = Arc::new(AtomicBool::new(false));
    let writer_iters = Arc::new(AtomicU64::new(0));
    let reader_iters = Arc::new(AtomicU64::new(0));

    let mut handles = Vec::with_capacity(READERS + 1);

    for _ in 0..READERS {
        let s = storage.clone();
        let stop = Arc::clone(&stop);
        let counter = Arc::clone(&reader_iters);
        handles.push(thread::spawn(move || {
            let mut last_rev: u64 = 0;
            let fid = FileId::new(1).expect("nonzero");
            while !stop.load(Ordering::Acquire) {
                let snap = s.snapshot().expect("snapshot");
                // Read MUST decode cleanly whether or not the writer has
                // committed yet (post-MVCC snapshot point).
                let _ = snap.file(fid).expect("file lookup");
                let rev_now = s.revision().0;
                assert!(
                    rev_now >= last_rev,
                    "revision regressed: {last_rev} -> {rev_now}",
                );
                last_rev = rev_now;
                counter.fetch_add(1, Ordering::Relaxed);
            }
        }));
    }

    {
        let s = storage.clone();
        let stop = Arc::clone(&stop);
        let counter = Arc::clone(&writer_iters);
        handles.push(thread::spawn(move || {
            let mut n: u64 = 0;
            while !stop.load(Ordering::Acquire) {
                let fid = FileId::new(1 + (u32::try_from(n % 64).unwrap_or(0))).expect("nonzero");
                let rec = FileRecord {
                    path: format!("file-{n}.rs"),
                    lang: Lang::Rust,
                    size: n,
                    blake3: [0u8; 32],
                    mtime_ns: i128::from(n),
                };
                let cs = Changeset::new().upsert_file(fid, rec);
                let txn = s.begin_write().expect("begin write");
                txn.apply(&cs).expect("apply");
                n += 1;
                counter.fetch_add(1, Ordering::Relaxed);
            }
        }));
    }

    thread::sleep(DURATION);
    stop.store(true, Ordering::Release);
    for h in handles {
        h.join().expect("join");
    }

    let writes = writer_iters.load(Ordering::Relaxed);
    let reads = reader_iters.load(Ordering::Relaxed);
    assert!(writes > 0, "writer made no progress in {DURATION:?}");
    assert!(reads > 0, "readers made no progress in {DURATION:?}");
    // Sanity: 16 readers × 5s shouldn't be starved by the writer.
    assert!(
        reads > writes,
        "readers ({reads}) starved by writer ({writes})",
    );
}
