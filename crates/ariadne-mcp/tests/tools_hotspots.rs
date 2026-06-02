//! Tier-15b — `hotspots` cold-path golden.
//!
//! Seeds the Block-C analytics fixture (`seed_analytics_project`), spawns the
//! `ariadne-mcp` binary with daemon autospawn off (so the routing path is the
//! cold fallback), and locks the churn × complexity ranking at both grains
//! against an `insta` golden. The same fixture backs the warm daemon
//! projection, so cold and warm output are byte-identical.

mod support;

use rmcp::model::CallToolRequestParams;
use rmcp::object;

/// File grain ranks `src/alpha.rs` (churn 9 × Σ-complexity 7) above
/// `src/beta.rs` (churn 4 × Σ-complexity 3).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn hotspots_file_grain_ranks_by_churn_times_complexity() {
    let (root, _guard) = support::seed_analytics_project();
    let client = support::spawn_client(&root).await;

    let resp = client
        .call_tool(
            CallToolRequestParams::new("hotspots").with_arguments(object!({ "grain": "file" })),
        )
        .await
        .expect("call");
    let v: serde_json::Value = serde_json::from_str(&support::extract_text(&resp)).expect("decode");

    // Strongest hotspot first: alpha (score 1.0) before beta.
    assert_eq!(v["rows"][0]["file"], "src/alpha.rs");
    assert_eq!(v["rows"][0]["score"], 1.0);
    assert_eq!(v["rows"][1]["file"], "src/beta.rs");

    let golden = serde_json::to_string_pretty(&v).expect("serialize golden");
    insta::assert_snapshot!("hotspots_file_grain", golden);

    client.cancel().await.ok();
}

/// Symbol grain ranks `crate::alpha` (churn 5 × complexity 7) above
/// `crate::beta` (churn 2 × complexity 3).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn hotspots_symbol_grain_ranks_by_churn_times_complexity() {
    let (root, _guard) = support::seed_analytics_project();
    let client = support::spawn_client(&root).await;

    let resp = client
        .call_tool(
            CallToolRequestParams::new("hotspots").with_arguments(object!({ "grain": "symbol" })),
        )
        .await
        .expect("call");
    let v: serde_json::Value = serde_json::from_str(&support::extract_text(&resp)).expect("decode");

    assert_eq!(v["rows"][0]["symbol"]["name"], "crate::alpha");
    assert_eq!(v["rows"][0]["score"], 1.0);
    assert_eq!(v["rows"][1]["symbol"]["name"], "crate::beta");

    let golden = serde_json::to_string_pretty(&v).expect("serialize golden");
    insta::assert_snapshot!("hotspots_symbol_grain", golden);

    client.cancel().await.ok();
}
