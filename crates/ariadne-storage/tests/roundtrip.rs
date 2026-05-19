//! Codec + storage roundtrip proptests.
//!
//! Two layers
//! [src: .claude/plans/ariadne-core/tier-02-storage.md step 1 + `exit_criteria`]:
//!
//! 1. Codec roundtrip (postcard, ID byte-order): ≥10K cases per record type
//!    — pure CPU, the strict gate the exit criteria reference.
//! 2. Storage roundtrip (through `RedbStorage`): write a single-record
//!    `Changeset`, open a `ReadSnapshot`, assert equality. Smaller case
//!    count because each case opens a fresh redb-backed tempdir.

mod support;

use ariadne_core::{
    Changeset, EdgeKey, EdgeRecord, FileId, FileRecord, IdEncode, ReadSnapshot, Storage, SymbolId,
    SymbolRecord, WriteTxn,
};
use proptest::prelude::*;

use support::{
    arb_edge_key, arb_edge_record, arb_file_id, arb_file_record, arb_symbol_id, arb_symbol_record,
    fresh_storage,
};

fn postcard_roundtrip<T>(value: &T) -> T
where
    T: serde::Serialize + serde::de::DeserializeOwned,
{
    let bytes = postcard::to_stdvec(value).expect("encode");
    postcard::from_bytes(&bytes).expect("decode")
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 10_000,
        max_shrink_iters: 256,
        .. ProptestConfig::default()
    })]

    #[test]
    fn id_byte_order_matches_numeric_order_file(a in arb_file_id(), b in arb_file_id()) {
        prop_assert_eq!(a.to_bytes().cmp(&b.to_bytes()), a.cmp(&b));
    }

    #[test]
    fn id_byte_order_matches_numeric_order_symbol(
        a in arb_symbol_id(),
        b in arb_symbol_id(),
    ) {
        prop_assert_eq!(a.to_bytes().cmp(&b.to_bytes()), a.cmp(&b));
    }

    #[test]
    fn id_byte_order_matches_numeric_order_edge_key(
        a in arb_edge_key(),
        b in arb_edge_key(),
    ) {
        prop_assert_eq!(a.to_bytes().cmp(&b.to_bytes()), a.cmp(&b));
    }

    #[test]
    fn file_record_codec_roundtrip(rec in arb_file_record()) {
        prop_assert_eq!(postcard_roundtrip::<FileRecord>(&rec), rec);
    }

    #[test]
    fn symbol_record_codec_roundtrip(rec in arb_symbol_record()) {
        prop_assert_eq!(postcard_roundtrip::<SymbolRecord>(&rec), rec);
    }

    #[test]
    fn edge_record_codec_roundtrip(rec in arb_edge_record()) {
        prop_assert_eq!(postcard_roundtrip::<EdgeRecord>(&rec), rec);
    }

    #[test]
    fn edge_key_byte_roundtrip(key in arb_edge_key()) {
        let bytes = key.to_bytes();
        prop_assert_eq!(EdgeKey::from_bytes(&bytes), Some(key));
    }
}

proptest! {
    // Storage roundtrip: smaller case count because each case opens a
    // tempdir + redb file. 256 cases is the proptest default; we leave it
    // there so `cargo nextest run -p ariadne-storage` stays fast while still
    // exercising the full write+read pipeline against real IO.

    #[test]
    fn file_record_storage_roundtrip(
        rec in arb_file_record(),
        id_seed in 1u32..=u32::MAX,
    ) {
        let (storage, _dir) = fresh_storage();
        let id = FileId::new(id_seed).expect("nonzero");
        let cs = Changeset::new().upsert_file(id, rec.clone());
        let txn = storage.begin_write().expect("begin write");
        txn.apply(&cs).expect("apply");
        let snap = storage.snapshot().expect("snapshot");
        prop_assert_eq!(snap.file(id).expect("file"), Some(rec));
    }

    #[test]
    fn symbol_record_storage_roundtrip(
        rec in arb_symbol_record(),
        sid_seed in 1u64..=u64::MAX,
    ) {
        let (storage, _dir) = fresh_storage();
        let sid = SymbolId::new(sid_seed).expect("nonzero");
        let cs = Changeset::new().upsert_symbol(sid, rec.clone());
        let txn = storage.begin_write().expect("begin write");
        txn.apply(&cs).expect("apply");
        let snap = storage.snapshot().expect("snapshot");
        let got = snap.symbols_in_file(rec.defining_file).expect("scan");
        prop_assert_eq!(got, vec![rec]);
    }

    #[test]
    fn edge_record_storage_roundtrip(key in arb_edge_key(), rec in arb_edge_record()) {
        let (storage, _dir) = fresh_storage();
        let cs = Changeset::new().add_edge(key, rec.clone());
        let txn = storage.begin_write().expect("begin write");
        txn.apply(&cs).expect("apply");
        let snap = storage.snapshot().expect("snapshot");
        let out = snap.outgoing_edges(key.src).expect("outgoing");
        prop_assert_eq!(out, vec![(key, rec.clone())]);
        let inn = snap.incoming_edges(key.dst).expect("incoming");
        prop_assert_eq!(inn, vec![(key, rec.clone())]);
        let in_file: Vec<EdgeKey> =
            snap.edges_in_file(rec.source_span.file).expect("in_file");
        prop_assert_eq!(in_file, vec![key]);
    }
}
