//! Block-1 tier-04 — the dedicated `ariadne affected-tests` CLI command (and
//! its `query affected_tests` twin) run end-to-end over a real indexed git
//! repo, resolving an uncommitted edit to its changed-symbol seed and applying
//! the economy projection (concise default, lossless `detailed`) byte-for-byte
//! like the MCP tool — the third serving path the tier's parity criterion names.

use std::path::Path;
use std::process::Command;

/// Binary under test (the workspace `ariadne` build).
fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_ariadne")
}

/// Resolve queries cold (no daemon) so the output is deterministic.
const AUTOSPAWN_ENV: &str = "ARIADNE_CLI_AUTOSPAWN";

/// Committed `src/lib.rs` — `subject` body holds `1` on line 2.
const HEAD: &str = "pub fn subject() -> i32 {\n    1\n}\n";
/// Worktree `src/lib.rs` — line 2 edited to `2`, so `subject` is the seed.
const WORKTREE: &str = "pub fn subject() -> i32 {\n    2\n}\n";

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

/// Run `<bin> <args...> --root <root>` cold; fail unless it exits successfully,
/// returning stdout.
fn run(root: &Path, args: &[&str]) -> String {
    let output = Command::new(bin())
        .args(args)
        .arg("--root")
        .arg(root)
        .env(AUTOSPAWN_ENV, "0")
        .output()
        .unwrap_or_else(|e| panic!("spawn `ariadne {}`: {e}", args.join(" ")));
    assert!(
        output.status.success(),
        "`ariadne {}` exited with {}: {}",
        args.join(" "),
        output.status,
        String::from_utf8_lossy(&output.stderr).trim(),
    );
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// Build a git repo whose committed `src/lib.rs` differs from the worktree on
/// line 2, then index the worktree layout. Returns the tempdir guard.
fn fixture() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path();

    git(root, &["init", "-b", "main"]);
    std::fs::create_dir_all(root.join("src")).expect("mkdir src");
    std::fs::write(root.join("src/lib.rs"), HEAD).expect("write head lib");
    git(root, &["add", "."]);
    git(root, &["commit", "-m", "c0", "--no-gpg-sign"]);
    // Uncommitted worktree edit — the change `working_tree` scopes.
    std::fs::write(root.join("src/lib.rs"), WORKTREE).expect("write worktree lib");

    // Init then index the worktree (cold parse) so the line-2 hunk resolves to
    // `subject`. Both take ROOT positionally (unlike the `--root` query commands).
    for cmd in [["init"], ["index"]] {
        let out = Command::new(bin())
            .args(cmd)
            .arg(root)
            .output()
            .unwrap_or_else(|e| panic!("spawn `ariadne {}`: {e}", cmd[0]));
        assert!(
            out.status.success(),
            "`ariadne {}` failed: {}",
            cmd[0],
            String::from_utf8_lossy(&out.stderr).trim(),
        );
    }
    dir
}

/// The `seeds` row whose `name` contains `subject`, as a JSON object.
fn subject_seed(json: &serde_json::Value) -> &serde_json::Map<String, serde_json::Value> {
    json["seeds"]
        .as_array()
        .expect("seeds array")
        .iter()
        .find(|s| s["name"].as_str().is_some_and(|n| n.contains("subject")))
        .expect("the changed `subject` resolves to a seed")
        .as_object()
        .expect("seed object")
}

#[test]
fn affected_tests_cli_resolves_seed_with_concise_default() {
    let dir = fixture();
    let root = dir.path();

    let out = run(root, &["affected-tests"]);
    let json: serde_json::Value = serde_json::from_str(&out).expect("valid JSON output");

    // The full report shape is present.
    assert!(json["tests"].is_array(), "tests is an array");
    assert!(json["seeds"].is_array(), "seeds is an array");
    assert!(json["unresolved"].is_array(), "unresolved is an array");

    // Concise default: the seed row omits the cryptic id / byte offsets.
    let seed = subject_seed(&json);
    assert!(
        !seed.contains_key("id"),
        "concise seed omits the cryptic id"
    );
    assert!(
        !seed.contains_key("byte_start"),
        "concise seed omits byte offsets",
    );
}

#[test]
fn affected_tests_cli_query_detailed_keeps_cryptic_fields() {
    let dir = fixture();
    let root = dir.path();

    // The `query affected_tests` route threads economy params from the JSON
    // input (the dedicated CLI path the tier wires); `detailed` is lossless.
    let out = run(
        root,
        &["query", "affected_tests", "{\"verbosity\":\"detailed\"}"],
    );
    let json: serde_json::Value = serde_json::from_str(&out).expect("valid JSON output");
    let seed = subject_seed(&json);
    assert!(
        seed.contains_key("id"),
        "detailed seed keeps the cryptic id (lossless superset)",
    );
    assert!(
        seed.contains_key("byte_start"),
        "detailed seed keeps byte offsets",
    );
}
