//! Block A tier-04 e2e — `ariadne fitness check`.
//!
//! Seeds a tiny two-file project, indexes it, and drives the CLI gate: a
//! committed `ariadne-fitness.toml` whose forbidden direction matches a real
//! cross-file edge fails (non-zero exit) and lists the violation, and re-running
//! is byte-identical (determinism); a rules file whose forbidden direction has
//! no matching edge passes (exit 0).

use std::path::Path;
use std::process::{Command, Output};

/// Binary under test (the workspace `ariadne` build).
fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_ariadne")
}

/// Cold-path switch: resolve in-process so no daemon is started and output is
/// deterministic [src: crates/ariadne-cli/tests/query.rs].
const AUTOSPAWN_ENV: &str = "ARIADNE_CLI_AUTOSPAWN";

/// `core.rs` calls a uniquely-named function defined in `adapter.rs`, so the
/// resolver links `core_entry → adapter_only_fn` — a `core.rs → adapter.rs`
/// inter-file edge.
const CORE_RS: &str = "pub fn core_entry() -> i32 {\n    adapter_only_fn() + 1\n}\n";
const ADAPTER_RS: &str = "pub fn adapter_only_fn() -> i32 {\n    7\n}\n";

/// Run `<bin> <args...>`; fail unless it exits successfully.
fn run_ok(args: &[&str]) {
    let output = Command::new(bin())
        .args(args)
        .output()
        .unwrap_or_else(|e| panic!("spawn `ariadne {}`: {e}", args.join(" ")));
    assert!(
        output.status.success(),
        "`ariadne {}` exited with {}: {}",
        args.join(" "),
        output.status,
        String::from_utf8_lossy(&output.stderr),
    );
}

/// Run `ariadne fitness check --root <root>` cold, returning the full output.
fn run_fitness(root: &Path) -> Output {
    Command::new(bin())
        .args(["fitness", "check", "--root"])
        .arg(root)
        .env(AUTOSPAWN_ENV, "0")
        .output()
        .expect("spawn `ariadne fitness check`")
}

/// Seed + index a two-file project, writing `rules` as `ariadne-fitness.toml`.
fn seed(rules: &str) -> tempfile::TempDir {
    let project = tempfile::tempdir().expect("create fixture tempdir");
    let root = project.path();
    std::fs::write(root.join("core.rs"), CORE_RS).expect("write core.rs");
    std::fs::write(root.join("adapter.rs"), ADAPTER_RS).expect("write adapter.rs");
    std::fs::write(root.join("ariadne-fitness.toml"), rules).expect("write rules");

    run_ok(&["init", root.to_str().expect("utf8 root")]);
    run_ok(&["index", root.to_str().expect("utf8 root")]);
    project
}

#[test]
fn fitness_check_fails_and_lists_a_seeded_forbidden_dependency() {
    let rules = r#"
[[layer]]
name = "core"
paths = ["core.rs"]

[[layer]]
name = "adapter"
paths = ["adapter.rs"]

[[rule]]
forbid = { from = "core", to = "adapter" }

[thresholds]
max_cycles = 100
"#;
    let project = seed(rules);
    let root = project.path();

    let out = run_fitness(root);
    assert!(
        !out.status.success(),
        "a forbidden dependency must exit non-zero; stderr: {}",
        String::from_utf8_lossy(&out.stderr),
    );
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(
        stdout.contains("forbidden_dependency"),
        "output must list the forbidden dependency:\n{stdout}",
    );
    assert!(
        stdout.contains("core.rs") && stdout.contains("adapter.rs"),
        "output must name both files:\n{stdout}",
    );
    assert!(
        stdout.contains("\"ok\": false"),
        "ok must be false:\n{stdout}"
    );

    // Determinism: a second run is byte-identical.
    let again = run_fitness(root);
    assert_eq!(
        stdout,
        String::from_utf8_lossy(&again.stdout),
        "re-run must be byte-identical",
    );
}

#[test]
fn fitness_check_passes_when_no_rule_is_violated() {
    // The only cross-file edge is core.rs → adapter.rs; forbidding the reverse
    // direction (adapter → core) leaves the project clean.
    let rules = r#"
[[layer]]
name = "core"
paths = ["core.rs"]

[[layer]]
name = "adapter"
paths = ["adapter.rs"]

[[rule]]
forbid = { from = "adapter", to = "core" }

[thresholds]
max_cycles = 100
"#;
    let project = seed(rules);
    let root = project.path();

    let out = run_fitness(root);
    assert!(
        out.status.success(),
        "a clean project must exit 0; stderr: {}",
        String::from_utf8_lossy(&out.stderr),
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("\"ok\": true"),
        "ok must be true:\n{stdout}"
    );
}
