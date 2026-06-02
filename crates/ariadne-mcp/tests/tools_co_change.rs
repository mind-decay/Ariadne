//! Tier-15b — `co_change` cold-path golden.
//!
//! Seeds the Block-C analytics fixture (one co-change pair: alpha↔beta, shared
//! 3) and asserts the logical-coupling edge. The fixture's revisions (9 / 4)
//! and shared count (3) fall below code-maat's published defaults, so the call
//! lowers all three thresholds to surface the edge; the degree is
//! `3 / mean(9, 4)`. Spawned with daemon autospawn off → cold fallback.

mod support;

use rmcp::model::CallToolRequestParams;
use rmcp::object;

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
