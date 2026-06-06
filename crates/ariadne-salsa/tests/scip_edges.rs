//! scip-driven-edges tier-01: SCIP occurrences drive precise cross-crate edges.
//!
//! The precise shape-gated resolver abstains on a `Method`/`Path` callee with no
//! same-file definition — a `socket.connect()` cross-crate call yields NO edge
//! (ADR-0025; r1-resolver-completion D1/D6). SCIP carries the globally-resolved
//! symbol the bare member name dropped, so with SCIP facts present the edge
//! resolves precisely, while a file with no SCIP facts keeps the abstaining
//! tree-sitter behaviour. Coverage is hash-gated: a file whose content hash no
//! longer matches the hash its SCIP facts were indexed at drops back to the
//! precise resolver (plan D3, D4). Facts are fed through `ScipFactsInput`
//! directly — salsa may not decode SCIP [src: tests/architecture.rs lines 30-43].

use ariadne_core::{EdgeKind, FileId, FileRecord, Lang, ReadSnapshot, Storage, SymbolId};
use ariadne_salsa::{
    AriadneDb, CallRaw, DeclRaw, ScipFactsRaw, ScipOccurrenceRaw, ScipRelationshipRaw,
    SyntacticFactsRaw,
};
use ariadne_storage::RedbStorage;

/// SCIP `SymbolRole::Definition` bit [src: crates/ariadne-scip/proto/scip.proto:526].
const SCIP_DEFINITION: u32 = 0x1;
/// SCIP `SymbolRole::WriteAccess` bit [src: crates/ariadne-scip/proto/scip.proto:530].
const SCIP_WRITE_ACCESS: u32 = 0x4;
/// SCIP `SymbolRole::ReadAccess` bit [src: crates/ariadne-scip/proto/scip.proto:532].
const SCIP_READ_ACCESS: u32 = 0x8;

/// A deterministic, non-zero content hash for a file id. The coverage gate
/// treats the all-zero default as "no SCIP facts", so a covered file's facts
/// must echo this exact (non-zero) hash; a different value models hash drift.
fn file_hash(file_id: u32) -> [u8; 32] {
    let mut h = [0u8; 32];
    h[..4].copy_from_slice(&file_id.to_be_bytes());
    h
}

/// A Rust `FileRecord` whose content hash is `file_hash(file_id)`.
fn record(path: &str, file_id: u32) -> FileRecord {
    FileRecord {
        path: path.to_owned(),
        lang: Lang::Rust,
        size: 101,
        blake3: file_hash(file_id),
        mtime_ns: 0,
    }
}

/// One public `fn <name>` spanning bytes `0..100`, name node at `3..10`.
fn fn_decl(name: &str) -> DeclRaw {
    DeclRaw {
        kind: "function".to_owned(),
        name: name.to_owned(),
        name_byte_range: (3, 10),
        def_byte_range: (0, 100),
        visibility_byte: 3,
        attributes: Vec::new(),
        complexity: 0,
    }
}

/// Seed a file with one `name` decl spanning the file plus the given call sites.
/// Its content hash is `file_hash(file_id)`.
fn seed(db: &mut AriadneDb, file_id: u32, path: &str, name: &str, calls: Vec<CallRaw>) {
    let facts = SyntacticFactsRaw {
        decls: vec![fn_decl(name)],
        calls,
        ..SyntacticFactsRaw::default()
    };
    db.seed_file(
        FileId::new(file_id).expect("nonzero file id"),
        record(path, file_id),
        vec![b' '; 101],
        facts,
    );
}

/// A `Method`-shaped (`recv.callee()`) call site nested in the file's decl. The
/// precise resolver refuses the cross-crate fallback for this shape.
fn method_call(callee: &str) -> CallRaw {
    CallRaw {
        callee: callee.to_owned(),
        kind_byte: 1,
        byte_range: (16, 24),
    }
}

/// One SCIP occurrence: a normalized symbol key, a byte range, and roles.
fn occ(symbol: &str, range: (u32, u32), roles: u32) -> ScipOccurrenceRaw {
    ScipOccurrenceRaw {
        symbol: symbol.to_owned(),
        byte_range: range,
        roles,
    }
}

/// Set a file's SCIP facts (occurrences only) with the given coverage hash.
fn set_facts(db: &mut AriadneDb, path: &str, occs: Vec<ScipOccurrenceRaw>, indexed_hash: [u8; 32]) {
    db.set_scip_facts(
        path,
        ScipFactsRaw {
            occurrences: occs,
            relationships: Vec::new(),
        },
        indexed_hash,
    );
}

/// Set a file's SCIP facts including relationships, with the given coverage hash.
fn set_facts_rel(
    db: &mut AriadneDb,
    path: &str,
    occs: Vec<ScipOccurrenceRaw>,
    rels: Vec<ScipRelationshipRaw>,
    indexed_hash: [u8; 32],
) {
    db.set_scip_facts(
        path,
        ScipFactsRaw {
            occurrences: occs,
            relationships: rels,
        },
        indexed_hash,
    );
}

fn commit_and_read(db: &mut AriadneDb, storage: &RedbStorage) -> Vec<(SymbolId, FileId, String)> {
    db.commit_revision(storage).expect("commit revision");
    let snap = storage.snapshot().expect("snapshot");
    snap.iter_symbols(1024)
        .expect("iter symbols")
        .flat_map(|chunk| chunk.expect("decode symbol chunk"))
        .map(|(id, rec)| (id, rec.defining_file, rec.canonical_name))
        .collect()
}

fn reference_targets(storage: &RedbStorage, src: SymbolId) -> Vec<SymbolId> {
    targets_of_kind(storage, src, EdgeKind::References)
}

/// Destinations of `src`'s outgoing edges of exactly `kind`.
fn targets_of_kind(storage: &RedbStorage, src: SymbolId, kind: EdgeKind) -> Vec<SymbolId> {
    let snap = storage.snapshot().expect("snapshot");
    snap.outgoing_edges(src)
        .expect("outgoing edges")
        .into_iter()
        .filter(|(k, _)| k.kind == kind)
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

/// The headline RED→GREEN. Crate A's `user` calls crate B's `connect` with a
/// `Method` shape and no import; SCIP records `connect`'s definition in B and a
/// reference in A under one global symbol key. With both files covered, the
/// occurrence pair resolves the precise `user -> connect` edge ADR-0025 refuses.
#[test]
fn cross_crate_method_call_resolves_with_scip_facts() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let storage = RedbStorage::open(&tmp.path().join("index.redb")).expect("open redb");
    let mut db = AriadneDb::new();

    seed(
        &mut db,
        1,
        "crates/crate_b/src/lib.rs",
        "connect",
        Vec::new(),
    );
    seed(
        &mut db,
        2,
        "crates/crate_a/src/lib.rs",
        "user",
        vec![method_call("connect")],
    );

    // SCIP: `connect` is defined in B (the def occurrence sits inside B's
    // `connect` span) and referenced from A's `user` body, both under the same
    // global symbol key. Both files carry their current content hash → covered.
    set_facts(
        &mut db,
        "crates/crate_b/src/lib.rs",
        vec![occ("scip:connect", (3, 10), SCIP_DEFINITION)],
        file_hash(1),
    );
    set_facts(
        &mut db,
        "crates/crate_a/src/lib.rs",
        vec![occ("scip:connect", (16, 24), 0)],
        file_hash(2),
    );

    let symbols = commit_and_read(&mut db, &storage);
    let user = id_in_file(&symbols, "user", 2);
    let connect = id_in_file(&symbols, "connect", 1);

    assert!(
        reference_targets(&storage, user).contains(&connect),
        "SCIP facts must resolve the cross-crate `user -> connect` edge the shape \
         gate drops",
    );
}

/// Control: the same shapes with NO SCIP facts keep the precise resolver's
/// abstention — a `Method`-shaped cross-crate callee yields no edge (ADR-0025).
#[test]
fn cross_crate_method_call_abstains_without_scip_facts() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let storage = RedbStorage::open(&tmp.path().join("index.redb")).expect("open redb");
    let mut db = AriadneDb::new();

    seed(
        &mut db,
        1,
        "crates/crate_b/src/lib.rs",
        "connect",
        Vec::new(),
    );
    seed(
        &mut db,
        2,
        "crates/crate_a/src/lib.rs",
        "user",
        vec![method_call("connect")],
    );

    let symbols = commit_and_read(&mut db, &storage);
    let user = id_in_file(&symbols, "user", 2);

    assert!(
        reference_targets(&storage, user).is_empty(),
        "without SCIP facts a method-shaped cross-crate callee must yield no edge",
    );
}

/// A std-callee occurrence — `Vec::new()` whose symbol has no definition in the
/// indexed workspace — maps to no `SymbolId`, so a covered file still yields no
/// edge for it (plan D3: an unmapped `dst` drops the edge).
#[test]
fn std_callee_occurrence_yields_no_edge() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let storage = RedbStorage::open(&tmp.path().join("index.redb")).expect("open redb");
    let mut db = AriadneDb::new();

    seed(
        &mut db,
        1,
        "crates/crate_a/src/lib.rs",
        "user",
        vec![method_call("new")],
    );
    // `user` references `Vec::new` (an external symbol) but no document defines
    // it, so the global map has no entry for the key.
    set_facts(
        &mut db,
        "crates/crate_a/src/lib.rs",
        vec![occ("scip:std::vec::Vec#new", (16, 24), 0)],
        file_hash(1),
    );

    let symbols = commit_and_read(&mut db, &storage);
    let user = id_in_file(&symbols, "user", 1);

    assert!(
        reference_targets(&storage, user).is_empty(),
        "an occurrence whose symbol has no indexed definition must yield no edge",
    );
}

/// A covered file whose content hash drifts away from its SCIP facts' indexed
/// hash falls back to the precise resolver (plan D4). With the resolver back in
/// charge, the `Method`-shaped cross-crate callee abstains again — the precise
/// `user -> connect` edge SCIP would have produced is NOT applied from stale
/// facts.
#[test]
fn edited_file_hash_drift_falls_back_to_precise_resolver() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let storage = RedbStorage::open(&tmp.path().join("index.redb")).expect("open redb");
    let mut db = AriadneDb::new();

    seed(
        &mut db,
        1,
        "crates/crate_b/src/lib.rs",
        "connect",
        Vec::new(),
    );
    seed(
        &mut db,
        2,
        "crates/crate_a/src/lib.rs",
        "user",
        vec![method_call("connect")],
    );

    set_facts(
        &mut db,
        "crates/crate_b/src/lib.rs",
        vec![occ("scip:connect", (3, 10), SCIP_DEFINITION)],
        file_hash(1),
    );
    // A's facts were indexed at a hash that no longer matches its record — an
    // edit landed after SCIP ran. A drops to the precise resolver.
    set_facts(
        &mut db,
        "crates/crate_a/src/lib.rs",
        vec![occ("scip:connect", (16, 24), 0)],
        file_hash(999),
    );

    let symbols = commit_and_read(&mut db, &storage);
    let user = id_in_file(&symbols, "user", 2);

    assert!(
        reference_targets(&storage, user).is_empty(),
        "a hash-drifted file must fall back to the resolver, which abstains on the \
         method-shaped callee — stale SCIP facts must not apply",
    );
}

/// Access roles promote occurrences to dedicated edges (tier-02, plan D5). `bump`
/// writes, reads, and plainly references the field `count` (defined in another
/// file under one global symbol key). The `WriteAccess` `0x4` occurrence yields a
/// `Writes` edge, the `ReadAccess` `0x8` occurrence a `Reads` edge, and the
/// bit-free occurrence a plain `References` edge — each emitted only on its
/// present role bit, with no fabrication.
#[test]
fn access_roles_drive_reads_and_writes_edges() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let storage = RedbStorage::open(&tmp.path().join("index.redb")).expect("open redb");
    let mut db = AriadneDb::new();

    seed(&mut db, 1, "crates/state/src/lib.rs", "count", Vec::new());
    seed(&mut db, 2, "crates/user/src/lib.rs", "bump", Vec::new());

    // SCIP: `count` is defined in file 1; file 2's `bump` writes it (0x4), reads
    // it (0x8), and references it with no access bit (0). All three occurrences
    // sit inside `bump`'s span and key the same global symbol. Both files carry
    // their current content hash → covered.
    set_facts(
        &mut db,
        "crates/state/src/lib.rs",
        vec![occ("scip:count", (3, 10), SCIP_DEFINITION)],
        file_hash(1),
    );
    set_facts(
        &mut db,
        "crates/user/src/lib.rs",
        vec![
            occ("scip:count", (16, 24), SCIP_WRITE_ACCESS),
            occ("scip:count", (30, 38), SCIP_READ_ACCESS),
            occ("scip:count", (40, 48), 0),
        ],
        file_hash(2),
    );

    let symbols = commit_and_read(&mut db, &storage);
    let bump = id_in_file(&symbols, "bump", 2);
    let count = id_in_file(&symbols, "count", 1);

    assert_eq!(
        targets_of_kind(&storage, bump, EdgeKind::Writes),
        vec![count],
        "the WriteAccess occurrence must yield a single `Writes` edge `bump -> count`",
    );
    assert_eq!(
        targets_of_kind(&storage, bump, EdgeKind::Reads),
        vec![count],
        "the ReadAccess occurrence must yield a single `Reads` edge `bump -> count`",
    );
    assert_eq!(
        targets_of_kind(&storage, bump, EdgeKind::References),
        vec![count],
        "the access-bit-free occurrence must stay a plain `References` edge (no fabrication)",
    );
}

/// Precedence is Write > Read when an occurrence carries both bits (tier-02
/// step 3): the occurrence resolves to a single `Writes` edge and no `Reads`
/// edge — the derivation never double-emits from one occurrence.
#[test]
fn write_access_takes_precedence_over_read_access() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let storage = RedbStorage::open(&tmp.path().join("index.redb")).expect("open redb");
    let mut db = AriadneDb::new();

    seed(&mut db, 1, "crates/state/src/lib.rs", "count", Vec::new());
    seed(&mut db, 2, "crates/user/src/lib.rs", "bump", Vec::new());

    set_facts(
        &mut db,
        "crates/state/src/lib.rs",
        vec![occ("scip:count", (3, 10), SCIP_DEFINITION)],
        file_hash(1),
    );
    // One occurrence with both WriteAccess and ReadAccess set (`x += 1` shape).
    set_facts(
        &mut db,
        "crates/user/src/lib.rs",
        vec![occ(
            "scip:count",
            (16, 24),
            SCIP_WRITE_ACCESS | SCIP_READ_ACCESS,
        )],
        file_hash(2),
    );

    let symbols = commit_and_read(&mut db, &storage);
    let bump = id_in_file(&symbols, "bump", 2);
    let count = id_in_file(&symbols, "count", 1);

    assert_eq!(
        targets_of_kind(&storage, bump, EdgeKind::Writes),
        vec![count],
        "a read+write occurrence resolves to a single `Writes` edge (Write > Read)",
    );
    assert!(
        targets_of_kind(&storage, bump, EdgeKind::Reads).is_empty(),
        "a read+write occurrence must not also emit a `Reads` edge",
    );
}

/// One SCIP relationship: from/to normalized keys plus the two edge flags.
fn rel(from: &str, to: &str, is_impl: bool, is_type: bool) -> ScipRelationshipRaw {
    ScipRelationshipRaw {
        from: from.to_owned(),
        to: to.to_owned(),
        is_implementation: is_impl,
        is_type_definition: is_type,
    }
}

/// tier-03 headline (plan T3): a trait-impl relationship yields an `Implements`
/// edge from the impl symbol to the trait symbol (graph `Overrides`). Crate
/// `dog`'s `Dog` implements crate `animal`'s `Animal`; SCIP records both
/// definitions under global keys plus an `is_implementation` relationship on
/// `Dog`. With both files covered, the relationship resolves the `Dog -> Animal`
/// edge no syntactic pass can produce.
#[test]
fn implementation_relationship_yields_implements_edge() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let storage = RedbStorage::open(&tmp.path().join("index.redb")).expect("open redb");
    let mut db = AriadneDb::new();

    seed(&mut db, 1, "crates/animal/src/lib.rs", "Animal", Vec::new());
    seed(&mut db, 2, "crates/dog/src/lib.rs", "Dog", Vec::new());

    // `Animal` is defined in file 1; `Dog` in file 2, with a relationship
    // declaring `Dog` implements `Animal`. Both def occurrences sit inside their
    // symbol's span and key the global map; both files carry their content hash.
    set_facts(
        &mut db,
        "crates/animal/src/lib.rs",
        vec![occ("scip:Animal", (3, 10), SCIP_DEFINITION)],
        file_hash(1),
    );
    set_facts_rel(
        &mut db,
        "crates/dog/src/lib.rs",
        vec![occ("scip:Dog", (3, 10), SCIP_DEFINITION)],
        vec![rel("scip:Dog", "scip:Animal", true, false)],
        file_hash(2),
    );

    let symbols = commit_and_read(&mut db, &storage);
    let dog = id_in_file(&symbols, "Dog", 2);
    let animal = id_in_file(&symbols, "Animal", 1);

    assert_eq!(
        targets_of_kind(&storage, dog, EdgeKind::Implements),
        vec![animal],
        "is_implementation must yield a single `Implements` edge `Dog -> Animal`",
    );
}

/// A type-definition relationship yields a `TypeOf` edge from the binding to its
/// type symbol (plan T3). `app`'s `binding` is typed by `types`'s `Animal`; SCIP
/// records an `is_type_definition` relationship on `binding`, resolving the
/// `binding -> Animal` edge.
#[test]
fn type_definition_relationship_yields_typeof_edge() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let storage = RedbStorage::open(&tmp.path().join("index.redb")).expect("open redb");
    let mut db = AriadneDb::new();

    seed(&mut db, 1, "crates/types/src/lib.rs", "Animal", Vec::new());
    seed(&mut db, 2, "crates/app/src/lib.rs", "binding", Vec::new());

    set_facts(
        &mut db,
        "crates/types/src/lib.rs",
        vec![occ("scip:Animal", (3, 10), SCIP_DEFINITION)],
        file_hash(1),
    );
    set_facts_rel(
        &mut db,
        "crates/app/src/lib.rs",
        vec![occ("scip:binding", (3, 10), SCIP_DEFINITION)],
        vec![rel("scip:binding", "scip:Animal", false, true)],
        file_hash(2),
    );

    let symbols = commit_and_read(&mut db, &storage);
    let binding = id_in_file(&symbols, "binding", 2);
    let animal = id_in_file(&symbols, "Animal", 1);

    assert_eq!(
        targets_of_kind(&storage, binding, EdgeKind::TypeOf),
        vec![animal],
        "is_type_definition must yield a single `TypeOf` edge `binding -> Animal`",
    );
}

/// A relationship to a symbol with no indexed definition — an external supertype
/// — maps to no `SymbolId`, so it drops (plan D3/T3: an unmapped endpoint drops
/// the edge), exactly as a std-callee occurrence does.
#[test]
fn relationship_to_unindexed_symbol_drops_edge() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let storage = RedbStorage::open(&tmp.path().join("index.redb")).expect("open redb");
    let mut db = AriadneDb::new();

    seed(&mut db, 1, "crates/dog/src/lib.rs", "Dog", Vec::new());
    // `Dog` implements an external trait the workspace never defines, so the
    // global key map has no entry for `to`.
    set_facts_rel(
        &mut db,
        "crates/dog/src/lib.rs",
        vec![occ("scip:Dog", (3, 10), SCIP_DEFINITION)],
        vec![rel("scip:Dog", "scip:external::Trait", true, false)],
        file_hash(1),
    );

    let symbols = commit_and_read(&mut db, &storage);
    let dog = id_in_file(&symbols, "Dog", 1);

    assert!(
        targets_of_kind(&storage, dog, EdgeKind::Implements).is_empty(),
        "a relationship whose target has no indexed definition must yield no edge",
    );
}
