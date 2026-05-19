//! Tier-08 step 9 — `blast_radius`.

mod support;

use rmcp::model::CallToolRequestParams;
use rmcp::object;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn blast_radius_returns_must_and_may_touch() {
    let (root, _guard) = support::seed_tiny_project();
    let client = support::spawn_client(&root).await;

    let resp = client
        .call_tool(
            CallToolRequestParams::new("blast_radius").with_arguments(object!({
                "symbol": "crate::util::helper",
                "depth": 3,
            })),
        )
        .await
        .expect("call");
    let v: serde_json::Value = serde_json::from_str(&support::extract_text(&resp)).expect("decode");
    let must: Vec<String> = v["must_touch"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["name"].as_str().unwrap().to_owned())
        .collect();
    let may: Vec<String> = v["may_touch"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["name"].as_str().unwrap().to_owned())
        .collect();
    assert!(!must.is_empty() || !may.is_empty());
    // crate::run calls helper; crate::main calls run → must transitively reach.
    let all: Vec<&String> = must.iter().chain(may.iter()).collect();
    assert!(all.iter().any(|n| *n == "crate::run"));

    client.cancel().await.ok();
}
