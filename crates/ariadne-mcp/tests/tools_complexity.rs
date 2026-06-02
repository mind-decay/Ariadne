//! Tier-15b — `complexity` cold-path golden.
//!
//! Seeds the Block-C analytics fixture and asserts the `McCabe` ranking at both
//! grains: file grain sums each file's symbol complexity (Σ), symbol grain
//! lists each symbol's own complexity, both descending. Spawned with daemon
//! autospawn off so the cold fallback serves the call.

mod support;

use rmcp::model::CallToolRequestParams;
use rmcp::object;

/// File grain returns the per-file Σ `McCabe`, alpha (7) before beta (3).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn complexity_file_grain_sums_per_file_mccabe() {
    let (root, _guard) = support::seed_analytics_project();
    let client = support::spawn_client(&root).await;

    let resp = client
        .call_tool(
            CallToolRequestParams::new("complexity").with_arguments(object!({ "grain": "file" })),
        )
        .await
        .expect("call");
    let v: serde_json::Value = serde_json::from_str(&support::extract_text(&resp)).expect("decode");

    assert_eq!(v["rows"][0]["file"], "src/alpha.rs");
    assert_eq!(v["rows"][0]["complexity"], 7);
    assert_eq!(v["rows"][1]["file"], "src/beta.rs");
    assert_eq!(v["rows"][1]["complexity"], 3);

    let golden = serde_json::to_string_pretty(&v).expect("serialize golden");
    insta::assert_snapshot!("complexity_file_grain", golden);

    client.cancel().await.ok();
}

/// Symbol grain returns each symbol's own `McCabe`, alpha (7) before beta (3).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn complexity_symbol_grain_ranks_per_symbol_mccabe() {
    let (root, _guard) = support::seed_analytics_project();
    let client = support::spawn_client(&root).await;

    let resp = client
        .call_tool(
            CallToolRequestParams::new("complexity").with_arguments(object!({ "grain": "symbol" })),
        )
        .await
        .expect("call");
    let v: serde_json::Value = serde_json::from_str(&support::extract_text(&resp)).expect("decode");

    assert_eq!(v["rows"][0]["symbol"]["name"], "crate::alpha");
    assert_eq!(v["rows"][0]["complexity"], 7);
    assert_eq!(v["rows"][1]["symbol"]["name"], "crate::beta");
    assert_eq!(v["rows"][1]["complexity"], 3);

    let golden = serde_json::to_string_pretty(&v).expect("serialize golden");
    insta::assert_snapshot!("complexity_symbol_grain", golden);

    client.cancel().await.ok();
}
