//! Block-1 tier-04 — `affected_tests`: the `tests` / `seeds` top-level lists
//! each cap at the per-sublist limit behind ONE shared multi-list cursor
//! (round-trip completeness), the concise default drops the embedded
//! `SymbolSummary` cryptic fields, and the changed-paths fingerprint guard
//! rejects a cursor once the working-tree diff changes between pages.

mod support;

use std::path::{Path, PathBuf};
use std::process::Command;

use ariadne_core::{
    Changeset, EdgeKey, EdgeKind, EdgeRecord, FileRecord, Lang, Span, Storage, SymbolRecord,
    Visibility, WriteTxn,
};
use ariadne_storage::RedbStorage;
use rmcp::model::{CallToolRequestParams, JsonObject};
use rmcp::service::RunningService;
use rmcp::{RoleClient, object};
use serde_json::Value;
use tempfile::TempDir;

/// Committed `src/lib.rs`: a `subject` (line 2) plus two `#[test]` functions
/// that call it. Line 2 holds `let v = 1;`.
const HEAD: &str = "fn subject() {\n    let v = 1;\n}\n\nfn test_one() {\n    subject();\n}\n\nfn test_two() {\n    subject();\n}\n";
/// Worktree `src/lib.rs`: line 2 edited to `let v = 2;` — `subject` is the seed.
const WORKTREE: &str = "fn subject() {\n    let v = 2;\n}\n\nfn test_one() {\n    subject();\n}\n\nfn test_two() {\n    subject();\n}\n";

/// Run `git` in `repo`, isolated from ambient config. Panics on non-zero exit.
fn git(repo: &Path, args: &[&str]) {
    let output = Command::new("git")
        .current_dir(repo)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@x")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@x")
        .args(args)
        .output()
        .expect("spawn git");
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr),
    );
}

fn u32_of(n: usize) -> u32 {
    u32::try_from(n).expect("offset fits u32")
}

/// Byte span `[start, end)` of the function declared by `marker` (single-line
/// bodies): from the `fn name` offset to just past its first `}`.
fn span_of(content: &str, marker: &str) -> (u32, u32) {
    let start = content.find(marker).expect("function decl present");
    let end = content[start..].find('}').expect("function brace") + start + 1;
    (u32_of(start), u32_of(end))
}

/// Build a git repo whose committed `src/lib.rs` differs from the worktree on
/// line 2 (inside `subject`), plus a committed `src/other.rs` (initially
/// unchanged). The index is seeded to the worktree layout; `test_one` /
/// `test_two` carry the Rust `#[test]` attribute and both call `subject`, so the
/// changed seed reverse-reaches two test roots.
fn seed_affected_fixture() -> (PathBuf, TempDir) {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().to_path_buf();

    git(&root, &["init", "-b", "main"]);
    std::fs::create_dir_all(root.join("src")).expect("mkdir src");
    std::fs::write(root.join("src/lib.rs"), HEAD).expect("write head lib");
    std::fs::write(root.join("src/other.rs"), "fn other() {}\n").expect("write other");
    git(&root, &["add", "."]);
    git(&root, &["commit", "-m", "c0", "--no-gpg-sign"]);

    std::fs::write(root.join("src/lib.rs"), WORKTREE).expect("write worktree lib");

    let blake = *blake3::hash(WORKTREE.as_bytes()).as_bytes();
    let mut cs = Changeset::new();
    cs = cs.upsert_file(
        support::fid(1),
        FileRecord {
            path: "src/lib.rs".into(),
            lang: Lang::Rust,
            size: u64::try_from(WORKTREE.len()).expect("size fits u64"),
            blake3: blake,
            mtime_ns: 1,
        },
    );
    // (sid, name, marker, is_test) — the two callers are `#[test]` roots.
    let funcs = [
        (1u64, "crate::subject", "fn subject", false),
        (2, "crate::test_one", "fn test_one", true),
        (3, "crate::test_two", "fn test_two", true),
    ];
    for (sid, name, marker, is_test) in funcs {
        let (byte_start, byte_end) = span_of(WORKTREE, marker);
        cs = cs.upsert_symbol(
            support::sid(sid),
            SymbolRecord {
                canonical_name: name.into(),
                kind: "function".into(),
                defining_file: support::fid(1),
                defining_span: Span {
                    file: support::fid(1),
                    byte_start,
                    byte_end,
                },
                visibility: Visibility::Unknown,
                attributes: if is_test {
                    vec!["test".into()]
                } else {
                    Vec::new()
                },
                complexity: 0,
            },
        );
    }
    for src in [2u64, 3] {
        let idx = usize::try_from(src - 1).expect("seed index fits usize");
        let (s0, s1) = span_of(WORKTREE, funcs[idx].2);
        cs = cs.add_edge(
            EdgeKey {
                src: support::sid(src),
                kind: EdgeKind::References,
                dst: support::sid(1),
            },
            EdgeRecord {
                source_span: Span {
                    file: support::fid(1),
                    byte_start: s0,
                    byte_end: s1,
                },
                evidence_lang: Lang::Rust,
                weight: 1,
            },
        );
    }

    let storage = RedbStorage::open(&root.join(".ariadne").join("index.redb")).expect("open redb");
    storage
        .begin_write()
        .expect("begin")
        .apply(&cs)
        .expect("apply changeset");
    drop(storage);

    (root, dir)
}

/// Call `affected_tests` with `args` and return the parsed output object.
async fn at(client: &RunningService<RoleClient, ()>, args: JsonObject) -> Value {
    let resp = client
        .call_tool(CallToolRequestParams::new("affected_tests").with_arguments(args))
        .await
        .expect("call affected_tests");
    serde_json::from_str(&support::extract_text(&resp)).expect("decode")
}

/// The `name` of every row in `out[key]`, in order.
fn names(out: &Value, key: &str) -> Vec<String> {
    out[key]
        .as_array()
        .unwrap_or(&Vec::new())
        .iter()
        .map(|r| r["name"].as_str().expect("name").to_owned())
        .collect()
}

/// Whether every row in `out[key]` carries `field` as a JSON key.
fn rows_have_field(out: &Value, key: &str, field: &str) -> bool {
    out[key]
        .as_array()
        .expect("array")
        .iter()
        .all(|r| r.as_object().expect("object").contains_key(field))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn affected_tests_resolves_changed_seed_to_its_tests() {
    let (root, _guard) = seed_affected_fixture();
    let client = support::spawn_client(&root).await;

    let out = at(&client, object!({ "limit": 50, "verbosity": "detailed" })).await;
    assert_eq!(
        names(&out, "tests"),
        vec!["crate::test_one", "crate::test_two"],
        "both #[test] callers of the changed seed are affected",
    );
    assert_eq!(names(&out, "seeds"), vec!["crate::subject"], "one seed");
    assert!(out["next_cursor"].is_null(), "un-capped → no cursor");

    client.cancel().await.ok();
}

/// `tests` and `seeds` each cap at the per-sublist limit behind ONE shared
/// multi-list cursor; a `limit:1` round-trip reconstructs the un-capped `tests`
/// list with no gap or dup (completeness across sublists).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn affected_tests_caps_and_round_trips() {
    let (root, _guard) = seed_affected_fixture();
    let client = support::spawn_client(&root).await;

    let full = at(&client, object!({ "limit": 50, "verbosity": "detailed" })).await;
    let tests_full = names(&full, "tests");
    assert_eq!(tests_full.len(), 2, "two affected tests");

    let p1 = at(&client, object!({ "limit": 1, "verbosity": "detailed" })).await;
    assert_eq!(names(&p1, "tests").len(), 1, "tests page caps at the limit");
    assert_eq!(names(&p1, "seeds").len(), 1, "the one seed exhausts");
    let cursor = p1["next_cursor"]
        .as_str()
        .expect("tests overflows → one shared cursor")
        .to_owned();
    assert!(
        p1["note"].as_str().expect("steer").contains("tests"),
        "note names the truncated list",
    );

    let p2 = at(
        &client,
        object!({ "limit": 1, "cursor": cursor, "verbosity": "detailed" }),
    )
    .await;
    assert!(p2["next_cursor"].is_null(), "last page carries no cursor");
    assert!(
        names(&p2, "seeds").is_empty(),
        "seeds exhausted on page 1 → empty on page 2",
    );

    let mut tests_union = names(&p1, "tests");
    tests_union.extend(names(&p2, "tests"));
    assert_eq!(
        tests_union, tests_full,
        "tests pages reconstruct the full list"
    );

    client.cancel().await.ok();
}

/// Concise (the default) drops the embedded `SymbolSummary` cryptic fields from
/// both `tests` and `seeds`; detailed keeps them. The semantic `name` survives.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn affected_tests_concise_drops_cryptic_fields() {
    let (root, _guard) = seed_affected_fixture();
    let client = support::spawn_client(&root).await;

    let concise = at(&client, object!({})).await;
    assert!(
        !rows_have_field(&concise, "tests", "id"),
        "concise tests omit the cryptic id",
    );
    let detailed = at(&client, object!({ "verbosity": "detailed" })).await;
    assert!(
        rows_have_field(&detailed, "tests", "id"),
        "detailed tests keep the id (lossless superset)",
    );
    assert_eq!(
        names(&concise, "tests"),
        names(&detailed, "tests"),
        "concise ⊂ detailed: same rows, fewer fields",
    );

    client.cancel().await.ok();
}

/// A working-tree diff that changes between pages invalidates the cursor: the
/// cursor carries a changed-paths fingerprint, so editing a second file after
/// page 1 yields a graceful invalid-cursor error.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn affected_tests_stale_fingerprint_rejects_cursor() {
    let (root, _guard) = seed_affected_fixture();
    let client = support::spawn_client(&root).await;

    let p1 = at(&client, object!({ "limit": 1 })).await;
    let cursor = p1["next_cursor"]
        .as_str()
        .expect("a remainder mints a cursor")
        .to_owned();

    std::fs::write(root.join("src/other.rs"), "fn other() { let z = 9; }\n")
        .expect("edit other.rs");

    let err = client
        .call_tool(
            CallToolRequestParams::new("affected_tests")
                .with_arguments(object!({ "limit": 1, "cursor": cursor })),
        )
        .await
        .expect_err("a stale changed-paths fingerprint must reject the cursor");
    let msg = format!("{err:?}");
    assert!(
        msg.contains("cursor"),
        "the error must name the invalid cursor, got: {msg}",
    );

    client.cancel().await.ok();
}
