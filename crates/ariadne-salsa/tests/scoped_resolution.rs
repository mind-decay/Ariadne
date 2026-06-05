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

/// Seed a file carrying one function decl and zero or more bare-name (`Free`)
/// call sites, all nested inside that decl's span. `Free` is the shape the
/// existing scoped-resolution tests assert; the spike tests use
/// [`seed_kinded_caller`] for the `Method`/`Path` shapes.
fn seed_fn_with_calls(
    db: &mut AriadneDb,
    file_id: u32,
    path: &str,
    fn_name: &str,
    callees: &[&str],
) {
    // Call ranges sit at 16-byte strides, all inside the decl span seeded by
    // `seed_file_with_calls`.
    let calls = callees
        .iter()
        .enumerate()
        .map(|(i, callee)| {
            let start = 16 + u32::try_from(i).expect("few calls") * 16;
            CallRaw {
                callee: (*callee).to_owned(),
                kind_byte: 0,
                byte_range: (start, start + 8),
            }
        })
        .collect();
    seed_file_with_calls(db, file_id, path, fn_name, calls);
}

/// Seed a `user` caller file whose single call site to `callee` carries the
/// given shape byte (`1=Method`, `2=Path`). The spike tests use this to
/// reproduce the `socket.connect()` / `Foo::new()` phantom shapes.
fn seed_kinded_caller(db: &mut AriadneDb, file_id: u32, path: &str, callee: &str, kind_byte: u8) {
    let calls = vec![CallRaw {
        callee: callee.to_owned(),
        kind_byte,
        byte_range: (16, 24),
    }];
    seed_file_with_calls(db, file_id, path, "user", calls);
}

/// Materialise one seeded file: a single `fn_name` decl spanning the file and
/// the given call sites nested inside it.
fn seed_file_with_calls(
    db: &mut AriadneDb,
    file_id: u32,
    path: &str,
    fn_name: &str,
    calls: Vec<CallRaw>,
) {
    let body_end: u32 = 256;
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
    // File 3 — crate A's caller, in a different file from A's `helper`. Free
    // shape: the same-crate tier resolves it regardless of the gate.
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
    // Crate B calls it with no import statement (the fixture shape). Free shape:
    // the unambiguous-global tier must still resolve it across crates.
    seed_fn_with_calls(&mut db, 2, "crates/crate_b/src/lib.rs", "run", &["helper"]);

    let symbols = commit_and_read(&mut db, &storage);
    let run = id_in_file(&symbols, "run", 2);
    let helper_a = id_in_file(&symbols, "helper", 1);

    assert!(
        reference_targets(&storage, run).contains(&helper_a),
        "unambiguous cross-crate callee must still resolve to its single definition",
    );
}

/// R1 completion spike — method-shaped phantom. `socket.connect()` captures the
/// bare member name `connect`, defined exactly once workspace-wide (in crate B)
/// but never in the caller's crate. The pre-gate resolver bound it cross-crate
/// via the `unambiguous-global` tier — the phantom afferent edge. The shape gate
/// refuses that fallback for `Method` calls, so a method-shaped cross-crate
/// callee with no in-scope definition yields NO edge.
#[test]
fn method_shaped_cross_crate_callee_yields_no_edge() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let storage = RedbStorage::open(&tmp.path().join("index.redb")).expect("open redb");
    let mut db = AriadneDb::new();

    // The only `connect` in the workspace lives in crate B.
    seed_fn_with_calls(&mut db, 1, "crates/crate_b/src/lib.rs", "connect", &[]);
    // Crate A calls `socket.connect()` → captured as a Method-shaped `connect`.
    seed_kinded_caller(&mut db, 2, "crates/crate_a/src/lib.rs", "connect", 1);

    let symbols = commit_and_read(&mut db, &storage);
    let user = id_in_file(&symbols, "user", 2);

    assert!(
        reference_targets(&storage, user).is_empty(),
        "method-shaped cross-crate callee `connect` must yield no edge (the phantom)",
    );
}

/// R1 completion spike — path-shaped phantom twin. `Foo::new()` captures the
/// bare associated name `build`, unique workspace-wide but out of the caller's
/// crate. The gate refuses the cross-crate fallback for `Path` calls too.
#[test]
fn path_shaped_cross_crate_callee_yields_no_edge() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let storage = RedbStorage::open(&tmp.path().join("index.redb")).expect("open redb");
    let mut db = AriadneDb::new();

    // The only `build` in the workspace lives in crate B.
    seed_fn_with_calls(&mut db, 1, "crates/crate_b/src/lib.rs", "build", &[]);
    // Crate A calls `Foo::build()` → captured as a Path-shaped `build`.
    seed_kinded_caller(&mut db, 2, "crates/crate_a/src/lib.rs", "build", 2);

    let symbols = commit_and_read(&mut db, &storage);
    let user = id_in_file(&symbols, "user", 2);

    assert!(
        reference_targets(&storage, user).is_empty(),
        "path-shaped cross-crate callee `build` must yield no edge (the phantom)",
    );
}
