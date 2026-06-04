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
    // Pre-existing fields are unchanged in name and value.
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

    // Tier-05 additive enrichment: a role one-liner derived from kind + the
    // owning file's hexagonal layer, the file's churn×complexity risk (None
    // here — the tiny fixture carries no Git history), and the blast-radius
    // summary counts at the doc depth (3). `crate::run` and `crate::helper::extra`
    // are the must-touch (funnel) callers, so `blast_must` is 2; `crate::main`
    // reaches `helper` transitively through `run`, so it is a may-touch caller
    // and `blast_may` is 1.
    assert_eq!(v["role"], "function in the interior layer");
    assert!(v["file_risk"].is_null());
    assert_eq!(v["blast_must"], 2);
    assert_eq!(v["blast_may"], 1);

    client.cancel().await.ok();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn doc_for_scope_filters_test_path_neighbours() {
    let (root, _guard) = support::seed_doc_scope_project();
    let client = support::spawn_client(&root).await;

    let resp = client
        .call_tool(
            CallToolRequestParams::new("doc_for")
                .with_arguments(object!({ "symbol": "crate::api::endpoint" })),
        )
        .await
        .expect("call");
    let v: serde_json::Value = serde_json::from_str(&support::extract_text(&resp)).expect("decode");

    let refs: Vec<String> = v["public_refs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["name"].as_str().unwrap().to_owned())
        .collect();
    // The source caller survives the scope filter; the `tests/`-path caller
    // is dropped from `public_refs` even though it is a real graph neighbour.
    assert!(refs.contains(&"crate::api::caller_src".to_owned()));
    assert!(!refs.contains(&"crate::api_callers::caller_test".to_owned()));
    // Blast counts are unfiltered — the scope filter is doc-layer only, so
    // both reverse-1-hop callers still register in the summary.
    assert_eq!(v["blast_must"], 2);
    assert_eq!(v["blast_may"], 0);

    client.cancel().await.ok();
}
