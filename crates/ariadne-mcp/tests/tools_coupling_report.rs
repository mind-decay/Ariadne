//! Tier-08 step 9 + Block-1 tier-02 — `coupling_report`: file-as-module
//! metrics, plus the default page cap, cursor round-trip completeness, and the
//! concise == detailed invariant (a metric-only tool has no cryptic fields to
//! drop, so the cap is its only economy win).

mod support;

use rmcp::model::{CallToolRequestParams, JsonObject};
use rmcp::service::RunningService;
use rmcp::{RoleClient, object};
use serde_json::Value;

/// Call `coupling_report` with `args` and return the parsed output object.
async fn coupling(client: &RunningService<RoleClient, ()>, args: JsonObject) -> Value {
    let resp = client
        .call_tool(CallToolRequestParams::new("coupling_report").with_arguments(args))
        .await
        .expect("call");
    serde_json::from_str(&support::extract_text(&resp)).expect("decode")
}

/// Module paths from a `coupling_report` output's `rows` array, in order.
fn module_seq(out: &Value) -> Vec<String> {
    out["rows"]
        .as_array()
        .expect("rows array")
        .iter()
        .map(|r| r["module"].as_str().expect("module").to_owned())
        .collect()
}

/// Default page caps at the economy size and a `limit`-2 page over the 4-module
/// fixture round-trips: page-1 ∪ page-2 equals the un-capped set in stable
/// (Ca desc, module asc) order, no gap or dup.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn coupling_report_caps_and_round_trips() {
    let (root, _guard) = support::seed_tiny_project();
    let client = support::spawn_client(&root).await;

    let full = coupling(&client, object!({ "limit": 50 })).await;
    let full_seq = module_seq(&full);
    assert_eq!(full_seq.len(), 4, "tiny fixture has four file-as-modules");
    assert!(
        full["next_cursor"].is_null(),
        "un-capped result has no cursor"
    );

    let p1 = coupling(&client, object!({ "limit": 2 })).await;
    assert_eq!(module_seq(&p1).len(), 2, "page caps at the limit");
    let cursor = p1["next_cursor"].as_str().expect("page-1 has a cursor");
    assert!(
        p1["note"]
            .as_str()
            .expect("page-1 steer")
            .contains("Showing 2 of 4"),
        "note steers on truncation",
    );

    let p2 = coupling(&client, object!({ "limit": 2, "cursor": cursor })).await;
    assert!(p2["next_cursor"].is_null(), "last page carries no cursor");

    let mut union = module_seq(&p1);
    union.extend(module_seq(&p2));
    assert_eq!(
        union, full_seq,
        "the two pages reconstruct the full set, no gap or dup"
    );

    client.cancel().await.ok();
}

/// A metric-only tool: concise (the default) equals detailed byte-for-byte —
/// `CouplingRow` carries no cryptic id/offset fields, so the cap is its only
/// economy win (tier-02 exit criterion).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn coupling_report_concise_equals_detailed() {
    let (root, _guard) = support::seed_tiny_project();
    let client = support::spawn_client(&root).await;

    let concise = coupling(&client, object!({})).await;
    let detailed = coupling(&client, object!({ "verbosity": "detailed" })).await;
    assert_eq!(
        concise, detailed,
        "coupling concise == detailed (no cryptic fields)"
    );

    client.cancel().await.ok();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn coupling_report_lists_files_as_modules() {
    let (root, _guard) = support::seed_tiny_project();
    let client = support::spawn_client(&root).await;

    let resp = client
        .call_tool(CallToolRequestParams::new("coupling_report").with_arguments(object!({})))
        .await
        .expect("call");
    let v: serde_json::Value = serde_json::from_str(&support::extract_text(&resp)).expect("decode");
    let rows = v["rows"].as_array().expect("rows");
    let modules: Vec<String> = rows
        .iter()
        .map(|r| r["module"].as_str().unwrap().to_owned())
        .collect();
    assert!(modules.contains(&"src/util.rs".to_string()));
    let util = rows.iter().find(|r| r["module"] == "src/util.rs").unwrap();
    assert!(util["efferent"].as_u64().unwrap() >= 1);

    client.cancel().await.ok();
}
