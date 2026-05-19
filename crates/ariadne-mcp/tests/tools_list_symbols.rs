//! Tier-08 step 9 — `list_symbols` integration. Spawns the bin and
//! verifies the substring filter surfaces the seeded fixture symbols.

mod support;

use rmcp::model::CallToolRequestParams;
use rmcp::object;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn list_symbols_filters_by_substring() {
    let (root, _guard) = support::seed_tiny_project();
    let client = support::spawn_client(&root).await;

    let resp = client
        .call_tool(
            CallToolRequestParams::new("list_symbols").with_arguments(object!({
                "query": "helper",
            })),
        )
        .await
        .expect("list_symbols");
    let text = support::extract_text(&resp);
    let names: Vec<String> = serde_json::from_str::<Vec<serde_json::Value>>(&text)
        .expect("decode list_symbols")
        .into_iter()
        .map(|v| v["name"].as_str().unwrap().to_owned())
        .collect();
    assert!(names.iter().any(|n| n == "crate::util::helper"));
    assert!(names.iter().any(|n| n == "crate::helper::extra"));
    assert!(!names.iter().any(|n| n == "crate::main"));

    client.cancel().await.ok();
}
