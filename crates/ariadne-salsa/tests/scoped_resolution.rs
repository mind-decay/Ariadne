//! R1 regression: index-time call resolution must be scope-aware.
//!
//! A callee is captured as bare identifier text — `Vec::new()` and a workspace
//! `Foo::new()` both collapse to `new` [src: rust.scm:38-40]. The pre-fix
//! resolver bound such a bare name to the *first* same-named workspace symbol
//! when no same-file match existed [src: derive.rs `resolve_edges`], so a call
//! to a name defined in several crates bound to an arbitrary out-of-scope one.
//!
//! These tests pin the scoped behaviour (ADR-0024): a callee resolves to a
//! same-file → same-crate → unambiguous-global definition; an ambiguous callee
//! with no in-scope definition (the std `Vec::new()` shape) yields no edge.
//! Facts are fed through `SyntacticFactsInput` directly — salsa may not parse
//! [src: crates/ariadne-salsa/tests/derivation.rs].

use ariadne_core::{EdgeKind, FileId, FileRecord, Lang, ReadSnapshot, Storage, SymbolId};
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

/// One public `fn <name>() {}` declaration spanning the whole file.
fn fn_decl(name: &str, def_end: u32) -> DeclRaw {
    DeclRaw {
        kind: "function".to_owned(),
        name: name.to_owned(),
        name_byte_range: (3, 9),
        def_byte_range: (0, def_end),
        visibility_byte: 3,
        attributes: Vec::new(),
        complexity: 0,
    }
}

/// Seed a file carrying one function decl and zero or more bare-name call
/// sites, all nested inside that decl's span.
fn seed_fn_with_calls(
    db: &mut AriadneDb,
    file_id: u32,
    path: &str,
    fn_name: &str,
    callees: &[&str],
) {
    // A body wide enough to contain every call range below.
    let body_end: u32 = 256;
    let calls = callees
        .iter()
        .enumerate()
        .map(|(i, callee)| {
            let start = 16 + u32::try_from(i).expect("few calls") * 16;
            CallRaw {
                callee: (*callee).to_owned(),
                byte_range: (start, start + 8),
            }
        })
        .collect();
    let facts = SyntacticFactsRaw {
        decls: vec![fn_decl(fn_name, body_end)],
        calls,
        ..SyntacticFactsRaw::default()
    };
    let content = vec![b' '; body_end as usize + 1];
    db.seed_file(
        FileId::new(file_id).expect("nonzero file id"),
        rust_record(path, content.len() as u64),
        content,
        facts,
    );
}

/// Commit the seeded derivation and return the persisted symbols.
fn commit_and_read(db: &mut AriadneDb, storage: &RedbStorage) -> Vec<(SymbolId, FileId, String)> {
    db.commit_revision(storage).expect("commit revision");
    let snap = storage.snapshot().expect("snapshot");
    snap.iter_symbols(1024)
        .expect("iter symbols")
        .flat_map(|chunk| chunk.expect("decode symbol chunk"))
        .map(|(id, rec)| (id, rec.defining_file, rec.canonical_name))
        .collect()
}

/// Outgoing `References` edge destinations of `src`, read from the snapshot.
fn reference_targets(storage: &RedbStorage, src: SymbolId) -> Vec<SymbolId> {
    let snap = storage.snapshot().expect("snapshot");
    snap.outgoing_edges(src)
        .expect("outgoing edges")
        .into_iter()
        .filter(|(k, _)| k.kind == EdgeKind::References)
        .map(|(k, _)| k.dst)
        .collect()
}

fn id_in_file(symbols: &[(SymbolId, FileId, String)], name: &str, file: u32) -> SymbolId {
    let want = FileId::new(file).expect("nonzero file id");
    symbols
        .iter()
        .find(|(_, f, n)| n == name && *f == want)
        .map_or_else(
            || panic!("symbol `{name}` in file {file} not persisted"),
            |(id, _, _)| *id,
        )
}

/// R1 repro. Two crates each define `helper`; crate A's caller calls `helper`.
/// The pre-fix first-global fallback bound the call to crate B's `helper`
/// (the lower `FileId`); scoped resolution binds it to crate A's own `helper`.
#[test]
fn same_crate_call_resolves_within_caller_crate_not_collision() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let storage = RedbStorage::open(&tmp.path().join("index.redb")).expect("open redb");
    let mut db = AriadneDb::new();

    // File 1 (lowest FileId) — crate B's `helper`: the pre-fix `candidates
    // .first()` pick, and the wrong target.
    seed_fn_with_calls(&mut db, 1, "crates/crate_b/src/lib.rs", "helper", &[]);
    // File 2 — crate A's own `helper`: the in-scope, correct target.
    seed_fn_with_calls(&mut db, 2, "crates/crate_a/src/lib.rs", "helper", &[]);
    // File 3 — crate A's caller, in a different file from A's `helper`.
    seed_fn_with_calls(
        &mut db,
        3,
        "crates/crate_a/src/caller.rs",
        "user",
        &["helper"],
    );

    let symbols = commit_and_read(&mut db, &storage);
    let user = id_in_file(&symbols, "user", 3);
    let helper_a = id_in_file(&symbols, "helper", 2);
    let helper_b = id_in_file(&symbols, "helper", 1);

    let targets = reference_targets(&storage, user);
    assert!(
        targets.contains(&helper_a),
        "expected the call to bind to crate A's own `helper`; got {targets:?}",
    );
    assert!(
        !targets.contains(&helper_b),
        "call wrongly bound to crate B's `helper` (cross-crate name collision)",
    );
}

/// Std-callee shape. `Vec::new()` captures the bare name `new`, which is
/// defined in several crates but none in the caller's crate. An ambiguous
/// callee with no in-scope definition resolves to no edge.
#[test]
fn ambiguous_callee_with_no_in_scope_definition_yields_no_edge() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let storage = RedbStorage::open(&tmp.path().join("index.redb")).expect("open redb");
    let mut db = AriadneDb::new();

    // Two crates define `new` (ambiguous), mirroring the real workspace where
    // `new` is a ubiquitous constructor name.
    seed_fn_with_calls(&mut db, 1, "crates/crate_b/src/lib.rs", "new", &[]);
    seed_fn_with_calls(&mut db, 2, "crates/crate_c/src/lib.rs", "new", &[]);
    // The caller crate defines no `new`; it calls `Vec::new()` → bare `new`.
    seed_fn_with_calls(&mut db, 3, "crates/crate_a/src/lib.rs", "user", &["new"]);

    let symbols = commit_and_read(&mut db, &storage);
    let user = id_in_file(&symbols, "user", 3);

    assert!(
        reference_targets(&storage, user).is_empty(),
        "std-shaped ambiguous callee `new` must yield no edge from the caller",
    );
}

/// Recall guard. A callee defined exactly once workspace-wide is unambiguous,
/// so a cross-crate call still resolves (the `beta::run → alpha::helper` shape
/// from the `ariadne doc` fixture). Scoping must not drop this legitimate edge.
#[test]
fn unambiguous_global_callee_resolves_cross_crate() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let storage = RedbStorage::open(&tmp.path().join("index.redb")).expect("open redb");
    let mut db = AriadneDb::new();

    // The only `helper` in the workspace lives in crate A.
    seed_fn_with_calls(&mut db, 1, "crates/crate_a/src/lib.rs", "helper", &[]);
    // Crate B calls it with no import statement (the fixture shape).
    seed_fn_with_calls(&mut db, 2, "crates/crate_b/src/lib.rs", "run", &["helper"]);

    let symbols = commit_and_read(&mut db, &storage);
    let run = id_in_file(&symbols, "run", 2);
    let helper_a = id_in_file(&symbols, "helper", 1);

    assert!(
        reference_targets(&storage, run).contains(&helper_a),
        "unambiguous cross-crate callee must still resolve to its single definition",
    );
}
