//! Tier-08 step 1 — `read_symbol` handler behaviour over a temp-dir fixture.
//!
//! Writes two real source files under the project root, seeds a
//! redb-backed `.ariadne/index.redb` whose symbol spans point into them,
//! builds the cold [`Catalog`] the production tool reads, and drives
//! `tools::read_symbol::handle` directly. Asserts: `full` mode returns
//! bytes equal to the on-disk slice `[byte_start, byte_end]`; `signature`
//! mode returns the declaration line; `context` mode widens by ±N lines;
//! the 1-based line range is correct; `file` disambiguates an overloaded
//! name; and a file truncated after indexing yields `stale: true` with a
//! clamped slice and no panic (R7).

use std::path::Path;

use ariadne_core::{
    Changeset, FileId, FileRecord, Lang, Span, Storage, SymbolId, SymbolRecord, Visibility,
    WriteTxn,
};
use ariadne_mcp::Catalog;
use ariadne_mcp::tools::read_symbol;
use ariadne_mcp::types::ReadSymbolInput;
use ariadne_storage::RedbStorage;
use tempfile::TempDir;

mod support;

/// `greet` lives on lines 2–4; line 1 is a header comment, lines 5–6 trail.
const DEMO: &str = "// a header line\nfn greet(name: &str) -> String {\n    String::from(\"hi\")\n}\n\nfn tail() {}\n";
/// A second `greet` in another file, to drive `file` disambiguation.
const OTHER: &str = "// other file\nfn greet() -> u8 {\n    7\n}\n";

fn fid(n: u32) -> FileId {
    FileId::new(n).expect("nonzero file id")
}

fn sid(n: u64) -> SymbolId {
    SymbolId::new(n).expect("nonzero symbol id")
}

/// Byte span of `greet` in [`DEMO`] — from `fn greet` to just past its
/// closing brace (the first `}` in the file).
fn demo_span() -> (u32, u32) {
    let start = u32::try_from(DEMO.find("fn greet").expect("greet decl")).expect("start fits u32");
    let end = u32::try_from(DEMO.find('}').expect("greet close") + 1).expect("end fits u32");
    (start, end)
}

/// Byte span of `greet` in [`OTHER`].
fn other_span() -> (u32, u32) {
    let start = u32::try_from(OTHER.find("fn greet").expect("greet decl")).expect("start fits u32");
    let end = u32::try_from(OTHER.find('}').expect("greet close") + 1).expect("end fits u32");
    (start, end)
}

/// Write the two source files under `root` and seed a redb index whose two
/// `greet` symbols span into them. Shared by the in-process catalog fixture
/// and the over-stdio real-run test so both read identical bytes.
fn seed(root: &Path) {
    std::fs::create_dir_all(root.join("src")).expect("mkdir src");
    std::fs::write(root.join("src/demo.rs"), DEMO).expect("write demo");
    std::fs::write(root.join("src/other.rs"), OTHER).expect("write other");

    let storage_path = root.join(".ariadne").join("index.redb");
    let storage = RedbStorage::open(&storage_path).expect("open redb");

    let mut cs = Changeset::new();
    for (id, path) in [(1u32, "src/demo.rs"), (2, "src/other.rs")] {
        cs = cs.upsert_file(
            fid(id),
            FileRecord {
                path: path.into(),
                lang: Lang::Rust,
                size: 128,
                blake3: [u8::try_from(id).expect("file id fits u8"); 32],
                mtime_ns: i128::from(id),
            },
        );
    }
    let (ds, de) = demo_span();
    let (os, oe) = other_span();
    let symbols = [(1u64, 1u32, ds, de), (2, 2, os, oe)];
    for (id, file, byte_start, byte_end) in symbols {
        cs = cs.upsert_symbol(
            sid(id),
            SymbolRecord {
                canonical_name: "greet".into(),
                kind: "function".into(),
                defining_file: fid(file),
                defining_span: Span {
                    file: fid(file),
                    byte_start,
                    byte_end,
                },
                visibility: Visibility::Public,
                attributes: Vec::new(),
                complexity: 0,
            },
        );
    }
    let txn = storage.begin_write().expect("begin");
    txn.apply(&cs).expect("apply changeset");
    drop(storage);
}

/// Seed under a fresh tempdir and build the cold [`Catalog`] the production
/// tool reads. Returns the catalog and the tempdir guard (kept alive for the
/// catalog's lifetime + any file mutation).
fn fixture() -> (Catalog, TempDir) {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().to_path_buf();
    seed(&root);
    let storage =
        RedbStorage::open(&root.join(".ariadne").join("index.redb")).expect("reopen redb");
    let catalog =
        Catalog::build(&storage, root.to_string_lossy().into_owned()).expect("build catalog");
    drop(storage);
    (catalog, dir)
}

fn input(symbol: &str) -> ReadSymbolInput {
    ReadSymbolInput {
        symbol: symbol.into(),
        file: None,
        mode: None,
        context_lines: None,
    }
}

#[test]
fn read_symbol_full_mode_returns_the_exact_on_disk_slice() {
    let (cat, _g) = fixture();
    let (start, end) = demo_span();
    let mut i = input("greet");
    i.mode = Some("full".into());
    let slice = read_symbol::handle(&cat, &i).expect("read ok");

    assert_eq!(
        slice.source.as_bytes(),
        &DEMO.as_bytes()[start as usize..end as usize],
        "full slice must equal the on-disk bytes",
    );
    assert_eq!(slice.name, "greet");
    assert_eq!(slice.file, "src/demo.rs");
    assert_eq!(slice.byte_start, start);
    assert_eq!(slice.byte_end, end);
    assert_eq!(slice.line_start, 2);
    assert_eq!(slice.line_end, 4);
    assert!(!slice.stale);
    assert_eq!(slice.revision, cat.revision);
}

#[test]
fn read_symbol_signature_mode_returns_the_declaration_line() {
    let (cat, _g) = fixture();
    let mut i = input("greet");
    i.mode = Some("signature".into());
    i.file = Some("src/demo.rs".into());
    let slice = read_symbol::handle(&cat, &i).expect("read ok");

    assert!(
        slice.source.contains("fn greet(name: &str) -> String"),
        "signature must carry the full declaration: {:?}",
        slice.source,
    );
    assert!(
        !slice.source.contains('{'),
        "signature must stop before the body brace: {:?}",
        slice.source,
    );
    assert_eq!(slice.line_start, 2);
    assert_eq!(slice.line_end, 2);
}

#[test]
fn read_symbol_context_mode_widens_by_n_surrounding_lines() {
    let (cat, _g) = fixture();
    let mut i = input("greet");
    i.mode = Some("context".into());
    i.context_lines = Some(1);
    i.file = Some("src/demo.rs".into());
    let slice = read_symbol::handle(&cat, &i).expect("read ok");

    assert!(
        slice.source.contains("// a header line"),
        "context must include the line above: {:?}",
        slice.source,
    );
    assert!(
        slice.source.contains("String::from"),
        "context must include the body: {:?}",
        slice.source,
    );
    assert_eq!(slice.line_start, 1, "±1 lines reaches line 1");
}

#[test]
fn read_symbol_file_disambiguates_an_overloaded_symbol() {
    let (cat, _g) = fixture();
    let (start, end) = other_span();
    let mut i = input("greet");
    i.mode = Some("full".into());
    i.file = Some("src/other.rs".into());
    let slice = read_symbol::handle(&cat, &i).expect("read ok");

    assert_eq!(slice.file, "src/other.rs");
    assert_eq!(
        slice.source.as_bytes(),
        &OTHER.as_bytes()[start as usize..end as usize],
    );
}

#[test]
fn read_symbol_truncated_file_is_flagged_stale_and_clamped() {
    let (cat, dir) = fixture();
    let (start, end) = demo_span();
    // Truncate the file mid-symbol, after indexing recorded the full span.
    let trunc_len = start as usize + 5;
    assert!(
        trunc_len < end as usize,
        "truncation must cut the span short"
    );
    std::fs::write(
        dir.path().join("src/demo.rs"),
        &DEMO.as_bytes()[..trunc_len],
    )
    .expect("truncate demo");

    let mut i = input("greet");
    i.mode = Some("full".into());
    i.file = Some("src/demo.rs".into());
    // Must not panic or fabricate bytes (R7).
    let slice = read_symbol::handle(&cat, &i).expect("read ok despite stale span");

    assert!(slice.stale, "out-of-range span must flag stale");
    assert_eq!(
        slice.byte_end,
        u32::try_from(trunc_len).expect("len fits u32"),
        "end clamped to file length"
    );
    assert_eq!(
        slice.source.as_bytes(),
        &DEMO.as_bytes()[start as usize..trunc_len],
        "stale slice serves only the bytes that still exist",
    );
}

#[test]
fn read_symbol_ambiguous_name_without_file_lists_alternatives() {
    let (cat, _g) = fixture();
    // Two `greet` defs (demo.rs, other.rs); with no `file` the resolver picks
    // the first and must surface the other so the caller knows overloads
    // existed and can re-query with `file` (tier-08 step 4 "+ note"; INFO-1).
    let mut i = input("greet");
    i.mode = Some("full".into());
    let slice = read_symbol::handle(&cat, &i).expect("read ok");

    assert_eq!(
        slice.alternatives.len(),
        1,
        "the not-picked `greet` def must be surfaced: {:?}",
        slice.alternatives,
    );
    // The chosen `file` plus the single alternative cover both defining files,
    // regardless of which the resolver happened to pick first.
    let mut all = slice.alternatives.clone();
    all.push(slice.file.clone());
    all.sort();
    assert_eq!(
        all,
        vec!["src/demo.rs".to_owned(), "src/other.rs".to_owned()],
        "alternatives + file must cover every defining site",
    );
}

#[test]
fn read_symbol_with_file_has_no_alternatives() {
    let (cat, _g) = fixture();
    let mut i = input("greet");
    i.mode = Some("full".into());
    i.file = Some("src/other.rs".into());
    let slice = read_symbol::handle(&cat, &i).expect("read ok");

    assert!(
        slice.alternatives.is_empty(),
        "a pinned `file` resolves unambiguously, so no alternatives: {:?}",
        slice.alternatives,
    );
}

#[test]
fn read_symbol_unknown_symbol_is_a_typed_error_not_a_panic() {
    let (cat, _g) = fixture();
    let i = input("does_not_exist");
    assert!(read_symbol::handle(&cat, &i).is_err());
}

/// End-to-end over the rmcp stdio transport: spawn the `ariadne-mcp` binary
/// against a seeded project, call the registered `read_symbol` tool in `full`
/// and `signature` modes, and assert the returned source matches the on-disk
/// bytes — the tier-08 `<verification>` real run, through the real server and
/// its lazily-built cold catalog.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn read_symbol_over_stdio_modes_and_ambiguity() {
    use rmcp::model::CallToolRequestParams;
    use rmcp::object;

    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().to_path_buf();
    seed(&root);
    let client = support::spawn_client(&root).await;

    // `full` mode: bytes must equal the on-disk slice of `greet` in demo.rs.
    let full = client
        .call_tool(
            CallToolRequestParams::new("read_symbol").with_arguments(object!({
                "symbol": "greet",
                "file": "src/demo.rs",
                "mode": "full",
            })),
        )
        .await
        .expect("read_symbol full call");
    let slice: serde_json::Value =
        serde_json::from_str(&support::extract_text(&full)).expect("decode full");
    let (start, end) = demo_span();
    assert_eq!(
        slice["source"].as_str().expect("source string").as_bytes(),
        &DEMO.as_bytes()[start as usize..end as usize],
        "stdio full slice must equal the on-disk bytes",
    );
    assert_eq!(slice["file"], "src/demo.rs");
    assert_eq!(slice["stale"], serde_json::Value::Bool(false));

    // `signature` mode: the declaration line, no body brace.
    let sig = client
        .call_tool(
            CallToolRequestParams::new("read_symbol").with_arguments(object!({
                "symbol": "greet",
                "file": "src/demo.rs",
                "mode": "signature",
            })),
        )
        .await
        .expect("read_symbol signature call");
    let sig_slice: serde_json::Value =
        serde_json::from_str(&support::extract_text(&sig)).expect("decode signature");
    let sig_src = sig_slice["source"].as_str().expect("source string");
    assert!(
        sig_src.contains("fn greet(name: &str) -> String") && !sig_src.contains('{'),
        "stdio signature must be the declaration line: {sig_src:?}",
    );

    // Ambiguous call: two `greet` defs and no `file`. The wire JSON must carry
    // `alternatives` listing the other defining file so the caller can pin one
    // (tier-08 step 4 "+ note"; validates the serialized shape end-to-end).
    let ambiguous = client
        .call_tool(
            CallToolRequestParams::new("read_symbol").with_arguments(object!({
                "symbol": "greet",
            })),
        )
        .await
        .expect("read_symbol ambiguous call");
    let amb_slice: serde_json::Value =
        serde_json::from_str(&support::extract_text(&ambiguous)).expect("decode ambiguous");
    let alts: Vec<&str> = amb_slice["alternatives"]
        .as_array()
        .expect("alternatives array present on the wire")
        .iter()
        .map(|v| v.as_str().expect("alternative path string"))
        .collect();
    let picked = amb_slice["file"].as_str().expect("file string");
    let mut covered: Vec<&str> = alts.clone();
    covered.push(picked);
    covered.sort_unstable();
    assert_eq!(
        covered,
        vec!["src/demo.rs", "src/other.rs"],
        "alternatives + file must cover both `greet` defs over the wire: alts={alts:?} picked={picked:?}",
    );

    client.cancel().await.ok();
}
