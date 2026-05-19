//! Tier-08 step 9 — `plan_assist`.

mod support;

use rmcp::model::CallToolRequestParams;
use rmcp::object;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn plan_assist_returns_ranked_files() {
    let (root, _guard) = support::seed_tiny_project();
    let client = support::spawn_client(&root).await;

    let resp = client
        .call_tool(
            CallToolRequestParams::new("plan_assist")
                .with_arguments(object!({ "symbol": "crate::util::helper", "max_files": 8 })),
        )
        .await
        .expect("call");
    let v: serde_json::Value = serde_json::from_str(&support::extract_text(&resp)).expect("decode");
    let files: Vec<String> = v["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["file"].as_str().unwrap().to_owned())
        .collect();
    assert!(files.contains(&"src/lib.rs".to_string()));
    assert!(files.contains(&"src/main.rs".to_string()));

    client.cancel().await.ok();
}
