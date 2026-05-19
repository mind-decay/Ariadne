//! Tier-08 step 1 — golden handshake test. Spawns the `ariadne-mcp` bin,
//! drives the rmcp initialize handshake, lists tools, and asserts the
//! sorted tool names + schema set match an insta golden snapshot.

mod support;

use std::collections::BTreeMap;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn handshake_lists_expected_tools() {
    let (root, _guard) = support::seed_tiny_project();
    let client = support::spawn_client(&root).await;

    let tools = client.list_all_tools().await.expect("list_all_tools");
    let mut by_name: BTreeMap<String, serde_json::Value> = BTreeMap::new();
    for tool in tools {
        let schema = serde_json::to_value(&tool.input_schema).expect("schema json");
        by_name.insert(tool.name.into_owned(), schema);
    }

    let golden = serde_json::to_string_pretty(&by_name).expect("serialize golden");
    insta::assert_snapshot!("tools_list", golden);

    client.cancel().await.ok();
}
