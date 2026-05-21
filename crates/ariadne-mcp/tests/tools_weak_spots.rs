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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn weak_spots_excludes_non_library_god_modules() {
    let (root, _guard) = support::seed_god_module_project();
    let client = support::spawn_client(&root).await;

    let resp = client
        .call_tool(CallToolRequestParams::new("weak_spots").with_arguments(object!({})))
        .await
        .expect("call");
    let v: serde_json::Value = serde_json::from_str(&support::extract_text(&resp)).expect("decode");
    let gods: Vec<String> = v["god_modules"]
        .as_array()
        .expect("god_modules array")
        .iter()
        .map(|m| m["module"].as_str().expect("module name").to_owned())
        .collect();
    // A library file with high efferent coupling is a real god module.
    assert!(
        gods.contains(&"src/hub.rs".to_owned()),
        "library file with high efferent coupling must be flagged, got {gods:?}"
    );
    // The integration-test file is excluded — high fan-out under `tests/`
    // is the expected shape of a test, not an architecture smell.
    assert!(
        !gods.contains(&"tests/big_suite.rs".to_owned()),
        "tests/ file must be excluded from god modules, got {gods:?}"
    );
    // The build script is excluded — `build.rs` is not a library
    // compilation target, so high fan-out there is not a smell either.
    assert!(
        !gods.contains(&"build.rs".to_owned()),
        "build.rs must be excluded from god modules, got {gods:?}"
    );

    client.cancel().await.ok();
}
