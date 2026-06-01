//! Golden snapshot of a deterministic 5-file × 15-symbol × 20-edge
//! changeset round-tripped through the redb adapter
//! [src: .claude/plans/ariadne-core/tier-02-storage.md step 11].

mod support;

use std::fmt::Write as _;

use ariadne_core::{
    Changeset, EdgeKey, EdgeKind, EdgeRecord, FileId, FileRecord, Lang, ReadSnapshot, Span,
    Storage, SymbolId, SymbolRecord, Visibility, WriteTxn,
};

fn fid(n: u32) -> FileId {
    FileId::new(n).expect("nonzero")
}

fn sid(n: u64) -> SymbolId {
    SymbolId::new(n).expect("nonzero")
}

fn deterministic_changeset() -> Changeset {
    let mut cs = Changeset::new();

    for f in 1u32..=5 {
        cs = cs.upsert_file(
            fid(f),
            FileRecord {
                path: format!("src/file_{f}.rs"),
                lang: Lang::Rust,
                size: u64::from(f) * 1000,
                blake3: [u8::try_from(f).expect("fits u8"); 32],
                mtime_ns: i128::from(f),
            },
        );
    }

    for s in 1u64..=15 {
        let owner = u32::try_from((s - 1) / 3 + 1).expect("fits u32");
        let span = Span {
            file: fid(owner),
            byte_start: u32::try_from(s * 10).expect("fits"),
            byte_end: u32::try_from(s * 10 + 5).expect("fits"),
        };
        cs = cs.upsert_symbol(
            sid(s),
            SymbolRecord {
                canonical_name: format!("sym_{s:02}"),
                kind: "function".to_owned(),
                defining_file: fid(owner),
                defining_span: span,
                visibility: Visibility::Unknown,
                attributes: Vec::new(),
            },
        );
    }

    let kinds = [EdgeKind::Defines, EdgeKind::References, EdgeKind::Imports];
    for e in 0u64..20 {
        let src = sid((e % 15) + 1);
        let dst = sid(((e + 7) % 15) + 1);
        let kind = kinds[usize::try_from(e % 3).expect("fits")];
        let owner_file = fid(u32::try_from((e % 5) + 1).expect("fits"));
        let span = Span {
            file: owner_file,
            byte_start: u32::try_from(e * 100).expect("fits"),
            byte_end: u32::try_from(e * 100 + 10).expect("fits"),
        };
        cs = cs.add_edge(
            EdgeKey { src, kind, dst },
            EdgeRecord {
                source_span: span,
                evidence_lang: Lang::Rust,
                weight: u32::try_from(e).expect("fits"),
            },
        );
    }
    cs
}

#[test]
fn golden_changeset_dump() {
    let (storage, _dir) = support::fresh_storage();
    let cs = deterministic_changeset();
    storage
        .begin_write()
        .expect("begin write")
        .apply(&cs)
        .expect("apply");
    let snap = storage.snapshot().expect("snapshot");

    let mut out = String::new();
    writeln!(out, "== FILES ==").unwrap();
    for f in 1u32..=5 {
        let rec = snap.file(fid(f)).expect("file");
        writeln!(out, "{f}: {rec:#?}").unwrap();
    }

    writeln!(out, "== SYMBOLS (per file, sorted by name) ==").unwrap();
    for f in 1u32..=5 {
        writeln!(out, "file {f}:").unwrap();
        let mut syms = snap.symbols_in_file(fid(f)).expect("symbols_in_file");
        syms.sort_by(|a, b| a.canonical_name.cmp(&b.canonical_name));
        for s in syms {
            writeln!(out, "  {s:#?}").unwrap();
        }
    }

    writeln!(out, "== EDGES (outgoing per symbol) ==").unwrap();
    for s in 1u64..=15 {
        let mut edges = snap.outgoing_edges(sid(s)).expect("outgoing");
        edges.sort_by_key(|(k, _)| (k.src, k.kind as u8, k.dst));
        if !edges.is_empty() {
            writeln!(out, "src {s}:").unwrap();
            for (k, r) in edges {
                writeln!(out, "  {k:#?} -> {r:#?}").unwrap();
            }
        }
    }

    writeln!(out, "== EDGES_BY_FILE ==").unwrap();
    for f in 1u32..=5 {
        let mut keys = snap.edges_in_file(fid(f)).expect("edges_in_file");
        keys.sort_by_key(|k| (k.src, k.kind as u8, k.dst));
        writeln!(out, "{f}: {keys:#?}").unwrap();
    }

    insta::assert_snapshot!(out);
}

#[test]
fn reopen_with_mismatched_schema_version_returns_schema_mismatch() {
    use ariadne_core::StorageError;
    use redb::{Database, TableDefinition};

    const META: TableDefinition<'_, &str, u64> = TableDefinition::new("meta");

    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("index.redb");

    // Bootstrap META with the current SCHEMA_VERSION via the production path.
    drop(ariadne_storage::RedbStorage::open(&path).expect("bootstrap"));

    // Force the on-disk schema_version above current — no registered
    // migration path covers it, so rebuild-on-mismatch must still apply.
    let db = Database::open(&path).expect("raw open");
    let txn = db.begin_write().expect("begin_write");
    {
        let mut meta = txn.open_table(META).expect("open meta");
        meta.insert("schema_version", &99u64).expect("insert");
    }
    txn.commit().expect("commit");
    drop(db);

    // Re-opening through the adapter must surface the typed mismatch error.
    let err = ariadne_storage::RedbStorage::open(&path).expect_err("expected mismatch");
    match err {
        StorageError::SchemaMismatch { found, expected } => {
            assert_eq!(found, 99);
            assert_eq!(expected, 5);
        }
        other => panic!("expected SchemaMismatch, got {other:?}"),
    }
}

#[test]
fn schema_version_and_revision_survive_reopen() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("index.redb");
    let revision_after_first_apply = {
        let storage = ariadne_storage::RedbStorage::open(&path).expect("open 1");
        let cs = deterministic_changeset();
        let new_rev = storage
            .begin_write()
            .expect("begin")
            .apply(&cs)
            .expect("apply");
        assert_eq!(new_rev.0, 1, "first commit yields revision 1");
        new_rev.0
    };
    let storage = ariadne_storage::RedbStorage::open(&path).expect("reopen");
    let observed = storage.revision().0;
    assert_eq!(
        observed, revision_after_first_apply,
        "revision survives reopen",
    );
}
