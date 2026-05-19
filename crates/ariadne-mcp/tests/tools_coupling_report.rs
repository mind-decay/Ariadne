//! Tier-08 step 9 — `coupling_report`.

mod support;

use rmcp::model::CallToolRequestParams;
use rmcp::object;

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
