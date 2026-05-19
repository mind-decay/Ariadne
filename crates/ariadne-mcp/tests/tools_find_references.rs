//! Tier-08 step 9 — `find_references`.

mod support;

use rmcp::model::CallToolRequestParams;
use rmcp::object;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn find_references_lists_callers() {
    let (root, _guard) = support::seed_tiny_project();
    let client = support::spawn_client(&root).await;

    let resp = client
        .call_tool(
            CallToolRequestParams::new("find_references")
                .with_arguments(object!({ "symbol": "crate::util::helper" })),
        )
        .await
        .expect("call");
    let rows: Vec<serde_json::Value> =
        serde_json::from_str(&support::extract_text(&resp)).expect("decode");
    let callers: Vec<String> = rows
        .iter()
        .map(|r| r["caller_name"].as_str().unwrap().to_owned())
        .collect();
    assert!(callers.contains(&"crate::run".to_string()));
    assert!(callers.contains(&"crate::helper::extra".to_string()));

    client.cancel().await.ok();
}
