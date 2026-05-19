//! Tier-08 step 9 — `doc_for`.

mod support;

use rmcp::model::CallToolRequestParams;
use rmcp::object;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn doc_for_returns_signature_and_refs() {
    let (root, _guard) = support::seed_tiny_project();
    let client = support::spawn_client(&root).await;

    let resp = client
        .call_tool(
            CallToolRequestParams::new("doc_for")
                .with_arguments(object!({ "symbol": "crate::util::helper" })),
        )
        .await
        .expect("call");
    let v: serde_json::Value = serde_json::from_str(&support::extract_text(&resp)).expect("decode");
    assert_eq!(v["signature"], "function crate::util::helper");
    assert_eq!(v["file"], "src/util.rs");
    assert_eq!(v["kind"], "function");
    let refs: Vec<String> = v["public_refs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["name"].as_str().unwrap().to_owned())
        .collect();
    assert!(
        refs.iter()
            .any(|n| n == "crate::run" || n == "crate::helper::extra")
    );

    client.cancel().await.ok();
}
