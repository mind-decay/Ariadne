//! Tier-04 step 9: durability test.
//!
//! Seed a HIGH-durability stdlib input alongside a LOW-durability project
//! input. Mutate the stdlib content with HIGH durability. A salsa query
//! whose only input is the LOW project file must not be recomputed when
//! the unrelated HIGH input changes — early-cutoff is what durability
//! buys us [src: <https://rust-analyzer.github.io/blog/2023/07/24/durable-incrementality.html>].

use std::sync::Arc;
use std::sync::Mutex;

use ariadne_salsa::{
    AriadneDb, FileContentInput, FileMetadataInput, ProjectConfigInput, ScipDocInput,
    symbols_for_file,
};
use salsa::{Durability, Setter};

#[test]
fn unrelated_high_durability_mutation_does_not_recompute_low_query() {
    let log = Arc::new(Mutex::new(Vec::<String>::new()));
    let mut db = AriadneDb::with_event_log(Arc::clone(&log));

    let _cfg = ProjectConfigInput::new(
        &db,
        "/repo".into(),
        vec!["rust".into()],
        Vec::<String>::new(),
    );

    // HIGH-durability stdlib input.
    let stdlib_path = "/usr/lib/rustlib/src/rust/library/core/src/lib.rs";
    let stdlib_content = FileContentInput::builder(
        stdlib_path.into(),
        b"pub fn core() {}\n".to_vec(),
        [9u8; 32],
    )
    .durability(Durability::HIGH)
    .new(&db);
    let _stdlib_meta = FileMetadataInput::builder("rust".into(), 16, 0)
        .durability(Durability::HIGH)
        .new(&db);
    let _stdlib_scip = ScipDocInput::builder(stdlib_path.into(), None)
        .durability(Durability::HIGH)
        .new(&db);

    // LOW-durability project input.
    let project_path = "/repo/src/lib.rs";
    let project_content =
        FileContentInput::builder(project_path.into(), b"fn user() {}\n".to_vec(), [3u8; 32])
            .durability(Durability::LOW)
            .new(&db);
    let _project_meta = FileMetadataInput::builder("rust".into(), 13, 0)
        .durability(Durability::LOW)
        .new(&db);
    let project_scip = ScipDocInput::builder(project_path.into(), None)
        .durability(Durability::LOW)
        .new(&db);

    // Warm the LOW project query.
    let baseline = symbols_for_file(&db, project_content, project_scip);
    log.lock().unwrap().clear();

    // Mutate the HIGH-durability stdlib content. The project query is
    // independent of the stdlib input, so salsa must not re-execute
    // `symbols_for_file(project_*)` on the next query.
    stdlib_content
        .set_content(&mut db)
        .with_durability(Durability::HIGH)
        .to(b"pub fn core_v2() {}\n".to_vec());

    let after = symbols_for_file(&db, project_content, project_scip);
    assert_eq!(baseline, after);

    let events = log.lock().unwrap().clone();
    assert!(
        !events.iter().any(|e| e.contains("symbols_for_file")),
        "LOW-durability project query must not re-execute when an unrelated \
         HIGH-durability input changes; events: {events:?}",
    );
}
