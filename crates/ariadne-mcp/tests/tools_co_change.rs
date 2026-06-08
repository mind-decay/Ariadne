//! Tier-15b — `co_change` cold-path golden.
//!
//! Seeds the Block-C analytics fixture (one co-change pair: alpha↔beta, shared
//! 3) and asserts the logical-coupling edge. The fixture's revisions (9 / 4)
//! and shared count (3) fall below code-maat's published defaults, so the call
//! lowers all three thresholds to surface the edge; the degree is
//! `3 / mean(9, 4)`. Spawned with daemon autospawn off → cold fallback.

mod support;

use rmcp::model::{CallToolRequestParams, JsonObject};
use rmcp::service::RunningService;
use rmcp::{RoleClient, object};
use serde_json::Value;

/// Call `co_change` with `args` and return the parsed output object.
async fn co_change(client: &RunningService<RoleClient, ()>, args: JsonObject) -> Value {
    let resp = client
        .call_tool(CallToolRequestParams::new("co_change").with_arguments(args))
        .await
        .expect("call");
    serde_json::from_str(&support::extract_text(&resp)).expect("decode")
}

/// `(a, b)` pairs from a `co_change` output's `edges` array, in order.
fn edge_seq(out: &Value) -> Vec<(String, String)> {
    out["edges"]
        .as_array()
        .expect("edges array")
        .iter()
        .map(|e| {
            (
                e["a"].as_str().expect("a").to_owned(),
                e["b"].as_str().expect("b").to_owned(),
            )
        })
        .collect()
}

/// A `limit`-1 page over the two-pair fixture round-trips: page-1 ∪ page-2
/// equals the un-capped set in stable (degree desc, then `(a,b)` asc) order,
/// no gap or dup.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn co_change_caps_and_round_trips() {
    let (root, _guard) = support::seed_cochange_pairs_project();
    let client = support::spawn_client(&root).await;

    let base = object!({ "min_revs": 1, "min_shared_commits": 1, "min_degree": 0.0 });
    let mut full_args = base.clone();
    full_args.insert("limit".into(), 50.into());
    let full = co_change(&client, full_args).await;
    let full_seq = edge_seq(&full);
    assert_eq!(full_seq.len(), 2, "fixture surfaces two coupled pairs");

    let mut p1_args = base.clone();
    p1_args.insert("limit".into(), 1.into());
    let p1 = co_change(&client, p1_args).await;
    assert_eq!(edge_seq(&p1).len(), 1, "page caps at the limit");
    let cursor = p1["next_cursor"]
        .as_str()
        .expect("page-1 has a cursor")
        .to_owned();
    assert!(
        p1["note"]
            .as_str()
            .expect("page-1 steer")
            .contains("Showing 1 of 2"),
        "note steers on truncation",
    );

    let mut p2_args = base.clone();
    p2_args.insert("limit".into(), 1.into());
    p2_args.insert("cursor".into(), cursor.into());
    let p2 = co_change(&client, p2_args).await;
    assert!(p2["next_cursor"].is_null(), "last page carries no cursor");

    let mut union = edge_seq(&p1);
    union.extend(edge_seq(&p2));
    assert_eq!(
        union, full_seq,
        "the two pages reconstruct the full set, no gap or dup"
    );

    client.cancel().await.ok();
}

/// A metric-only tool: concise (the default) equals detailed byte-for-byte —
/// `CoChangeEdge` carries no cryptic fields, so the cap is its only win.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn co_change_concise_equals_detailed() {
    let (root, _guard) = support::seed_cochange_pairs_project();
    let client = support::spawn_client(&root).await;

    let base = object!({ "min_revs": 1, "min_shared_commits": 1, "min_degree": 0.0 });
    let concise = co_change(&client, base.clone()).await;
    let mut detailed_args = base.clone();
    detailed_args.insert("verbosity".into(), "detailed".into());
    let detailed = co_change(&client, detailed_args).await;
    assert_eq!(
        concise, detailed,
        "co_change concise == detailed (no cryptic fields)"
    );

    client.cancel().await.ok();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn co_change_lists_coupled_pair_under_lowered_thresholds() {
    let (root, _guard) = support::seed_analytics_project();
    let client = support::spawn_client(&root).await;

    let resp = client
        .call_tool(
            CallToolRequestParams::new("co_change").with_arguments(object!({
                "min_revs": 1,
                "min_shared_commits": 1,
                "min_degree": 0.0,
            })),
        )
        .await
        .expect("call");
    let v: serde_json::Value = serde_json::from_str(&support::extract_text(&resp)).expect("decode");

    assert_eq!(v["edges"][0]["a"], "src/alpha.rs");
    assert_eq!(v["edges"][0]["b"], "src/beta.rs");
    assert_eq!(v["edges"][0]["shared_commits"], 3);

    let golden = serde_json::to_string_pretty(&v).expect("serialize golden");
    insta::assert_snapshot!("co_change_lowered", golden);

    client.cancel().await.ok();
}

/// At code-maat's default thresholds the fixture clears nothing (beta has 4
/// revisions < `min_revs` 5, shared 3 < `min_shared_commits` 5), so the report
/// is empty — the thresholds are honored, not ignored.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn co_change_defaults_filter_the_fixture_pair() {
    let (root, _guard) = support::seed_analytics_project();
    let client = support::spawn_client(&root).await;

    let resp = client
        .call_tool(CallToolRequestParams::new("co_change").with_arguments(object!({})))
        .await
        .expect("call");
    let v: serde_json::Value = serde_json::from_str(&support::extract_text(&resp)).expect("decode");

    assert!(
        v["edges"].as_array().expect("edges array").is_empty(),
        "default thresholds exclude the fixture pair, got {v}",
    );

    client.cancel().await.ok();
}
