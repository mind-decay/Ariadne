//! Tier-07b: edit-stable `SymbolId` + the incremental==full-rebuild invariant.
//!
//! Step 1 (stability): a 1-file fixture with two functions where the first
//! calls the second. Record the callee's `SymbolId`, prepend a blank line via
//! `rederive_file`, then assert the callee keeps its id and the caller->callee
//! edge survives. With the old offset-based id this is RED — the callee's byte
//! offset shifts, re-keying it and severing the edge [src: tier-07b step 1].

use std::fmt::Write as _;

use ariadne_core::{EdgeKind, FileId, FileRecord, Lang, ReadSnapshot, Storage, SymbolId};
use ariadne_salsa::{AriadneDb, CallRaw, DeclRaw, FileDerivation, SyntacticFactsRaw};
use ariadne_storage::RedbStorage;
use proptest::collection::vec as prop_vec;
use proptest::prelude::*;

/// A Rust `FileRecord` with a content-independent hash/mtime — the derivation
/// reads only `path`, `lang`, and `size`.
fn rust_record(path: &str, size: u64) -> FileRecord {
    FileRecord {
        path: path.to_owned(),
        lang: Lang::Rust,
        size,
        blake3: [0u8; 32],
        mtime_ns: 0,
    }
}

/// Facts for `m.rs` = `fn a() { b(); }\nfn b() {}\n`, every byte offset moved
/// right by `shift` (a prepended blank-line prefix). The decl names/kinds and
/// the call target are offset-independent; only the byte ranges move.
fn two_fn_facts(shift: u32) -> (Vec<u8>, SyntacticFactsRaw) {
    // Unshifted layout:
    //   `fn a() { b(); }`  -> a def (0,15); call `b` at (9,10)
    //   `\n`
    //   `fn b() {}`        -> b def (16,25)
    let prefix = "\n".repeat(shift as usize);
    let src = format!("{prefix}fn a() {{ b(); }}\nfn b() {{}}\n").into_bytes();
    let facts = SyntacticFactsRaw {
        decls: vec![
            DeclRaw {
                kind: "function".to_owned(),
                name: "a".to_owned(),
                name_byte_range: (shift + 3, shift + 4),
                def_byte_range: (shift, shift + 15),
                visibility_byte: 3,
                attributes: Vec::new(),
                complexity: 0,
            },
            DeclRaw {
                kind: "function".to_owned(),
                name: "b".to_owned(),
                name_byte_range: (shift + 19, shift + 20),
                def_byte_range: (shift + 16, shift + 25),
                visibility_byte: 3,
                attributes: Vec::new(),
                complexity: 0,
            },
        ],
        calls: vec![CallRaw {
            callee: "b".to_owned(),
            kind_byte: 0,
            byte_range: (shift + 9, shift + 10),
        }],
        ..SyntacticFactsRaw::default()
    };
    (src, facts)
}

/// Read back the `(a, b)` symbol ids from the committed snapshot.
fn ab_ids(storage: &RedbStorage) -> (SymbolId, SymbolId) {
    let snap = storage.snapshot().expect("snapshot");
    let syms: Vec<_> = snap
        .iter_symbols(1024)
        .expect("iter symbols")
        .flat_map(|chunk| chunk.expect("decode symbol chunk"))
        .collect();
    let a = syms
        .iter()
        .find(|(_, r)| r.canonical_name == "a")
        .expect("symbol a persisted")
        .0;
    let b = syms
        .iter()
        .find(|(_, r)| r.canonical_name == "b")
        .expect("symbol b persisted")
        .0;
    (a, b)
}

/// True if a `References` edge `src -> dst` is present.
fn has_ref_edge(storage: &RedbStorage, src: SymbolId, dst: SymbolId) -> bool {
    let snap = storage.snapshot().expect("snapshot");
    snap.outgoing_edges(src)
        .expect("outgoing edges")
        .iter()
        .any(|(k, _)| k.kind == EdgeKind::References && k.dst == dst)
}

#[test]
fn symbol_id_is_edit_stable_across_offset_shift() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let storage = RedbStorage::open(&tmp.path().join("index.redb")).expect("open redb");
    let mut db = AriadneDb::new();

    let fid = FileId::new(1).expect("file id 1");
    let (src0, facts0) = two_fn_facts(0);
    db.seed_file(fid, rust_record("m.rs", src0.len() as u64), src0, facts0);
    db.commit_revision(&storage).expect("initial commit");

    let (a_before, b_before) = ab_ids(&storage);
    assert!(
        has_ref_edge(&storage, a_before, b_before),
        "expected a References edge a -> b before the edit",
    );

    // Prepend a blank line (shift every offset by 1) and re-derive the file.
    let (src1, facts1) = two_fn_facts(1);
    db.rederive_file(
        FileDerivation {
            file_id: fid,
            record: rust_record("m.rs", src1.len() as u64),
            content: src1,
            facts: facts1,
        },
        &storage,
    )
    .expect("rederive file");

    let (a_after, b_after) = ab_ids(&storage);
    assert_eq!(
        b_before, b_after,
        "callee SymbolId must be stable across an offset-shifting edit",
    );
    assert_eq!(
        a_before, a_after,
        "caller SymbolId must be stable across an offset-shifting edit",
    );
    assert!(
        has_ref_edge(&storage, a_after, b_after),
        "the References edge a -> b must survive the offset-shifting edit",
    );
}

// --- Step 6: incremental == full-rebuild (divergence 0) proptest. -----------
//
// A small file universe `item0.rs .. item{N-1}.rs`. Each present file defines
// exactly one function `item{i}` and optionally calls one other file's
// `item{t}`; a call to an absent file resolves to no edge. A random sequence of
// Set (create/edit) and Del (delete) ops is applied incrementally via
// `rederive_file` / `forget_file`; a fresh `AriadneDb` is then built from the
// final state and committed once. The sorted persisted record sets must be
// byte-identical — proving stale symbols/edges/files are removed and nothing is
// missed (the invariant the tier-08 watcher depends on)
// [src: tier-07b steps 5-6; crates/ariadne-salsa/tests/equivalence.rs:104-161].

const NUM_FILES: usize = 4;

fn fid(i: usize) -> FileId {
    FileId::new(u32::try_from(i + 1).expect("fits u32")).expect("non-zero")
}

/// Build `item{i}.rs`: `fn item{i}() { item{t}(); }` (or no call), returning the
/// bytes, the parsed facts, and the file record. Byte ranges are computed as
/// the source string is assembled, so the facts match the content exactly.
fn build_file(i: usize, call: Option<usize>) -> (Vec<u8>, SyntacticFactsRaw, FileRecord) {
    let name = format!("item{i}");
    let mut src = String::from("fn ");
    let name_start = u32::try_from(src.len()).expect("fits u32");
    src.push_str(&name);
    let name_end = u32::try_from(src.len()).expect("fits u32");
    src.push_str("() {");
    let mut calls = Vec::new();
    if let Some(t) = call {
        src.push(' ');
        let callee = format!("item{t}");
        let c_start = u32::try_from(src.len()).expect("fits u32");
        src.push_str(&callee);
        let c_end = u32::try_from(src.len()).expect("fits u32");
        src.push_str("();");
        calls.push(CallRaw {
            callee,
            kind_byte: 0,
            byte_range: (c_start, c_end),
        });
    }
    src.push('}');
    let def_end = u32::try_from(src.len()).expect("fits u32");
    src.push('\n');
    let bytes = src.into_bytes();
    let facts = SyntacticFactsRaw {
        decls: vec![DeclRaw {
            kind: "function".to_owned(),
            name,
            name_byte_range: (name_start, name_end),
            def_byte_range: (0, def_end),
            visibility_byte: 3,
            attributes: Vec::new(),
            complexity: 0,
        }],
        calls,
        ..SyntacticFactsRaw::default()
    };
    let record = FileRecord {
        path: format!("item{i}.rs"),
        lang: Lang::Rust,
        size: bytes.len() as u64,
        blake3: [0u8; 32],
        mtime_ns: 0,
    };
    (bytes, facts, record)
}

/// Canonical, deterministic dump of every persisted record, sorted by id / key.
fn dump(storage: &RedbStorage) -> String {
    let snap = storage.snapshot().expect("snapshot");
    let mut files: Vec<_> = snap
        .iter_files(4096)
        .expect("iter files")
        .flat_map(|c| c.expect("file chunk"))
        .collect();
    files.sort_by_key(|(id, _)| id.get());
    let mut symbols: Vec<_> = snap
        .iter_symbols(4096)
        .expect("iter symbols")
        .flat_map(|c| c.expect("symbol chunk"))
        .collect();
    symbols.sort_by_key(|(id, _)| id.get());
    let mut edges: Vec<_> = snap
        .iter_edges(4096)
        .expect("iter edges")
        .flat_map(|c| c.expect("edge chunk"))
        .collect();
    edges.sort_by_key(|(k, _)| k.to_bytes());

    let mut out = String::new();
    out.push_str("== FILES ==\n");
    for (id, r) in &files {
        let _ = writeln!(
            out,
            "{}\t{}\t{}\t{}",
            id.get(),
            r.path,
            r.lang.tag(),
            r.size
        );
    }
    out.push_str("== SYMBOLS ==\n");
    for (id, r) in &symbols {
        let _ = writeln!(
            out,
            "{}\t{}\t{}\tfile={}\tspan={}:{}-{}",
            id.get(),
            r.canonical_name,
            r.kind,
            r.defining_file.get(),
            r.defining_span.file.get(),
            r.defining_span.byte_start,
            r.defining_span.byte_end,
        );
    }
    out.push_str("== EDGES ==\n");
    for (k, r) in &edges {
        let _ = writeln!(
            out,
            "{}\t{}\t{}\tspan={}:{}-{}",
            k.src.get(),
            k.kind.to_byte(),
            k.dst.get(),
            r.source_span.file.get(),
            r.source_span.byte_start,
            r.source_span.byte_end,
        );
    }
    out
}

#[derive(Debug, Clone)]
enum Op {
    /// Create or edit `item{file}.rs`, optionally calling `item{call}`.
    Set { file: usize, call: Option<usize> },
    /// Delete `item{file}.rs`.
    Del { file: usize },
}

fn ops_strategy() -> impl Strategy<Value = Vec<Op>> {
    let op = prop_oneof![
        (0..NUM_FILES, prop::option::of(0..NUM_FILES))
            .prop_map(|(file, call)| Op::Set { file, call }),
        (0..NUM_FILES).prop_map(|file| Op::Del { file }),
    ];
    prop_vec(op, 0..=20)
}

#[test]
fn incremental_sequence_equals_fresh_rebuild() {
    let mut runner = proptest::test_runner::TestRunner::new(ProptestConfig {
        cases: 100,
        ..ProptestConfig::default()
    });
    runner
        .run(&ops_strategy(), |ops| {
            // Final per-file state: Some(call_target) if present, None if absent.
            let mut state: Vec<Option<Option<usize>>> = vec![None; NUM_FILES];

            // Apply the sequence incrementally.
            let inc_dir = tempfile::tempdir().expect("tempdir");
            let inc_storage =
                RedbStorage::open(&inc_dir.path().join("inc.redb")).expect("open inc redb");
            let mut inc = AriadneDb::new();
            for op in &ops {
                match *op {
                    Op::Set { file, call } => {
                        let (content, facts, record) = build_file(file, call);
                        inc.rederive_file(
                            FileDerivation {
                                file_id: fid(file),
                                record,
                                content,
                                facts,
                            },
                            &inc_storage,
                        )
                        .expect("rederive");
                        state[file] = Some(call);
                    }
                    Op::Del { file } => {
                        inc.forget_file(&format!("item{file}.rs"), &inc_storage)
                            .expect("forget");
                        state[file] = None;
                    }
                }
            }

            // Memory probe (plan R1): after the incremental sequence no salsa
            // table exceeds the 256MB budget [src: tier-07b <verification>].
            // NOTE (audit I2): this assertion is currently a placeholder —
            // `memory_report()` is still the tier-04 zero-baseline stub
            // [src: crates/ariadne-salsa/src/memory.rs:44-57], so `over_budget()`
            // can never be non-empty. It becomes a live check once a future tier
            // wires real per-table counters (the tier-04-authorized fallback).
            prop_assert!(
                inc.memory_report().over_budget().next().is_none(),
                "a salsa table exceeded the 256MB budget after the incremental sequence",
            );

            // Build a fresh full index from the final state and commit once.
            let fresh_dir = tempfile::tempdir().expect("tempdir");
            let fresh_storage =
                RedbStorage::open(&fresh_dir.path().join("fresh.redb")).expect("open fresh redb");
            let mut fresh = AriadneDb::new();
            for (i, st) in state.iter().enumerate() {
                if let Some(call) = *st {
                    let (content, facts, record) = build_file(i, call);
                    fresh.seed_file(fid(i), record, content, facts);
                }
            }
            fresh.commit_revision(&fresh_storage).expect("fresh commit");

            prop_assert_eq!(
                dump(&inc_storage),
                dump(&fresh_storage),
                "incremental commit sequence diverged from a fresh full rebuild",
            );
            Ok(())
        })
        .unwrap();
}

// --- Audit I1: nth>0 disambiguator + R-B5 residual churn. -------------------
//
// Every fixture above uses a unique `(name, kind)` per file, so the intra-file
// occurrence-index disambiguator (`nth`) and the R-B5 residual-churn claim in
// ADR-0017 are otherwise unexercised. This fixture is one file whose only decls
// are `count` functions all named `dup` (same name, same kind): the k-th in
// source order gets `nth = k`, so the ids are distinct; an offset-shifting edit
// leaves them stable; and prepending another `dup` re-keys the later siblings
// (the accepted R-B5 churn) while an incremental commit still equals a fresh
// rebuild (divergence 0) [src: tier-07b audit I1; ADR-0017 R-B5].

/// `count` same-name same-kind functions `fn dup() {}` in one file, every byte
/// offset shifted right by `shift`. Byte ranges are computed as the source is
/// assembled, so the facts match the content exactly.
fn dup_fns(count: usize, shift: u32) -> (Vec<u8>, SyntacticFactsRaw, FileRecord) {
    let mut src = "\n".repeat(shift as usize);
    let mut decls = Vec::with_capacity(count);
    for _ in 0..count {
        let def_start = u32::try_from(src.len()).expect("fits u32");
        src.push_str("fn ");
        let name_start = u32::try_from(src.len()).expect("fits u32");
        src.push_str("dup");
        let name_end = u32::try_from(src.len()).expect("fits u32");
        src.push_str("() {}");
        let def_end = u32::try_from(src.len()).expect("fits u32");
        src.push('\n');
        decls.push(DeclRaw {
            kind: "function".to_owned(),
            name: "dup".to_owned(),
            name_byte_range: (name_start, name_end),
            def_byte_range: (def_start, def_end),
            visibility_byte: 3,
            attributes: Vec::new(),
            complexity: 0,
        });
    }
    let bytes = src.into_bytes();
    let facts = SyntacticFactsRaw {
        decls,
        ..SyntacticFactsRaw::default()
    };
    let record = rust_record("dup.rs", bytes.len() as u64);
    (bytes, facts, record)
}

/// All `dup` symbol ids from the committed snapshot in source order (sorted by
/// defining-span byte start).
fn dup_ids(storage: &RedbStorage) -> Vec<SymbolId> {
    let snap = storage.snapshot().expect("snapshot");
    let mut syms: Vec<_> = snap
        .iter_symbols(1024)
        .expect("iter symbols")
        .flat_map(|chunk| chunk.expect("decode symbol chunk"))
        .filter(|(_, r)| r.canonical_name == "dup")
        .collect();
    syms.sort_by_key(|(_, r)| r.defining_span.byte_start);
    syms.into_iter().map(|(id, _)| id).collect()
}

#[test]
fn symbol_id_disambiguates_same_name_kind_and_stays_divergence_zero() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let storage = RedbStorage::open(&tmp.path().join("dup.redb")).expect("open redb");
    let mut db = AriadneDb::new();
    let f = fid(0);

    // Two same-(name, kind) decls -> nth 0 and 1 -> distinct, non-zero ids.
    let (c0, facts0, rec0) = dup_fns(2, 0);
    db.rederive_file(
        FileDerivation {
            file_id: f,
            record: rec0,
            content: c0,
            facts: facts0,
        },
        &storage,
    )
    .expect("initial derive");
    let ids0 = dup_ids(&storage);
    assert_eq!(
        ids0.len(),
        2,
        "both same-name decls persist as distinct symbols",
    );
    assert_ne!(
        ids0[0], ids0[1],
        "nth disambiguates same-(name, kind) decls"
    );

    // Offset-shifting edit (prepend a blank line): the ids must be stable.
    let (c1, facts1, rec1) = dup_fns(2, 1);
    db.rederive_file(
        FileDerivation {
            file_id: f,
            record: rec1,
            content: c1,
            facts: facts1,
        },
        &storage,
    )
    .expect("offset-shift rederive");
    assert_eq!(
        ids0,
        dup_ids(&storage),
        "nth-based ids are stable across an offset-shifting edit",
    );

    // Before-insert: a third `dup` shifts the later siblings' nth (accepted R-B5
    // churn). The incremental result must still equal a fresh full rebuild.
    let (c2, facts2, rec2) = dup_fns(3, 0);
    db.rederive_file(
        FileDerivation {
            file_id: f,
            record: rec2,
            content: c2,
            facts: facts2,
        },
        &storage,
    )
    .expect("before-insert rederive");

    let fresh_dir = tempfile::tempdir().expect("tempdir");
    let fresh_storage =
        RedbStorage::open(&fresh_dir.path().join("fresh.redb")).expect("open fresh redb");
    let mut fresh = AriadneDb::new();
    let (fresh_content, fresh_facts, fresh_rec) = dup_fns(3, 0);
    fresh.seed_file(f, fresh_rec, fresh_content, fresh_facts);
    fresh.commit_revision(&fresh_storage).expect("fresh commit");

    assert_eq!(
        dup_ids(&storage).len(),
        3,
        "all three same-name decls persist after the before-insert",
    );
    assert_eq!(
        dump(&storage),
        dump(&fresh_storage),
        "incremental nth re-keying diverged from a fresh full rebuild",
    );
}
