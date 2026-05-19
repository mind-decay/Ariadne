//! Tier-08 step 9 — `find_definition`.

mod support;

use rmcp::model::CallToolRequestParams;
use rmcp::object;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn find_definition_returns_seeded_symbol() {
    let (root, _guard) = support::seed_tiny_project();
    let client = support::spawn_client(&root).await;

    let resp = client
        .call_tool(
            CallToolRequestParams::new("find_definition")
                .with_arguments(object!({ "symbol": "crate::util::helper" })),
        )
        .await
        .expect("call");
    let v: serde_json::Value = serde_json::from_str(&support::extract_text(&resp)).expect("decode");
    assert_eq!(v["name"], "crate::util::helper");
    assert_eq!(v["file"], "src/util.rs");
    assert_eq!(v["kind"], "function");

    let err = client
        .call_tool(
            CallToolRequestParams::new("find_definition")
                .with_arguments(object!({ "symbol": "nope" })),
        )
        .await
        .expect_err("missing symbol must error");
    assert!(err.to_string().contains("not found"), "got {err}");

    client.cancel().await.ok();
}
