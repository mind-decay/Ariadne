//! Tier-07 step 1 — `search_code` handler behaviour over a fixture catalog.
//!
//! Seeds a redb-backed `.ariadne/index.redb` with symbols of known
//! names / kinds / langs / paths / visibilities, builds the cold
//! [`Catalog`] the production tool reads, and drives
//! `tools::search_code::handle` directly. Asserts the substring default,
//! the `regex` name match, the `path` glob, the `kind` / `lang` /
//! `visibility` filters, the `limit` cap, and that an invalid regex or
//! glob returns a typed `Err` (never a panic).

use ariadne_core::{
    Changeset, FileId, FileRecord, Lang, Span, Storage, SymbolId, SymbolRecord, Visibility,
    WriteTxn,
};
use ariadne_mcp::Catalog;
use ariadne_mcp::tools::search_code;
use ariadne_mcp::types::SearchCodeInput;
use ariadne_storage::RedbStorage;
use tempfile::TempDir;

mod support;

fn fid(n: u32) -> FileId {
    FileId::new(n).expect("nonzero file id")
}

fn sid(n: u64) -> SymbolId {
    SymbolId::new(n).expect("nonzero symbol id")
}

fn span(file: u32) -> Span {
    Span {
        file: fid(file),
        byte_start: 0,
        byte_end: 32,
    }
}

/// Seed a fixture index and return the built cold [`Catalog`] plus the
/// tempdir guard (kept alive for the catalog's lifetime).
///
/// Symbols (name, kind, file, lang, visibility):
/// - `handle_request`  function  src/main.rs       rust  public
/// - `handle_response` function  src/main.rs       rust  private
/// - `make_handler`    function  src/lib.rs        rust  public
/// - `Server`          struct    src/lib.rs        rust  public
/// - `renderWidget`    function  app/widget.ts     typescript  public
fn fixture() -> (Catalog, TempDir) {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().to_path_buf();
    let storage_path = root.join(".ariadne").join("index.redb");
    let storage = RedbStorage::open(&storage_path).expect("open redb");

    let mut cs = Changeset::new();
    for (id, path, lang) in [
        (1u32, "src/main.rs", Lang::Rust),
        (2, "src/lib.rs", Lang::Rust),
        (3, "app/widget.ts", Lang::TypeScript),
    ] {
        cs = cs.upsert_file(
            fid(id),
            FileRecord {
                path: path.into(),
                lang,
                size: 128,
                blake3: [u8::try_from(id).expect("file id fits u8"); 32],
                mtime_ns: i128::from(id),
            },
        );
    }
    let symbols = [
        (1u64, "handle_request", "function", 1u32, Visibility::Public),
        (2, "handle_response", "function", 1, Visibility::Private),
        (3, "make_handler", "function", 2, Visibility::Public),
        (4, "Server", "struct", 2, Visibility::Public),
        (5, "renderWidget", "function", 3, Visibility::Public),
    ];
    for (id, name, kind, file, visibility) in symbols {
        cs = cs.upsert_symbol(
            sid(id),
            SymbolRecord {
                canonical_name: name.into(),
                kind: kind.into(),
                defining_file: fid(file),
                defining_span: span(file),
                visibility,
                attributes: Vec::new(),
                complexity: 0,
            },
        );
    }
    let txn = storage.begin_write().expect("begin");
    txn.apply(&cs).expect("apply changeset");

    let catalog =
        Catalog::build(&storage, root.to_string_lossy().into_owned()).expect("build catalog");
    drop(storage);
    (catalog, dir)
}

/// Sorted canonical names of a successful search.
fn names(cat: &Catalog, input: &SearchCodeInput) -> Vec<String> {
    let mut out: Vec<String> = search_code::handle(cat, input)
        .expect("search ok")
        .into_iter()
        .map(|s| s.name)
        .collect();
    out.sort();
    out
}

fn input(query: &str) -> SearchCodeInput {
    SearchCodeInput {
        query: query.into(),
        regex: false,
        path: None,
        kind: None,
        lang: None,
        visibility: None,
        limit: None,
    }
}

#[test]
fn search_code_substring_is_the_default_name_match() {
    let (cat, _g) = fixture();
    // Case-insensitive substring "handle" hits the two `handle_*` symbols
    // and `make_handler` (contains "handle"), but not `Server` / the TS one.
    assert_eq!(
        names(&cat, &input("handle")),
        vec!["handle_request", "handle_response", "make_handler"],
    );
}

#[test]
fn search_code_regex_anchors_the_name_match() {
    let (cat, _g) = fixture();
    let mut i = input("^handle");
    i.regex = true;
    // `^handle` matches only names that *start* with "handle" — drops
    // `make_handler`, which the substring path keeps.
    assert_eq!(names(&cat, &i), vec!["handle_request", "handle_response"],);
}

#[test]
fn search_code_path_glob_filters_by_defining_file() {
    let (cat, _g) = fixture();
    let mut i = input("");
    i.path = Some("src/**/*.rs".into());
    // Only the four symbols under `src/*.rs`; the TS symbol in `app/` drops.
    assert_eq!(
        names(&cat, &i),
        vec![
            "Server",
            "handle_request",
            "handle_response",
            "make_handler"
        ],
    );
}

#[test]
fn search_code_kind_filter_narrows_to_a_single_kind() {
    let (cat, _g) = fixture();
    let mut i = input("");
    i.kind = Some("struct".into());
    assert_eq!(names(&cat, &i), vec!["Server"]);
}

#[test]
fn search_code_lang_filter_narrows_to_a_single_language() {
    let (cat, _g) = fixture();
    let mut i = input("");
    i.lang = Some("typescript".into());
    assert_eq!(names(&cat, &i), vec!["renderWidget"]);
}

#[test]
fn search_code_visibility_filter_narrows_to_one_visibility() {
    let (cat, _g) = fixture();
    let mut i = input("");
    i.visibility = Some("private".into());
    assert_eq!(names(&cat, &i), vec!["handle_response"]);
}

#[test]
fn search_code_limit_caps_the_result_count() {
    let (cat, _g) = fixture();
    let mut i = input("handle");
    i.limit = Some(1);
    let hits = search_code::handle(&cat, &i).expect("search ok");
    assert_eq!(hits.len(), 1);
}

#[test]
fn search_code_invalid_regex_is_a_typed_error_not_a_panic() {
    let (cat, _g) = fixture();
    let mut i = input("(unterminated");
    i.regex = true;
    assert!(search_code::handle(&cat, &i).is_err());
}

#[test]
fn search_code_invalid_glob_is_a_typed_error_not_a_panic() {
    let (cat, _g) = fixture();
    let mut i = input("");
    i.path = Some("a[".into());
    assert!(search_code::handle(&cat, &i).is_err());
}

/// End-to-end over the rmcp stdio transport: spawn the `ariadne-mcp` binary,
/// call the registered `search_code` tool with an anchored regex and a path
/// glob, and assert the structured hits — the tier-07 `<verification>` real
/// run, exercised against the shared 4-file fixture.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn search_code_over_stdio_with_regex_and_path_glob() {
    use rmcp::model::CallToolRequestParams;
    use rmcp::object;

    let (root, _guard) = support::seed_tiny_project();
    let client = support::spawn_client(&root).await;

    let resp = client
        .call_tool(
            CallToolRequestParams::new("search_code").with_arguments(object!({
                "query": "^crate::util::",
                "regex": true,
                "path": "src/**/*.rs",
            })),
        )
        .await
        .expect("search_code call");
    let text = support::extract_text(&resp);
    let mut names: Vec<String> = serde_json::from_str::<Vec<serde_json::Value>>(&text)
        .expect("decode search_code")
        .into_iter()
        .map(|v| v["name"].as_str().expect("name string").to_owned())
        .collect();
    names.sort();
    assert_eq!(names, vec!["crate::util::helper", "crate::util::leaf"]);

    client.cancel().await.ok();
}
