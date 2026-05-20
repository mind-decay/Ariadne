//! Tier-09 step 10 — `doc_for_module` / `doc_for_project` integration
//! tests over a real spawned MCP server + redb fixture.

mod support;

use rmcp::model::CallToolRequestParams;
use rmcp::object;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn doc_for_module_renders_markdown() {
    let (root, _guard) = support::seed_tiny_project();
    let client = support::spawn_client(&root).await;

    let resp = client
        .call_tool(
            CallToolRequestParams::new("doc_for_module")
                .with_arguments(object!({ "path": "src/util.rs" })),
        )
        .await
        .expect("call");
    let v: serde_json::Value = serde_json::from_str(&support::extract_text(&resp)).expect("decode");
    let md = v["markdown"].as_str().expect("markdown field");
    assert!(
        md.contains("# Module `src/util.rs`"),
        "missing module header, got:\n{md}"
    );
    assert!(md.contains("## Public API"), "missing Public API section");
    assert!(md.contains("## Cycles"), "missing Cycles section");
    assert!(
        md.contains("crate::util::helper"),
        "module symbol absent from doc"
    );

    client.cancel().await.ok();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn doc_for_project_renders_overview() {
    let (root, _guard) = support::seed_tiny_project();
    let client = support::spawn_client(&root).await;

    let resp = client
        .call_tool(CallToolRequestParams::new("doc_for_project").with_arguments(object!({})))
        .await
        .expect("call");
    let v: serde_json::Value = serde_json::from_str(&support::extract_text(&resp)).expect("decode");
    let md = v["markdown"].as_str().expect("markdown field");
    assert!(
        md.contains("# Project Architecture Overview"),
        "missing overview header, got:\n{md}"
    );
    assert!(md.contains("flowchart TD"), "missing Mermaid layer diagram");
    assert!(md.contains("## Hot-Spots"), "missing Hot-Spots section");

    client.cancel().await.ok();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn doc_for_project_empty_has_placeholder() {
    let (root, _guard) = support::seed_empty_project();
    let client = support::spawn_client(&root).await;

    let resp = client
        .call_tool(CallToolRequestParams::new("doc_for_project").with_arguments(object!({})))
        .await
        .expect("call");
    let v: serde_json::Value = serde_json::from_str(&support::extract_text(&resp)).expect("decode");
    let md = v["markdown"].as_str().expect("markdown field");
    assert!(
        md.contains("_No modules indexed._"),
        "empty project must render a placeholder, not an error; got:\n{md}"
    );

    client.cancel().await.ok();
}
