//! Tier-07a step 1: the shared per-file derivation produces real symbols and
//! resolves a cross-file edge through `commit_revision`.
//!
//! Seed a two-file Rust fixture — a `callee` definition and a `caller` that
//! calls it — by feeding parsed facts in through `SyntacticFactsInput` (salsa
//! may not parse; the composition root does [src: tests/architecture.rs]).
//! `commit_revision` derives both files' symbols, resolves the global
//! `References` edge, and writes the changeset to redb. Read the snapshot back
//! and assert the two symbols plus the cross-file edge exist.

use ariadne_core::{EdgeKind, FileId, FileRecord, Lang, ReadSnapshot, Storage};
use ariadne_salsa::{AriadneDb, CallRaw, DeclRaw, SyntacticFactsRaw};
use ariadne_storage::RedbStorage;

/// A Rust `FileRecord` with a deterministic, content-independent hash/mtime —
/// the derivation under test reads only `path`, `lang`, and `size`.
fn rust_record(path: &str, size: u64) -> FileRecord {
    FileRecord {
        path: path.to_owned(),
        lang: Lang::Rust,
        size,
        blake3: [0u8; 32],
        mtime_ns: 0,
    }
}

// `caller` / `callee` are the fixture's intentional domain vocabulary; the
// pedantic similar-names lint flags their shared prefix.
#[allow(clippy::similar_names)]
#[test]
fn commit_revision_derives_symbols_and_cross_file_edge() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let storage = RedbStorage::open(&tmp.path().join("index.redb")).expect("open redb");

    let mut db = AriadneDb::new();

    // File 1: `callee.rs` — `fn callee() {}` (one function decl, no calls).
    let callee_src = b"fn callee() {}\n".to_vec();
    let callee_facts = SyntacticFactsRaw {
        decls: vec![DeclRaw {
            kind: "function".to_owned(),
            name: "callee".to_owned(),
            name_byte_range: (3, 9),
            def_byte_range: (0, 14),
            visibility_byte: 3,
            attributes: Vec::new(),
            complexity: 0,
        }],
        ..SyntacticFactsRaw::default()
    };
    db.seed_file(
        FileId::new(1).expect("file id 1"),
        rust_record("callee.rs", callee_src.len() as u64),
        callee_src,
        callee_facts,
    );

    // File 2: `caller.rs` — `fn caller() { callee(); }` (one decl + one call).
    let caller_src = b"fn caller() { callee(); }\n".to_vec();
    let caller_facts = SyntacticFactsRaw {
        decls: vec![DeclRaw {
            kind: "function".to_owned(),
            name: "caller".to_owned(),
            name_byte_range: (3, 9),
            def_byte_range: (0, 25),
            visibility_byte: 3,
            attributes: Vec::new(),
            complexity: 0,
        }],
        calls: vec![CallRaw {
            callee: "callee".to_owned(),
            byte_range: (14, 20),
        }],
        ..SyntacticFactsRaw::default()
    };
    db.seed_file(
        FileId::new(2).expect("file id 2"),
        rust_record("caller.rs", caller_src.len() as u64),
        caller_src,
        caller_facts,
    );

    db.commit_revision(&storage).expect("commit revision");

    // Memory probe (plan R1): no salsa table exceeds the 256MB budget after
    // seeding + deriving. The per-table counters are still the tier-04 stub
    // (all zero), so this asserts the budget invariant holds, not a live
    // byte count [src: crates/ariadne-salsa/src/memory.rs].
    assert!(
        db.memory_report().over_budget().next().is_none(),
        "a salsa table exceeded the 256MB budget after seeding",
    );

    // Read the persisted snapshot back.
    let snap = storage.snapshot().expect("snapshot");
    let symbols: Vec<_> = snap
        .iter_symbols(1024)
        .expect("iter symbols")
        .flat_map(|chunk| chunk.expect("decode symbol chunk"))
        .collect();

    let callee = symbols
        .iter()
        .find(|(_, r)| r.canonical_name == "callee")
        .expect("callee symbol persisted");
    let caller = symbols
        .iter()
        .find(|(_, r)| r.canonical_name == "caller")
        .expect("caller symbol persisted");
    assert_eq!(callee.1.defining_file, FileId::new(1).expect("file id 1"));
    assert_eq!(caller.1.defining_file, FileId::new(2).expect("file id 2"));

    // The cross-file `References` edge caller -> callee.
    let out = snap.outgoing_edges(caller.0).expect("outgoing edges");
    assert!(
        out.iter()
            .any(|(k, _)| k.kind == EdgeKind::References && k.dst == callee.0),
        "expected a References edge caller -> callee; got {out:?}",
    );
}
