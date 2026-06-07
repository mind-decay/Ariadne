//! Block A tier-04 — `fitness_report`. Drives the MCP `#[tool]` end-to-end
//! against a seeded index plus a written `ariadne-fitness.toml`, proving the
//! cold in-process path (catalog → engine) returns the architecture verdict —
//! the same `tools::fitness_report::handle` the CLI `fitness check` calls, so
//! the two surfaces are parity by construction.

mod support;

use rmcp::model::CallToolRequestParams;
use rmcp::object;

/// The canonical fixture carries a `helper.rs → util.rs` edge (sid 5 → sid 3).
/// A rule forbidding `helper → util` flags exactly that inter-file dependency.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn fitness_report_flags_a_forbidden_layer_edge() {
    let (root, _guard) = support::seed_tiny_project();
    std::fs::write(
        root.join("ariadne-fitness.toml"),
        r#"
[[layer]]
name = "helper"
paths = ["src/helper.rs"]

[[layer]]
name = "util"
paths = ["src/util.rs"]

[[rule]]
forbid = { from = "helper", to = "util" }

[thresholds]
max_cycles = 100
"#,
    )
    .expect("write fitness rules");

    let client = support::spawn_client(&root).await;
    let resp = client
        .call_tool(CallToolRequestParams::new("fitness_report").with_arguments(object!({})))
        .await
        .expect("call");
    let v: serde_json::Value = serde_json::from_str(&support::extract_text(&resp)).expect("decode");

    assert_eq!(v["ok"], serde_json::Value::Bool(false), "must fail: {v}");
    let violations = v["violations"].as_array().expect("violations array");
    assert_eq!(violations.len(), 1, "exactly one forbidden dependency: {v}");
    let fd = &violations[0]["forbidden_dependency"];
    assert_eq!(fd["from_layer"], "helper");
    assert_eq!(fd["to_layer"], "util");
    assert_eq!(fd["from_file"], "src/helper.rs");
    assert_eq!(fd["to_file"], "src/util.rs");

    client.cancel().await.ok();
}

/// A rule whose direction has no matching edge (`util → main`) passes clean.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn fitness_report_passes_when_no_rule_is_violated() {
    let (root, _guard) = support::seed_tiny_project();
    std::fs::write(
        root.join("ariadne-fitness.toml"),
        r#"
[[layer]]
name = "main"
paths = ["src/main.rs"]

[[layer]]
name = "util"
paths = ["src/util.rs"]

[[rule]]
forbid = { from = "util", to = "main" }

[thresholds]
max_cycles = 100
"#,
    )
    .expect("write fitness rules");

    let client = support::spawn_client(&root).await;
    let resp = client
        .call_tool(CallToolRequestParams::new("fitness_report").with_arguments(object!({})))
        .await
        .expect("call");
    let v: serde_json::Value = serde_json::from_str(&support::extract_text(&resp)).expect("decode");

    assert_eq!(v["ok"], serde_json::Value::Bool(true), "must pass: {v}");
    assert!(
        v["violations"].as_array().expect("violations").is_empty(),
        "no violations: {v}",
    );

    client.cancel().await.ok();
}
