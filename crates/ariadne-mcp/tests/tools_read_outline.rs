//! Tier-02 (context-efficient-read) step 1 — `read_outline` handler over a
//! temp-dir fixture.
//!
//! Writes a real multi-symbol source file under the project root, seeds a
//! redb-backed `.ariadne/index.redb` whose symbol spans point into it, builds
//! the cold [`Catalog`] the production tool reads, and drives
//! `tools::read_outline::handle` directly. Asserts: the folded skeleton keeps
//! every signature and leading doc comment, folds each multi-line body to a
//! marker, drops a private symbol when `include_private=false`, and is strictly
//! smaller than a whole-file read; a file truncated after indexing flags
//! `stale` (clamped, no panic); and a file with no indexed symbols returns a
//! line-count note instead of dumping the source. A timing loop asserts the
//! query SLO (outline p95 <100ms) against a repo-scale fixture — a multi-symbol
//! file inside a repo-scale catalog — so the assertion is load-bearing. An
//! over-stdio run exercises the registered
//! `#[tool]` end-to-end through the real server and its lazy cold catalog.

use std::fmt::Write as _;
use std::path::Path;
use std::time::{Duration, Instant};

use ariadne_core::{
    Changeset, FileId, FileRecord, Lang, Span, Storage, SymbolId, SymbolRecord, Visibility,
    WriteTxn,
};
use ariadne_mcp::Catalog;
use ariadne_mcp::tools::read_outline;
use ariadne_mcp::types::ReadOutlineInput;
use ariadne_storage::RedbStorage;
use tempfile::TempDir;

mod support;

/// Multi-symbol Rust fixture: a public `add` (multi-line body), a *private*
/// `secret` (multi-line body), and a public `LIMIT` constant — each with a
/// leading doc comment. Bodies are >2 lines so the assembler folds them.
const DEMO: &str = "\
//! demo crate\n\
\n\
/// Adds two numbers.\n\
pub fn add(a: i64, b: i64) -> i64 {\n\
    let s = a + b;\n\
    let t = s;\n\
    t\n\
}\n\
\n\
/// A private helper, hidden when include_private is false.\n\
fn secret() -> i64 {\n\
    let x = 1;\n\
    let y = 2;\n\
    x + y\n\
}\n\
\n\
/// The public limit.\n\
pub const LIMIT: i64 = 100;\n\
";

/// A second file that is indexed (has a `FileRecord`) but owns no symbols,
/// driving the zero-symbol note branch.
const EMPTY: &str = "// just a comment\nlet x = 1;\n// no symbols here\n";

fn fid(n: u32) -> FileId {
    FileId::new(n).expect("nonzero file id")
}

fn sid(n: u64) -> SymbolId {
    SymbolId::new(n).expect("nonzero symbol id")
}

/// `(byte_start, byte_end)` of `add` — `pub fn add` to just past its closing
/// brace (the `}` preceding the private-helper doc comment).
fn add_span() -> (u32, u32) {
    let start = u32::try_from(DEMO.find("pub fn add").expect("add decl")).expect("fits u32");
    let end =
        u32::try_from(DEMO.find("}\n\n/// A private").expect("add close") + 1).expect("fits u32");
    (start, end)
}

/// `(byte_start, byte_end)` of the private `secret`.
fn secret_span() -> (u32, u32) {
    let start = u32::try_from(DEMO.find("fn secret").expect("secret decl")).expect("fits u32");
    let end = u32::try_from(DEMO.find("}\n\n/// The public").expect("secret close") + 1)
        .expect("fits u32");
    (start, end)
}

/// `(byte_start, byte_end)` of the `LIMIT` constant — to just past its `;`.
fn limit_span() -> (u32, u32) {
    let start = u32::try_from(DEMO.find("pub const LIMIT").expect("limit decl")).expect("fits u32");
    let end =
        u32::try_from(DEMO.find("100;").expect("limit end") + "100;".len()).expect("fits u32");
    (start, end)
}

/// Write the fixture files under `root` and seed a redb index. `src/demo.rs`
/// carries three symbols (public `add`, private `secret`, public `LIMIT`);
/// `src/empty.rs` is indexed as a file but owns no symbols.
fn seed(root: &Path) {
    std::fs::create_dir_all(root.join("src")).expect("mkdir src");
    std::fs::write(root.join("src/demo.rs"), DEMO).expect("write demo");
    std::fs::write(root.join("src/empty.rs"), EMPTY).expect("write empty");

    let storage = RedbStorage::open(&root.join(".ariadne").join("index.redb")).expect("open redb");
    let mut cs = Changeset::new();
    for (id, path) in [(1u32, "src/demo.rs"), (2, "src/empty.rs")] {
        cs = cs.upsert_file(
            fid(id),
            FileRecord {
                path: path.into(),
                lang: Lang::Rust,
                size: 256,
                blake3: [u8::try_from(id).expect("file id fits u8"); 32],
                mtime_ns: i128::from(id),
            },
        );
    }
    let (as_, ae) = add_span();
    let (ss, se) = secret_span();
    let (ls, le) = limit_span();
    let symbols = [
        (1u64, "add", as_, ae, Visibility::Public),
        (2, "secret", ss, se, Visibility::Private),
        (3, "LIMIT", ls, le, Visibility::Public),
    ];
    for (id, name, byte_start, byte_end, visibility) in symbols {
        cs = cs.upsert_symbol(
            sid(id),
            SymbolRecord {
                canonical_name: name.into(),
                kind: "function".into(),
                defining_file: fid(1),
                defining_span: Span {
                    file: fid(1),
                    byte_start,
                    byte_end,
                },
                visibility,
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
/// tool reads. Returns the catalog and the tempdir guard.
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

/// Symbols in the generated repo-scale fixture file. Chosen to exceed a real
/// multi-symbol module (e.g. `server.rs` indexes ~60 symbols) so the p95 timing
/// exercises the assembler's per-symbol work (`parent_of` is O(n²), `line_index`
/// O(n·bytes)), not a 3-symbol toy (INFO-2).
const BIG_SYMS: usize = 160;
/// Padding symbols seeded on an auxiliary file so the cold catalog the timed
/// `handle` scans is repo-scale (this repo indexes ~3.9k symbols), making the
/// catalog-wide symbol filter loop load-bearing too.
const PAD_SYMS: u64 = 3_900;

/// Build a large, deterministic multi-symbol Rust source plus each symbol's
/// `(name, byte_start, byte_end, visibility)`. Every function carries a leading
/// doc comment and a multi-line body so the assembler folds it; every third
/// symbol is private, exercising the visibility filter at scale. `byte_start`
/// points at the declaration (the doc comment is derived above it); `byte_end`
/// is one past the closing brace, matching the hand-written fixture spans.
fn big_source() -> (String, Vec<(String, u32, u32, Visibility)>) {
    let mut src = String::from("//! generated repo-scale fixture\n\n");
    let mut spans: Vec<(String, u32, u32, Visibility)> = Vec::with_capacity(BIG_SYMS);
    for i in 0..BIG_SYMS {
        writeln!(src, "/// Function number {i}.").expect("write doc");
        let private = i % 3 == 0;
        let visibility = if private {
            Visibility::Private
        } else {
            Visibility::Public
        };
        let kw = if private { "fn" } else { "pub fn" };
        let name = format!("func_{i}");
        let start = u32::try_from(src.len()).expect("decl start fits u32");
        writeln!(src, "{kw} {name}(a: i64, b: i64) -> i64 {{").expect("write header");
        src.push_str("    let x0 = a + b;\n");
        src.push_str("    let x1 = x0 * 2;\n");
        src.push_str("    let x2 = x1 - 1;\n");
        src.push_str("    let x3 = x2 + a;\n");
        src.push_str("    let x4 = x3 * b;\n");
        src.push_str("    let x5 = x4 - a;\n");
        src.push_str("    let x6 = x5 + 1;\n");
        src.push_str("    x6\n");
        src.push('}');
        let end = u32::try_from(src.len()).expect("decl end fits u32");
        src.push_str("\n\n");
        spans.push((name, start, end, visibility));
    }
    (src, spans)
}

/// Write the repo-scale fixture under `root` and seed a redb index whose
/// `src/big.rs` carries [`BIG_SYMS`] symbols and whose `src/pad.rs` carries
/// [`PAD_SYMS`] padding symbols, so both the assembler and the catalog scan run
/// at repo scale.
fn seed_large(root: &Path) {
    std::fs::create_dir_all(root.join("src")).expect("mkdir src");
    let (src, spans) = big_source();
    std::fs::write(root.join("src/big.rs"), &src).expect("write big");
    std::fs::write(root.join("src/pad.rs"), "// padding file\n").expect("write pad");

    let storage = RedbStorage::open(&root.join(".ariadne").join("index.redb")).expect("open redb");
    let mut cs = Changeset::new();
    for (id, path) in [(1u32, "src/big.rs"), (2, "src/pad.rs")] {
        cs = cs.upsert_file(
            fid(id),
            FileRecord {
                path: path.into(),
                lang: Lang::Rust,
                size: 256,
                blake3: [u8::try_from(id).expect("file id fits u8"); 32],
                mtime_ns: i128::from(id),
            },
        );
    }
    for (i, (name, byte_start, byte_end, visibility)) in spans.iter().enumerate() {
        let id = u64::try_from(i + 1).expect("symbol id fits u64");
        cs = cs.upsert_symbol(
            sid(id),
            SymbolRecord {
                canonical_name: name.clone(),
                kind: "function".into(),
                defining_file: fid(1),
                defining_span: Span {
                    file: fid(1),
                    byte_start: *byte_start,
                    byte_end: *byte_end,
                },
                visibility: *visibility,
                attributes: Vec::new(),
                complexity: 0,
            },
        );
    }
    let base = u64::try_from(spans.len()).expect("base id fits u64") + 1;
    for k in 0..PAD_SYMS {
        cs = cs.upsert_symbol(
            sid(base + k),
            SymbolRecord {
                canonical_name: format!("pad_{k}"),
                kind: "function".into(),
                defining_file: fid(2),
                defining_span: Span {
                    file: fid(2),
                    byte_start: 0,
                    byte_end: 1,
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

/// Seed the repo-scale fixture under a fresh tempdir and build the cold
/// [`Catalog`] the production tool reads.
fn large_fixture() -> (Catalog, TempDir) {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().to_path_buf();
    seed_large(&root);
    let storage =
        RedbStorage::open(&root.join(".ariadne").join("index.redb")).expect("reopen redb");
    let catalog =
        Catalog::build(&storage, root.to_string_lossy().into_owned()).expect("build catalog");
    drop(storage);
    (catalog, dir)
}

fn input(path: &str, include_private: Option<bool>) -> ReadOutlineInput {
    ReadOutlineInput {
        path: path.into(),
        include_private,
    }
}

#[test]
fn read_outline_folds_bodies_and_keeps_signatures_and_docs() {
    let (cat, g) = fixture();
    let out = read_outline::handle(&cat, &input("src/demo.rs", None)).expect("outline ok");

    // Signatures and leading doc comments survive byte-faithfully.
    assert!(
        out.skeleton.contains("pub fn add(a: i64, b: i64) -> i64"),
        "signature kept: {:?}",
        out.skeleton
    );
    assert!(
        out.skeleton.contains("/// Adds two numbers."),
        "doc comment kept: {:?}",
        out.skeleton
    );
    assert!(
        out.skeleton.contains("pub const LIMIT: i64 = 100;"),
        "short const kept verbatim: {:?}",
        out.skeleton
    );
    // Bodies fold to a marker carrying the elided-line count, not the source.
    assert!(
        out.skeleton.contains("4 lines"),
        "body folded to a line-count marker: {:?}",
        out.skeleton
    );
    assert!(
        !out.skeleton.contains("let s = a + b;"),
        "folded body text must be elided: {:?}",
        out.skeleton
    );

    // Default include_private (None → true) keeps the private symbol.
    assert!(
        out.skeleton.contains("fn secret"),
        "private kept by default"
    );
    assert_eq!(out.symbols.len(), 3, "three symbols indexed");
    assert!(out.symbols.iter().any(|e| e.name == "add" && e.has_body));
    assert!(out.symbols.iter().any(|e| e.name == "LIMIT"));

    // The skeleton is strictly smaller than a whole-file read.
    let raw = std::fs::read(g.path().join("src/demo.rs")).expect("read demo");
    assert!(
        out.skeleton.len() < raw.len(),
        "skeleton ({}) must be smaller than the whole file ({})",
        out.skeleton.len(),
        raw.len(),
    );
    assert!(!out.stale, "intact file is not stale");
    assert!(out.note.is_none(), "indexed file carries no note");
    assert_eq!(out.revision, cat.revision);
    // Every source line is accounted for as kept or elided.
    let total_lines = u32::try_from(DEMO.lines().count()).expect("fits u32");
    assert_eq!(out.kept_lines + out.elided_lines, total_lines);
}

#[test]
fn read_outline_include_private_false_drops_private_symbols() {
    let (cat, _g) = fixture();
    let out = read_outline::handle(&cat, &input("src/demo.rs", Some(false))).expect("outline ok");

    assert!(
        !out.skeleton.contains("fn secret"),
        "private symbol must be elided: {:?}",
        out.skeleton
    );
    assert!(out.skeleton.contains("pub fn add"), "public kept");
    assert!(out.skeleton.contains("pub const LIMIT"), "public kept");
    assert_eq!(out.symbols.len(), 2, "only the two public symbols indexed");
    assert!(out.symbols.iter().all(|e| e.name != "secret"));
}

#[test]
fn read_outline_truncated_file_is_flagged_stale_and_does_not_panic() {
    let (cat, dir) = fixture();
    let (_as, add_end) = add_span();
    // Truncate just past `add`'s body, after indexing recorded the later
    // symbols' full spans — their `byte_end` now runs past EOF.
    let trunc_len = add_end as usize;
    std::fs::write(
        dir.path().join("src/demo.rs"),
        &DEMO.as_bytes()[..trunc_len],
    )
    .expect("truncate demo");

    // Must not panic or fabricate bytes (R5).
    let out = read_outline::handle(&cat, &input("src/demo.rs", None))
        .expect("outline ok despite stale spans");
    assert!(out.stale, "out-of-range spans must flag stale");
    assert!(
        out.skeleton.contains("pub fn add"),
        "the surviving symbol still renders: {:?}",
        out.skeleton
    );
}

#[test]
fn read_outline_zero_symbol_file_returns_a_line_count_note() {
    let (cat, _g) = fixture();
    let out = read_outline::handle(&cat, &input("src/empty.rs", None)).expect("outline ok");

    let note = out
        .note
        .as_deref()
        .expect("zero-symbol file carries a note");
    assert!(
        note.contains("3 lines"),
        "note carries the line count: {note:?}",
    );
    assert!(
        note.contains("no indexed symbols"),
        "note advises a native read: {note:?}",
    );
    assert!(out.skeleton.is_empty(), "note branch never dumps the file");
    assert!(out.symbols.is_empty());
    assert_eq!(out.kept_lines, 0);
    assert_eq!(out.elided_lines, 0);
}

#[test]
fn read_outline_unindexed_path_is_a_typed_error_not_a_panic() {
    let (cat, _g) = fixture();
    assert!(read_outline::handle(&cat, &input("src/missing.rs", None)).is_err());
}

#[test]
fn read_outline_p95_under_100ms_on_repo_scale_file() {
    let (cat, g) = large_fixture();

    // The SLO assertion is only load-bearing if the timed target is genuinely
    // repo-scale: a multi-symbol file (more symbols than a real module such as
    // server.rs) within a repo-scale catalog. Pin those preconditions so the
    // test can never silently regress to a toy fixture (INFO-2).
    let raw = std::fs::read(g.path().join("src/big.rs")).expect("read big");
    assert!(
        raw.len() > 20_000,
        "fixture must be repo-scale ({} bytes)",
        raw.len(),
    );
    assert!(
        cat.symbols.len() >= 3_000,
        "catalog must be repo-scale ({} symbols)",
        cat.symbols.len(),
    );
    let probe = read_outline::handle(&cat, &input("src/big.rs", None)).expect("outline ok");
    assert!(
        probe.symbols.len() >= 60,
        "target file must be multi-symbol ({} symbols kept)",
        probe.symbols.len(),
    );

    let mut samples: Vec<Duration> = Vec::with_capacity(100);
    for _ in 0..100 {
        let start = Instant::now();
        let _ = read_outline::handle(&cat, &input("src/big.rs", None)).expect("outline ok");
        samples.push(start.elapsed());
    }
    samples.sort_unstable();
    let p95 = samples[94];
    assert!(
        p95 < Duration::from_millis(100),
        "outline p95 {p95:?} exceeds the 100ms query SLO on a repo-scale file",
    );
}

/// End-to-end over the rmcp stdio transport: spawn the `ariadne-mcp` binary
/// against a seeded project, call the registered `read_outline` tool, and
/// assert the wire JSON folds bodies, keeps signatures, and is smaller than a
/// whole-file read — the tier-02 `<verification>` real run, through the real
/// server and its lazily-built cold catalog. A second call with
/// `include_private=false` drops the private symbol over the wire.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn read_outline_over_stdio_folds_and_honors_private_filter() {
    use rmcp::model::CallToolRequestParams;
    use rmcp::object;

    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().to_path_buf();
    seed(&root);
    let client = support::spawn_client(&root).await;

    let resp = client
        .call_tool(
            CallToolRequestParams::new("read_outline").with_arguments(object!({
                "path": "src/demo.rs",
            })),
        )
        .await
        .expect("read_outline call");
    let outline: serde_json::Value =
        serde_json::from_str(&support::extract_text(&resp)).expect("decode outline");
    let skeleton = outline["skeleton"].as_str().expect("skeleton string");
    assert!(
        skeleton.contains("pub fn add(a: i64, b: i64) -> i64") && skeleton.contains("4 lines"),
        "stdio skeleton folds the body but keeps the signature: {skeleton:?}",
    );
    assert!(skeleton.contains("fn secret"), "private kept by default");
    assert!(
        skeleton.len() < DEMO.len(),
        "stdio skeleton ({}) smaller than the whole file ({})",
        skeleton.len(),
        DEMO.len(),
    );
    assert_eq!(outline["stale"], serde_json::Value::Bool(false));

    let private_off = client
        .call_tool(
            CallToolRequestParams::new("read_outline").with_arguments(object!({
                "path": "src/demo.rs",
                "include_private": false,
            })),
        )
        .await
        .expect("read_outline private-off call");
    let filtered: serde_json::Value =
        serde_json::from_str(&support::extract_text(&private_off)).expect("decode filtered");
    assert!(
        !filtered["skeleton"]
            .as_str()
            .expect("skeleton string")
            .contains("fn secret"),
        "include_private=false drops the private symbol over the wire: {filtered}",
    );

    client.cancel().await.ok();
}
