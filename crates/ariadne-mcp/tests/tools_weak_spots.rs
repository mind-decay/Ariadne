//! Tier-08 step 9 — `weak_spots`.

mod support;

use rmcp::model::CallToolRequestParams;
use rmcp::object;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn weak_spots_lists_cycles_and_dead_code() {
    let (root, _guard) = support::seed_tiny_project();
    let client = support::spawn_client(&root).await;

    let resp = client
        .call_tool(CallToolRequestParams::new("weak_spots").with_arguments(object!({})))
        .await
        .expect("call");
    let v: serde_json::Value = serde_json::from_str(&support::extract_text(&resp)).expect("decode");
    // Fixture has a cycle helper -> extra -> helper.
    let cycles = v["cycles"].as_array().expect("cycles");
    let members: Vec<Vec<String>> = cycles
        .iter()
        .map(|c| {
            c["members"]
                .as_array()
                .unwrap()
                .iter()
                .map(|m| m.as_str().unwrap().to_owned())
                .collect()
        })
        .collect();
    assert!(
        members
            .iter()
            .any(|c| c.contains(&"crate::util::helper".into())
                && c.contains(&"crate::helper::extra".into())),
        "expected helper⇄extra cycle, got {members:?}"
    );
    let dead: Vec<String> = v["dead_symbols"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["name"].as_str().unwrap().to_owned())
        .collect();
    // The fixture's only fan_in=0 symbol is `crate::main` (every other
    // symbol has at least one incoming edge).
    assert_eq!(dead, vec!["crate::main"]);

    client.cancel().await.ok();
}
