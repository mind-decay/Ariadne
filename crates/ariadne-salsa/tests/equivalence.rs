//! Tier-04 step 2: failing cache-hit test, and step 8: proptest equivalence
//! between an incrementally-updated database and a fresh full-rebuild.
//!
//! The cache-hit test asserts that mutating one file's content does NOT
//! re-run the tracked `symbols_for_file` for a different file. We use
//! `salsa::EventKind::WillExecute` as the canonical recompute signal
//! [src: salsa-rs/salsa examples/lazy-input — event-driven recompute log].

use std::sync::Arc;
use std::sync::Mutex;

use ariadne_salsa::{
    AriadneDb, FileContentInput, FileMetadataInput, ProjectConfigInput, SyntacticFactsInput,
    SyntacticFactsRaw, symbols_for_file,
};
use pretty_assertions::assert_eq;
use proptest::collection::vec as prop_vec;
use proptest::prelude::*;
use salsa::{Durability, Setter};

struct FileInputs {
    content: FileContentInput,
    facts: SyntacticFactsInput,
}

fn seed_file(db: &AriadneDb, i: usize, payload: &[u8]) -> FileInputs {
    let path = format!("/repo/f{i}.rs");
    let content = FileContentInput::builder(path, seed_content(i, payload), seed_hash(i))
        .durability(Durability::LOW)
        .new(db);
    let _ = FileMetadataInput::builder("rust".into(), 0, 0)
        .durability(Durability::LOW)
        .new(db);
    let facts = SyntacticFactsInput::builder(SyntacticFactsRaw::default())
        .durability(Durability::LOW)
        .new(db);
    FileInputs { content, facts }
}

#[test]
fn cache_hit_on_unrelated_edit() {
    let recompute_log = Arc::new(Mutex::new(Vec::<String>::new()));
    let mut db = AriadneDb::with_event_log(Arc::clone(&recompute_log));

    let _cfg = ProjectConfigInput::new(
        &db,
        "/repo".into(),
        vec!["rust".into()],
        Vec::<String>::new(),
    );
    let file_a = seed_file(&db, 0, b"");
    let file_b = seed_file(&db, 1, b"");

    // Warm both files.
    let baseline_a = symbols_for_file(&db, file_a.content, file_a.facts);
    let _ = symbols_for_file(&db, file_b.content, file_b.facts);
    recompute_log.lock().unwrap().clear();

    // Mutate file_b only.
    file_b
        .content
        .set_content(&mut db)
        .with_durability(Durability::LOW)
        .to(b"fn b_changed() {}\n".to_vec());

    // Re-query file_a; result equals baseline. Because file_b is not
    // re-queried, file_b's downstream never executes either. The only
    // permissible recompute events post-edit are zero — any event
    // mentioning `symbols_for_file` is a cache miss.
    let after_a = symbols_for_file(&db, file_a.content, file_a.facts);
    assert_eq!(baseline_a, after_a);

    let events = recompute_log.lock().unwrap().clone();
    let symbols_recomputed = events.iter().any(|e| e.contains("symbols_for_file"));
    assert!(
        !symbols_recomputed,
        "symbols_for_file must hit the salsa cache when only an unrelated \
         file's content is mutated and that file is not re-queried; events: {events:?}",
    );
}

#[derive(Debug, Clone)]
struct Edit {
    file: usize,
    payload: Vec<u8>,
}

fn edits_strategy(num_files: usize) -> impl Strategy<Value = Vec<Edit>> {
    prop_vec(
        (0..num_files, prop_vec(any::<u8>(), 0..=32))
            .prop_map(|(file, payload)| Edit { file, payload }),
        0..=16,
    )
}

#[test]
fn fresh_vs_incremental_equivalence() {
    const NUM_FILES: usize = 5;
    let mut runner = proptest::test_runner::TestRunner::new(ProptestConfig {
        cases: 100,
        ..ProptestConfig::default()
    });
    runner
        .run(&edits_strategy(NUM_FILES), |edits| {
            let mut inc = AriadneDb::new();
            let _cfg = ProjectConfigInput::new(
                &inc,
                "/repo".into(),
                vec!["rust".into()],
                Vec::<String>::new(),
            );
            let files: Vec<FileInputs> = (0..NUM_FILES).map(|i| seed_file(&inc, i, &[])).collect();
            let mut final_payload: Vec<Vec<u8>> =
                (0..NUM_FILES).map(|i| seed_content(i, &[])).collect();
            for Edit { file, payload } in &edits {
                final_payload[*file] = seed_content(*file, payload);
                files[*file]
                    .content
                    .set_content(&mut inc)
                    .with_durability(Durability::LOW)
                    .to(final_payload[*file].clone());
            }
            let incremental: Vec<_> = files
                .iter()
                .map(|f| symbols_for_file(&inc, f.content, f.facts))
                .collect();

            let fresh = AriadneDb::new();
            let _cfg = ProjectConfigInput::new(
                &fresh,
                "/repo".into(),
                vec!["rust".into()],
                Vec::<String>::new(),
            );
            // For the fresh DB, seed each file with its final payload.
            let fresh_files: Vec<FileInputs> = (0..NUM_FILES)
                .map(|i| {
                    let payload = final_payload[i]
                        .strip_prefix(seed_content(i, &[]).as_slice())
                        .unwrap_or(&final_payload[i]);
                    seed_file(&fresh, i, payload)
                })
                .collect();
            let fresh_results: Vec<_> = fresh_files
                .iter()
                .map(|f| symbols_for_file(&fresh, f.content, f.facts))
                .collect();

            prop_assert_eq!(incremental, fresh_results);
            Ok(())
        })
        .unwrap();
}

fn seed_content(i: usize, payload: &[u8]) -> Vec<u8> {
    let mut out = format!("fn f{i}(){{}}\n").into_bytes();
    out.extend_from_slice(payload);
    out
}

fn seed_hash(i: usize) -> [u8; 32] {
    let mut h = [0u8; 32];
    h[..8].copy_from_slice(&(i as u64).to_be_bytes());
    h
}
