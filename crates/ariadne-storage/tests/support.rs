//! Shared test fixtures. `#[allow(dead_code)]` because each integration
//! test binary includes this module via `mod support;` and only uses a
//! subset of the helpers.

#![allow(dead_code, clippy::missing_panics_doc, clippy::must_use_candidate)]

use ariadne_core::{
    EdgeKey, EdgeKind, EdgeRecord, FileId, FileRecord, Lang, Span, SymbolId, SymbolRecord,
};
use ariadne_storage::RedbStorage;
use proptest::prelude::*;
use tempfile::TempDir;

/// Open a fresh redb-backed storage inside a tempdir.
pub fn fresh_storage() -> (RedbStorage, TempDir) {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("index.redb");
    let storage = RedbStorage::open(&path).expect("open redb storage");
    (storage, dir)
}

/// Closed-set `Lang` strategy — avoids the `Other` variant's leak-on-decode
/// to keep 10K-case proptests bounded.
pub fn arb_lang() -> impl Strategy<Value = Lang> {
    prop_oneof![
        Just(Lang::TypeScript),
        Just(Lang::JavaScript),
        Just(Lang::Python),
        Just(Lang::Rust),
        Just(Lang::Go),
        Just(Lang::Java),
        Just(Lang::Kotlin),
        Just(Lang::CSharp),
    ]
}

/// Arbitrary non-zero `FileId`.
pub fn arb_file_id() -> impl Strategy<Value = FileId> {
    (1u32..=u32::MAX).prop_map(|v| FileId::new(v).expect("nonzero"))
}

/// Arbitrary non-zero `SymbolId`.
pub fn arb_symbol_id() -> impl Strategy<Value = SymbolId> {
    (1u64..=u64::MAX).prop_map(|v| SymbolId::new(v).expect("nonzero"))
}

/// Arbitrary half-open `Span` constrained to fit `u32` byte offsets.
pub fn arb_span() -> impl Strategy<Value = Span> {
    (arb_file_id(), 0u32..1_000_000, 0u32..1_000).prop_map(|(file, start, len)| Span {
        file,
        byte_start: start,
        byte_end: start.saturating_add(len),
    })
}

/// Arbitrary `FileRecord`. Path is a short ASCII filename.
pub fn arb_file_record() -> impl Strategy<Value = FileRecord> {
    (
        "[a-zA-Z0-9_./-]{1,64}",
        arb_lang(),
        any::<u64>(),
        any::<[u8; 32]>(),
        any::<i128>(),
    )
        .prop_map(|(path, lang, size, hash, mtime)| FileRecord {
            path,
            lang,
            size,
            blake3: hash,
            mtime_ns: mtime,
        })
}

/// Arbitrary `SymbolRecord`.
pub fn arb_symbol_record() -> impl Strategy<Value = SymbolRecord> {
    (
        "[a-zA-Z0-9_:.]{1,64}",
        "[a-zA-Z]{3,16}",
        arb_file_id(),
        arb_span(),
    )
        .prop_map(
            |(canonical_name, kind, defining_file, defining_span)| SymbolRecord {
                canonical_name,
                kind,
                defining_file,
                defining_span,
            },
        )
}

/// Arbitrary `EdgeKind`.
pub fn arb_edge_kind() -> impl Strategy<Value = EdgeKind> {
    prop_oneof![
        Just(EdgeKind::Defines),
        Just(EdgeKind::References),
        Just(EdgeKind::Imports),
    ]
}

/// Arbitrary `EdgeKey`.
pub fn arb_edge_key() -> impl Strategy<Value = EdgeKey> {
    (arb_symbol_id(), arb_edge_kind(), arb_symbol_id()).prop_map(|(src, kind, dst)| EdgeKey {
        src,
        kind,
        dst,
    })
}

/// Arbitrary `EdgeRecord`.
pub fn arb_edge_record() -> impl Strategy<Value = EdgeRecord> {
    (arb_span(), arb_lang(), any::<u32>()).prop_map(|(source_span, evidence_lang, weight)| {
        EdgeRecord {
            source_span,
            evidence_lang,
            weight,
        }
    })
}
