//! Tier-09 step 10 — `refactor_suggestions` integration test over a real
//! spawned MCP server + redb fixture.

mod support;

use rmcp::model::CallToolRequestParams;
use rmcp::object;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn refactor_suggestions_lists_findings() {
    let (root, _guard) = support::seed_tiny_project();
    let client = support::spawn_client(&root).await;

    let resp = client
        .call_tool(CallToolRequestParams::new("refactor_suggestions").with_arguments(object!({})))
        .await
        .expect("call");
    let v: serde_json::Value = serde_json::from_str(&support::extract_text(&resp)).expect("decode");

    // All three suggestion lists are always present.
    assert!(v["god_modules"].is_array(), "god_modules must be a list");
    assert!(
        v["misplaced_symbols"].is_array(),
        "misplaced_symbols must be a list"
    );

    // The fixture's helper⇄extra cycle yields cycle-break proposals.
    let breaks = v["cycle_breaks"].as_array().expect("cycle_breaks list");
    assert!(!breaks.is_empty(), "expected cycle-break proposals");
    let pairs: Vec<(String, String)> = breaks
        .iter()
        .map(|b| {
            (
                b["from"].as_str().unwrap().to_owned(),
                b["to"].as_str().unwrap().to_owned(),
            )
        })
        .collect();
    assert!(
        pairs.iter().any(|(f, t)| {
            (f == "crate::util::helper" && t == "crate::helper::extra")
                || (f == "crate::helper::extra" && t == "crate::util::helper")
        }),
        "expected a helper⇄extra break edge, got {pairs:?}"
    );
    assert!(
        breaks[0]["rationale"]
            .as_str()
            .unwrap()
            .contains("Dependency-Inversion"),
        "rationale must cite the Dependency-Inversion Principle"
    );

    client.cancel().await.ok();
}
