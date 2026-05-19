//! Tier-08 step 9 — `project_status`.

mod support;

use rmcp::model::CallToolRequestParams;
use rmcp::object;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn project_status_reports_counts() {
    let (root, _guard) = support::seed_tiny_project();
    let client = support::spawn_client(&root).await;

    let resp = client
        .call_tool(CallToolRequestParams::new("project_status").with_arguments(object!({})))
        .await
        .expect("call");
    let v: serde_json::Value = serde_json::from_str(&support::extract_text(&resp)).expect("decode");
    assert!(v["revision"].as_u64().unwrap() >= 1);
    assert_eq!(v["file_count"].as_u64().unwrap(), 4);
    assert_eq!(v["symbol_count"].as_u64().unwrap(), 6);
    assert_eq!(v["edge_count"].as_u64().unwrap(), 6);
    assert!(
        v["root"]
            .as_str()
            .unwrap()
            .contains(root.file_name().unwrap().to_str().unwrap())
    );

    client.cancel().await.ok();
}
