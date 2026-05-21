//! Tier-09 step 4 — `file_summary` component-graph surface.
//!
//! Seeds a Vue SFC project and asserts `file_summary` labels `Component`
//! symbols and carries their rendered children + used hooks. Golden
//! `insta` snapshots lock the wire shape against drift.

mod support;

use rmcp::model::CallToolRequestParams;
use rmcp::object;

/// `file_summary` on a component that both renders a child and uses a hook.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn file_summary_surfaces_component_renders_and_hooks() {
    let (root, _guard) = support::seed_component_project();
    let client = support::spawn_client(&root).await;

    let resp = client
        .call_tool(
            CallToolRequestParams::new("file_summary")
                .with_arguments(object!({ "path": "src/Card.vue" })),
        )
        .await
        .expect("call");
    let v: serde_json::Value = serde_json::from_str(&support::extract_text(&resp)).expect("decode");

    // The Card component carries exactly its rendered child + used hook.
    let component = &v["components"][0];
    assert_eq!(component["component"], "Card");
    assert_eq!(component["renders"][0], "Button");
    assert_eq!(component["hooks"][0], "useToggle");

    let golden = serde_json::to_string_pretty(&v).expect("serialize golden");
    insta::assert_snapshot!("file_summary_card_vue", golden);

    client.cancel().await.ok();
}

/// `file_summary` on a component that only renders — empty `hooks`.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn file_summary_surfaces_render_only_component() {
    let (root, _guard) = support::seed_component_project();
    let client = support::spawn_client(&root).await;

    let resp = client
        .call_tool(
            CallToolRequestParams::new("file_summary")
                .with_arguments(object!({ "path": "src/App.vue" })),
        )
        .await
        .expect("call");
    let v: serde_json::Value = serde_json::from_str(&support::extract_text(&resp)).expect("decode");

    let component = &v["components"][0];
    assert_eq!(component["component"], "App");
    assert_eq!(component["renders"][0], "Card");
    assert!(
        component["hooks"]
            .as_array()
            .expect("hooks array")
            .is_empty(),
        "App uses no hooks",
    );

    let golden = serde_json::to_string_pretty(&v).expect("serialize golden");
    insta::assert_snapshot!("file_summary_app_vue", golden);

    client.cancel().await.ok();
}
