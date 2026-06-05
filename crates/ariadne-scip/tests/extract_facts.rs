//! scip-driven-edges tier-01: `extract_facts` reduces a SCIP ingest to the
//! per-file edge signal (normalized symbol key, byte range, roles) plus the
//! indexed content hash. Two source-text paths are exercised: a document that
//! embeds its `text`, and a document whose text is read from disk via the index
//! `project_root` + `relative_path` (the path a real indexer like rust-analyzer
//! takes, since it omits `text` by default).

use ariadne_core::Lang;
use ariadne_scip::indexer::{IngestReport, ScipDoc};
use ariadne_scip::{extract_facts, normalize_scip_symbol, proto};

/// SCIP `SymbolRole::Definition` [src: crates/ariadne-scip/proto/scip.proto:526].
const DEFINITION: i32 = 0x1;

const RAW_SYMBOL: &str = "scip-rust cargo demo 1.0 lib/connect().";
const SOURCE: &str = "fn connect() {}\n    connect();\n";

/// Build a one-document SCIP index. When `embed_text` the document carries its
/// `text`; otherwise `text` is empty and consumers must read `project_root`.
fn one_doc_index(project_root: &str, embed_text: bool) -> proto::Index {
    let occurrences = vec![
        proto::Occurrence {
            range: vec![0, 3, 0, 10],
            symbol: RAW_SYMBOL.to_owned(),
            symbol_roles: DEFINITION,
            ..Default::default()
        },
        proto::Occurrence {
            range: vec![1, 4, 1, 11],
            symbol: RAW_SYMBOL.to_owned(),
            symbol_roles: 0,
            ..Default::default()
        },
    ];
    proto::Index {
        metadata: Some(proto::Metadata {
            version: proto::ProtocolVersion::UnspecifiedProtocolVersion as i32,
            tool_info: None,
            project_root: project_root.to_owned(),
            text_document_encoding: proto::TextEncoding::Utf8 as i32,
        }),
        documents: vec![proto::Document {
            language: "Rust".to_owned(),
            relative_path: "src/lib.rs".to_owned(),
            occurrences,
            symbols: Vec::new(),
            text: if embed_text {
                SOURCE.to_owned()
            } else {
                String::new()
            },
            position_encoding: proto::PositionEncoding::Utf8CodeUnitOffsetFromLineStart as i32,
        }],
        external_symbols: Vec::new(),
    }
}

fn report_for(index: proto::Index) -> IngestReport {
    IngestReport {
        docs: vec![ScipDoc {
            lang: Lang::Rust,
            index,
        }],
        ..Default::default()
    }
}

/// The expected normalized key both occurrences share.
fn expected_key() -> String {
    normalize_scip_symbol(RAW_SYMBOL).unwrap().id().to_hex()
}

#[test]
fn extracts_byte_ranges_roles_and_hash_from_embedded_text() {
    let report = report_for(one_doc_index("file:///synth", true));
    let facts = extract_facts(&report);

    assert_eq!(facts.len(), 1, "one document => one facts entry");
    let (path, scip) = &facts[0];
    assert_eq!(path, "src/lib.rs");

    // Both occurrences normalize to the same global key.
    let key = expected_key();
    assert_eq!(scip.occurrences.len(), 2);
    assert_eq!(scip.occurrences[0].symbol, key);
    assert_eq!(scip.occurrences[1].symbol, key);

    // Line/character ranges converted to byte ranges over the UTF-8 source.
    assert_eq!(scip.occurrences[0].byte_range, (3, 10));
    assert_eq!(scip.occurrences[0].roles, DEFINITION as u32);
    assert_eq!(scip.occurrences[1].byte_range, (20, 27));
    assert_eq!(scip.occurrences[1].roles, 0);

    // The indexed hash is blake3 of the converted source — the D4 coverage key.
    assert_eq!(
        scip.indexed_hash,
        *blake3::hash(SOURCE.as_bytes()).as_bytes()
    );
}

#[test]
fn reads_source_from_disk_when_text_is_absent() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let src_dir = tmp.path().join("src");
    std::fs::create_dir_all(&src_dir).expect("create src dir");
    std::fs::write(src_dir.join("lib.rs"), SOURCE).expect("write source");

    let root_uri = format!("file://{}", tmp.path().display());
    let report = report_for(one_doc_index(&root_uri, false));
    let facts = extract_facts(&report);

    assert_eq!(facts.len(), 1, "disk-read document still yields facts");
    let (_, scip) = &facts[0];
    // Identical result to the embedded-text path: the bytes are the same.
    assert_eq!(scip.occurrences.len(), 2);
    assert_eq!(scip.occurrences[0].byte_range, (3, 10));
    assert_eq!(scip.occurrences[1].byte_range, (20, 27));
    assert_eq!(scip.occurrences[0].symbol, expected_key());
    assert_eq!(
        scip.indexed_hash,
        *blake3::hash(SOURCE.as_bytes()).as_bytes()
    );
}

#[test]
fn document_with_no_resolvable_text_is_skipped() {
    // No embedded text and a project_root that does not exist on disk: the
    // document is dropped rather than producing wrong byte ranges (plan D4).
    let report = report_for(one_doc_index("file:///nonexistent-ariadne-root", false));
    assert!(
        extract_facts(&report).is_empty(),
        "an unresolvable document must be skipped, not mis-converted",
    );
}
