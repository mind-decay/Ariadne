//! Tier-08 step 9 + Block-1 tier-03 — `weak_spots`: cycles ∪ god modules ∪
//! dead code, plus the per-sublist page cap, the single multi-list cursor
//! round-trip (completeness across the three lists), and the concise projection
//! that drops the embedded dead-symbol cryptic fields while leaving the
//! name/metric-only cycle + god-module rows unchanged.

mod support;

use rmcp::model::{CallToolRequestParams, JsonObject};
use rmcp::service::RunningService;
use rmcp::{RoleClient, object};
use serde_json::Value;

/// Call `weak_spots` with `args` and return the parsed output object.
async fn weak(client: &RunningService<RoleClient, ()>, args: JsonObject) -> Value {
    let resp = client
        .call_tool(CallToolRequestParams::new("weak_spots").with_arguments(args))
        .await
        .expect("call");
    serde_json::from_str(&support::extract_text(&resp)).expect("decode")
}

/// A stable per-row key for each sublist (a cycle's joined members, a
/// god-module's path, a dead symbol's name), in order — for union comparison.
fn keys(out: &Value, list: &str) -> Vec<String> {
    out[list]
        .as_array()
        .expect("array")
        .iter()
        .map(|r| match list {
            "cycles" => r["members"]
                .as_array()
                .unwrap()
                .iter()
                .map(|m| m.as_str().unwrap())
                .collect::<Vec<_>>()
                .join(","),
            "god_modules" => r["module"].as_str().expect("module").to_owned(),
            _ => r["name"].as_str().expect("name").to_owned(),
        })
        .collect()
}

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
    // tier-05 root classifier excludes `crate::main` (Rust `fn main`
    // convention); the genuinely dead non-root `crate::unused_helper`
    // still surfaces. The other symbols have at least one incoming edge.
    assert_eq!(dead, vec!["crate::unused_helper"]);

    client.cancel().await.ok();
}

/// The three lists each cap at the per-sublist limit behind ONE shared
/// multi-list cursor; a `limit:1` round-trip reconstructs every un-capped list
/// with no gap or dup (completeness across sublists, tier-03). The god-module
/// fixture's three fan-in-0 hub symbols are dead, so `dead_symbols` overflows
/// `limit:1` and the page carries a cursor.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn weak_spots_caps_and_round_trips() {
    let (root, _guard) = support::seed_god_module_project();
    let client = support::spawn_client(&root).await;

    let full = weak(&client, object!({ "limit": 50, "verbosity": "detailed" })).await;
    let cycles_full = keys(&full, "cycles");
    let gods_full = keys(&full, "god_modules");
    let dead_full = keys(&full, "dead_symbols");
    let max = cycles_full.len().max(gods_full.len()).max(dead_full.len());
    assert!(
        max >= 2,
        "fixture must overflow some sublist at limit:1 (cycles {}, gods {}, dead {})",
        cycles_full.len(),
        gods_full.len(),
        dead_full.len(),
    );
    assert!(
        full["next_cursor"].is_null(),
        "un-capped result has no cursor"
    );

    let mut cycles_seen = Vec::new();
    let mut gods_seen = Vec::new();
    let mut dead_seen = Vec::new();
    let mut cursor: Option<String> = None;
    let mut saw_cursor = false;
    loop {
        let mut args = object!({ "limit": 1, "verbosity": "detailed" });
        if let Some(c) = &cursor {
            args.insert("cursor".into(), Value::String(c.clone()));
        }
        let page = weak(&client, args).await;
        cycles_seen.extend(keys(&page, "cycles"));
        gods_seen.extend(keys(&page, "god_modules"));
        dead_seen.extend(keys(&page, "dead_symbols"));
        match page["next_cursor"].as_str() {
            Some(c) => {
                saw_cursor = true;
                cursor = Some(c.to_owned());
            }
            None => break,
        }
    }
    assert!(saw_cursor, "an overflowing sublist must mint a cursor");
    assert_eq!(
        cycles_seen, cycles_full,
        "cycles pages reconstruct the full list"
    );
    assert_eq!(
        gods_seen, gods_full,
        "god_modules pages reconstruct the full list"
    );
    assert_eq!(
        dead_seen, dead_full,
        "dead_symbols pages reconstruct the full list"
    );

    client.cancel().await.ok();
}

/// Concise (the default) drops the dead-symbol rows' cryptic fields while the
/// cycle (name-only) and god-module (metric-only) lists are byte-identical to
/// detailed — those carry no cryptic fields (tier-03 exit criterion).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn weak_spots_concise_drops_dead_ids_only() {
    let (root, _guard) = support::seed_god_module_project();
    let client = support::spawn_client(&root).await;

    let concise = weak(&client, object!({})).await;
    let detailed = weak(&client, object!({ "verbosity": "detailed" })).await;

    assert!(
        !concise["dead_symbols"]
            .as_array()
            .expect("array")
            .iter()
            .any(|r| r.as_object().unwrap().contains_key("id")),
        "concise dead_symbols omit the cryptic id",
    );
    assert!(
        detailed["dead_symbols"]
            .as_array()
            .expect("array")
            .iter()
            .all(|r| r.as_object().unwrap().contains_key("id")),
        "detailed dead_symbols keep the id (lossless superset)",
    );
    assert_eq!(
        concise["cycles"], detailed["cycles"],
        "cycles are name-only: concise == detailed",
    );
    assert_eq!(
        concise["god_modules"], detailed["god_modules"],
        "god_modules are metric-only: concise == detailed",
    );

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
