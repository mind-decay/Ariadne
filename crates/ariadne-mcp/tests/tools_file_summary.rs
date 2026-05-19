//! Tier-08 step 9 — `file_summary`.

mod support;

use rmcp::model::CallToolRequestParams;
use rmcp::object;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn file_summary_reports_symbols_and_fan() {
    let (root, _guard) = support::seed_tiny_project();
    let client = support::spawn_client(&root).await;

    let resp = client
        .call_tool(
            CallToolRequestParams::new("file_summary")
                .with_arguments(object!({ "path": "src/util.rs" })),
        )
        .await
        .expect("call");
    let v: serde_json::Value = serde_json::from_str(&support::extract_text(&resp)).expect("decode");
    assert_eq!(v["path"], "src/util.rs");
    let names: Vec<String> = v["symbols"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["name"].as_str().unwrap().to_owned())
        .collect();
    assert!(names.contains(&"crate::util::helper".to_string()));
    assert!(names.contains(&"crate::util::leaf".to_string()));
    assert!(v["fan_in"].as_u64().unwrap() >= 1);

    client.cancel().await.ok();
}
