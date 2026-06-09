//! Tier-15c — `diff_blast_radius` cold-path golden over a real fixture git repo.
//!
//! Builds a throwaway git repo whose committed `src/lib.rs` differs from the
//! worktree on line 2 (inside `callee`), then seeds `.ariadne/index.redb` to the
//! *worktree* layout — the live-index invariant the daemon watcher upholds
//! (tier-08) — so the `WorkingTree` diff's line-2 hunk resolves to the `callee`
//! seed. Spawns the `ariadne-mcp` binary with daemon autospawn off (cold path),
//! calls `diff_blast_radius` with the default `WorkingTree` spec, and asserts a
//! stable `insta` golden plus the tier-14 invariant re-asserted through the live
//! tool: the report's must∪may equals the union of `blast_radius` over the
//! changed seeds [src: .claude/plans/post-v1-roadmap/tier-15c-diff-blast-radius-tool.md step 1].

mod support;

use std::collections::BTreeSet;
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

/// Committed (HEAD) content — line 2 holds `let v = 1;`.
const HEAD_LIB: &str = "fn callee() {\n    let v = 1;\n}\n\nfn caller() {\n    callee();\n}\n";
/// Worktree content (uncommitted edit) — line 2 holds `let v = 2;`.
const WORKTREE_LIB: &str = "fn callee() {\n    let v = 2;\n}\n\nfn caller() {\n    callee();\n}\n";

/// Run `git` in `repo`, isolated from ambient user/global config so the fixture
/// is reproducible on any host. Panics on non-zero exit.
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

/// Build the fixture: a git repo whose committed `src/lib.rs` differs from the
/// worktree on line 2, plus an index seeded to the worktree layout. `callee`
/// (sid 1) spans lines 1-3, `caller` (sid 2) spans lines 5-7, and `caller`
/// references `callee`, so `blast_radius(callee)` reaches `caller`.
fn seed_diff_fixture() -> (PathBuf, TempDir) {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().to_path_buf();

    git(&root, &["init", "-b", "main"]);
    std::fs::create_dir_all(root.join("src")).expect("mkdir src");
    std::fs::write(root.join("src/lib.rs"), HEAD_LIB).expect("write head lib");
    git(&root, &["add", "src/lib.rs"]);
    git(&root, &["commit", "-m", "c0", "--no-gpg-sign"]);

    // Uncommitted worktree edit (line 2) — the change `WorkingTree` scopes.
    std::fs::write(root.join("src/lib.rs"), WORKTREE_LIB).expect("write worktree lib");

    // Symbol spans derived from the worktree content so the index matches it.
    let seed_span_end = u32_of(WORKTREE_LIB.find('}').expect("callee brace") + 1);
    let dep_span_start = u32_of(WORKTREE_LIB.find("fn caller").expect("caller decl"));
    let dep_span_end = u32_of(WORKTREE_LIB.rfind('}').expect("caller brace") + 1);
    let blake = *blake3::hash(WORKTREE_LIB.as_bytes()).as_bytes();

    let mut cs = Changeset::new();
    cs = cs.upsert_file(
        support::fid(1),
        FileRecord {
            path: "src/lib.rs".into(),
            lang: Lang::Rust,
            size: 128,
            blake3: blake,
            mtime_ns: 1,
        },
    );
    cs = cs.upsert_symbol(
        support::sid(1),
        SymbolRecord {
            canonical_name: "crate::callee".into(),
            kind: "function".into(),
            defining_file: support::fid(1),
            defining_span: Span {
                file: support::fid(1),
                byte_start: 0,
                byte_end: seed_span_end,
            },
            visibility: Visibility::Unknown,
            attributes: Vec::new(),
            complexity: 0,
        },
    );
    cs = cs.upsert_symbol(
        support::sid(2),
        SymbolRecord {
            canonical_name: "crate::caller".into(),
            kind: "function".into(),
            defining_file: support::fid(1),
            defining_span: Span {
                file: support::fid(1),
                byte_start: dep_span_start,
                byte_end: dep_span_end,
            },
            visibility: Visibility::Unknown,
            attributes: Vec::new(),
            complexity: 0,
        },
    );
    // `caller` (src) references `callee` (dst): `blast_radius(callee)` reaches
    // `caller` as a first-hop must-touch.
    cs = cs.add_edge(
        EdgeKey {
            src: support::sid(2),
            kind: EdgeKind::References,
            dst: support::sid(1),
        },
        EdgeRecord {
            source_span: Span {
                file: support::fid(1),
                byte_start: dep_span_start,
                byte_end: dep_span_end,
            },
            evidence_lang: Lang::Rust,
            weight: 1,
        },
    );

    let storage = RedbStorage::open(&root.join(".ariadne").join("index.redb")).expect("open redb");
    storage
        .begin_write()
        .expect("begin")
        .apply(&cs)
        .expect("apply changeset");
    drop(storage);

    (root, dir)
}

/// The set of `must_touch ∪ may_touch` canonical names in a `blast_radius`-shaped
/// JSON value (works for both `blast_radius` and a `diff_blast_radius` seed/report).
fn impact_names(value: &serde_json::Value) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    for key in ["must_touch", "may_touch"] {
        if let Some(rows) = value[key].as_array() {
            for row in rows {
                if let Some(name) = row["name"].as_str() {
                    names.insert(name.to_owned());
                }
            }
        }
    }
    names
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn diff_blast_radius_working_tree_resolves_changed_seed() {
    let (root, _guard) = seed_diff_fixture();
    let client = support::spawn_client(&root).await;

    // Default spec is `WorkingTree`: the uncommitted line-2 edit lands inside
    // `callee`, whose blast radius reaches `caller`.
    let resp = client
        .call_tool(CallToolRequestParams::new("diff_blast_radius").with_arguments(object!({})))
        .await
        .expect("call diff_blast_radius");
    let report: serde_json::Value =
        serde_json::from_str(&support::extract_text(&resp)).expect("decode report");

    // One seed (`crate::callee`), must-touch `crate::caller`, nothing unresolved.
    assert_eq!(report["seeds"].as_array().expect("seeds").len(), 1);
    assert_eq!(report["seeds"][0]["symbol"]["name"], "crate::callee");
    assert_eq!(report["must_touch"][0]["name"], "crate::caller");
    assert!(
        report["unresolved"]
            .as_array()
            .expect("unresolved")
            .is_empty(),
        "the changed file resolved to a seed, so nothing is unresolved",
    );

    // tier-14 invariant through the live tool: the report's must∪may equals the
    // union of `blast_radius` over the changed seed (`crate::callee`).
    let br = client
        .call_tool(
            CallToolRequestParams::new("blast_radius")
                .with_arguments(object!({ "symbol": "crate::callee" })),
        )
        .await
        .expect("call blast_radius");
    let br_value: serde_json::Value =
        serde_json::from_str(&support::extract_text(&br)).expect("decode blast_radius");
    assert_eq!(
        impact_names(&report),
        impact_names(&br_value),
        "diff_blast_radius must∪may equals the per-seed blast_radius union",
    );

    let golden = serde_json::to_string_pretty(&report).expect("serialize golden");
    insta::assert_snapshot!("diff_blast_working_tree", golden);

    client.cancel().await.ok();
}

// ---------------------------------------------------------------------------
// Block-1 tier-04 — top-level cap + multi-list cursor, per-seed inner cap with
// a reported count, concise projection, and the changed-paths fingerprint guard.
// ---------------------------------------------------------------------------

/// Committed `src/lib.rs`: two seed functions (`alpha`, `beta`) on lines 2 + 6,
/// three callers (two of `alpha`, one of `beta`). Line 2/6 hold `let v = 1;`.
const MULTI_HEAD: &str = "fn alpha() {\n    let v = 1;\n}\n\nfn beta() {\n    let v = 1;\n}\n\nfn caller_a1() {\n    alpha();\n}\n\nfn caller_a2() {\n    alpha();\n}\n\nfn caller_b1() {\n    beta();\n}\n";
/// Worktree `src/lib.rs`: lines 2 + 6 edited to `let v = 2;` — both `alpha` and
/// `beta` are changed seeds.
const MULTI_WORKTREE: &str = "fn alpha() {\n    let v = 2;\n}\n\nfn beta() {\n    let v = 2;\n}\n\nfn caller_a1() {\n    alpha();\n}\n\nfn caller_a2() {\n    alpha();\n}\n\nfn caller_b1() {\n    beta();\n}\n";

/// Byte span `[start, end)` of the function declared by `marker` in `content`:
/// from the `fn name` offset to just past its first `}` (single-line bodies).
fn span_of(content: &str, marker: &str) -> (u32, u32) {
    let start = content.find(marker).expect("function decl present");
    let end = content[start..].find('}').expect("function brace") + start + 1;
    (u32_of(start), u32_of(end))
}

/// Build a git repo whose committed `src/lib.rs` differs from the worktree on
/// lines 2 + 6 (inside `alpha` and `beta`), plus a committed `src/other.rs`
/// (initially unchanged). The index is seeded to the worktree layout so the
/// `WorkingTree` diff resolves both edits to their seeds: `alpha` (two callers)
/// and `beta` (one caller). Returns the root + tempdir guard.
fn seed_multi_seed_fixture() -> (PathBuf, TempDir) {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().to_path_buf();

    git(&root, &["init", "-b", "main"]);
    std::fs::create_dir_all(root.join("src")).expect("mkdir src");
    std::fs::write(root.join("src/lib.rs"), MULTI_HEAD).expect("write head lib");
    std::fs::write(root.join("src/other.rs"), "fn other() {}\n").expect("write other");
    git(&root, &["add", "."]);
    git(&root, &["commit", "-m", "c0", "--no-gpg-sign"]);

    // Uncommitted worktree edit on lines 2 + 6 — the change `WorkingTree` scopes.
    std::fs::write(root.join("src/lib.rs"), MULTI_WORKTREE).expect("write worktree lib");

    let blake = *blake3::hash(MULTI_WORKTREE.as_bytes()).as_bytes();
    let mut cs = Changeset::new();
    cs = cs.upsert_file(
        support::fid(1),
        FileRecord {
            path: "src/lib.rs".into(),
            lang: Lang::Rust,
            size: u64::try_from(MULTI_WORKTREE.len()).expect("size fits u64"),
            blake3: blake,
            mtime_ns: 1,
        },
    );
    // (sid, canonical name) for the five functions; spans derived from content.
    let funcs = [
        (1u64, "crate::alpha", "fn alpha"),
        (2, "crate::beta", "fn beta"),
        (3, "crate::caller_a1", "fn caller_a1"),
        (4, "crate::caller_a2", "fn caller_a2"),
        (5, "crate::caller_b1", "fn caller_b1"),
    ];
    for (sid, name, marker) in funcs {
        let (byte_start, byte_end) = span_of(MULTI_WORKTREE, marker);
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
                attributes: Vec::new(),
                complexity: 0,
            },
        );
    }
    // caller_a1, caller_a2 → alpha; caller_b1 → beta. Reverse-reach gives
    // alpha two first-hop must-touch callers, beta one.
    for (src, dst) in [(3u64, 1u64), (4, 1), (5, 2)] {
        let idx = usize::try_from(src - 1).expect("seed index fits usize");
        let (s0, s1) = span_of(MULTI_WORKTREE, funcs[idx].2);
        cs = cs.add_edge(
            EdgeKey {
                src: support::sid(src),
                kind: EdgeKind::References,
                dst: support::sid(dst),
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

/// Call `diff_blast_radius` with `args` and return the parsed output object.
async fn dbr(client: &RunningService<RoleClient, ()>, args: JsonObject) -> Value {
    let resp = client
        .call_tool(CallToolRequestParams::new("diff_blast_radius").with_arguments(args))
        .await
        .expect("call diff_blast_radius");
    serde_json::from_str(&support::extract_text(&resp)).expect("decode")
}

/// The `name` of every row in the top-level list `out[key]`, in order.
fn top_names(out: &Value, key: &str) -> Vec<String> {
    out[key]
        .as_array()
        .unwrap_or(&Vec::new())
        .iter()
        .map(|r| r["name"].as_str().expect("name").to_owned())
        .collect()
}

/// The `symbol.name` of every seed row, in order.
fn seed_names(out: &Value) -> Vec<String> {
    out["seeds"]
        .as_array()
        .unwrap_or(&Vec::new())
        .iter()
        .map(|s| s["symbol"]["name"].as_str().expect("seed name").to_owned())
        .collect()
}

/// The three top-level lists each cap at the per-sublist limit behind ONE shared
/// multi-list cursor; a `limit:1` round-trip reconstructs the un-capped `seeds`
/// and aggregate `must_touch` with no gap or dup (completeness across sublists).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn diff_blast_caps_and_round_trips_top_level() {
    let (root, _guard) = seed_multi_seed_fixture();
    let client = support::spawn_client(&root).await;

    let full = dbr(&client, object!({ "limit": 50, "verbosity": "detailed" })).await;
    let seeds_full = seed_names(&full);
    let must_full = top_names(&full, "must_touch");
    assert_eq!(seeds_full, vec!["crate::alpha", "crate::beta"], "two seeds");
    assert_eq!(must_full.len(), 3, "three aggregate must-touch callers");
    assert!(full["next_cursor"].is_null(), "un-capped → no cursor");

    let p1 = dbr(&client, object!({ "limit": 1, "verbosity": "detailed" })).await;
    assert_eq!(seed_names(&p1).len(), 1, "seeds page caps at the limit");
    assert_eq!(top_names(&p1, "must_touch").len(), 1, "must page caps");
    let cursor = p1["next_cursor"]
        .as_str()
        .expect("a remainder mints one shared cursor")
        .to_owned();
    assert!(
        p1["note"].as_str().expect("steer").contains("seeds"),
        "note names a truncated top-level list",
    );

    // Page through until the cursor is exhausted, unioning seeds + must_touch.
    let mut seed_union = seed_names(&p1);
    let mut must_union = top_names(&p1, "must_touch");
    let mut cur = cursor;
    let mut pages = 1;
    loop {
        let p = dbr(
            &client,
            object!({ "limit": 1, "cursor": cur, "verbosity": "detailed" }),
        )
        .await;
        pages += 1;
        seed_union.extend(seed_names(&p));
        must_union.extend(top_names(&p, "must_touch"));
        match p["next_cursor"].as_str() {
            Some(next) => cur = next.to_owned(),
            None => break,
        }
        assert!(pages < 10, "pagination must terminate");
    }
    assert_eq!(
        seed_union, seeds_full,
        "seeds pages reconstruct the full list"
    );
    assert_eq!(
        must_union, must_full,
        "must_touch pages reconstruct the full list"
    );

    client.cancel().await.ok();
}

/// Each paged seed's inner `must_touch` is bounded by a fixed cap (= `limit`)
/// with a reported per-seed count — never silently dropped, never a nested
/// cursor. `alpha` has two first-hop callers; `limit:1` shows one and reports
/// `must_touch_total: 2`.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn diff_blast_per_seed_inner_cap_reports_count() {
    let (root, _guard) = seed_multi_seed_fixture();
    let client = support::spawn_client(&root).await;

    // `limit:1` keeps `alpha` first in the seeds page (sorted by byte_start).
    let p = dbr(&client, object!({ "limit": 1, "verbosity": "detailed" })).await;
    let alpha = &p["seeds"][0];
    assert_eq!(alpha["symbol"]["name"], "crate::alpha");
    assert_eq!(
        alpha["must_touch"].as_array().expect("inner must").len(),
        1,
        "inner must_touch bounded by the fixed cap (= limit)",
    );
    assert_eq!(
        alpha["must_touch_total"], 2,
        "the full inner must count is reported, not silently dropped",
    );
    assert_eq!(alpha["may_touch_total"], 0, "inner may count reported as 0");

    client.cancel().await.ok();
}

/// Concise (the default) drops the embedded `SymbolSummary` cryptic fields from
/// the seeds, the aggregate lists, and the per-seed inner lists; detailed keeps
/// them. The per-seed count fields survive both verbosities.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn diff_blast_concise_drops_cryptic_fields() {
    let (root, _guard) = seed_multi_seed_fixture();
    let client = support::spawn_client(&root).await;

    let concise = dbr(&client, object!({})).await;
    assert!(
        !concise["seeds"][0]["symbol"]
            .as_object()
            .expect("seed symbol")
            .contains_key("id"),
        "concise seed symbol omits the cryptic id",
    );
    assert!(
        !concise["must_touch"][0]
            .as_object()
            .expect("must row")
            .contains_key("byte_start"),
        "concise aggregate must_touch omits byte offsets",
    );
    assert!(
        concise["seeds"][0]
            .as_object()
            .expect("seed row")
            .contains_key("must_touch_total"),
        "the per-seed count survives concise",
    );

    let detailed = dbr(&client, object!({ "verbosity": "detailed" })).await;
    assert!(
        detailed["seeds"][0]["symbol"]
            .as_object()
            .expect("seed symbol")
            .contains_key("id"),
        "detailed keeps the cryptic id (lossless superset)",
    );

    client.cancel().await.ok();
}

/// A working-tree diff that changes between pages invalidates the cursor: the
/// cursor is stamped with a changed-paths fingerprint, so editing a second file
/// after page 1 yields a graceful invalid-cursor error, never wrong rows.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn diff_blast_stale_fingerprint_rejects_cursor() {
    let (root, _guard) = seed_multi_seed_fixture();
    let client = support::spawn_client(&root).await;

    let p1 = dbr(&client, object!({ "limit": 1 })).await;
    let cursor = p1["next_cursor"]
        .as_str()
        .expect("a remainder mints a cursor")
        .to_owned();

    // Change the working-tree diff: edit a second tracked file so `changed_paths`
    // grows from {lib.rs} to {lib.rs, other.rs} — a different result set.
    std::fs::write(root.join("src/other.rs"), "fn other() { let z = 9; }\n")
        .expect("edit other.rs");

    let err = client
        .call_tool(
            CallToolRequestParams::new("diff_blast_radius")
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
