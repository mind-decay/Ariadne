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
use rmcp::model::CallToolRequestParams;
use rmcp::object;
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
