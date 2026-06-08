//! Tier-08 step 9 + Block-1 tier-01 — `find_references`: caller listing,
//! cursor round-trip completeness, and concise ⊂ detailed verbosity.

mod support;

use rmcp::model::{CallToolRequestParams, JsonObject};
use rmcp::service::RunningService;
use rmcp::{RoleClient, object};
use serde_json::Value;

const SYMBOL: &str = "crate::util::helper";

/// Call `find_references` with `args` and return the parsed output object.
async fn refs(client: &RunningService<RoleClient, ()>, args: JsonObject) -> Value {
    let resp = client
        .call_tool(CallToolRequestParams::new("find_references").with_arguments(args))
        .await
        .expect("call");
    serde_json::from_str(&support::extract_text(&resp)).expect("decode")
}

/// Caller names from a `find_references` output's `references` array.
fn caller_names(out: &Value) -> Vec<String> {
    out["references"]
        .as_array()
        .expect("references array")
        .iter()
        .map(|r| r["caller_name"].as_str().expect("caller_name").to_owned())
        .collect()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn find_references_lists_callers_concise_by_default() {
    let (root, _guard) = support::seed_tiny_project();
    let client = support::spawn_client(&root).await;

    let out = refs(&client, object!({ "symbol": SYMBOL })).await;
    let callers = caller_names(&out);
    assert!(callers.contains(&"crate::run".to_string()));
    assert!(callers.contains(&"crate::helper::extra".to_string()));

    // Concise default: the cryptic id/offset fields are omitted, the semantic
    // name/file are kept (D3).
    let first = &out["references"][0];
    assert!(
        first.get("caller_name").is_some(),
        "concise keeps caller_name"
    );
    assert!(first.get("file").is_some(), "concise keeps file");
    assert!(first.get("caller").is_none(), "concise omits caller id");
    assert!(
        first.get("byte_start").is_none(),
        "concise omits byte_start"
    );
    assert!(first.get("byte_end").is_none(), "concise omits byte_end");

    client.cancel().await.ok();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn find_references_cursor_round_trips_to_full_set() {
    let (root, _guard) = support::seed_tiny_project();
    let client = support::spawn_client(&root).await;

    // Page size 1 over the 2-caller set: page-1 ∪ page-2 == the un-capped set
    // in stable order, no gap or dup.
    let p1 = refs(&client, object!({ "symbol": SYMBOL, "limit": 1 })).await;
    assert_eq!(p1["references"].as_array().expect("array").len(), 1);
    let cursor = p1["next_cursor"].as_str().expect("page-1 has a cursor");
    assert!(
        p1["note"]
            .as_str()
            .expect("page-1 steer")
            .contains("Showing 1 of 2")
    );

    let p2 = refs(
        &client,
        object!({ "symbol": SYMBOL, "limit": 1, "cursor": cursor }),
    )
    .await;
    assert_eq!(p2["references"].as_array().expect("array").len(), 1);
    assert!(p2["next_cursor"].is_null(), "last page carries no cursor");

    let mut union = caller_names(&p1);
    union.extend(caller_names(&p2));
    // Stable (file, …) order: src/helper.rs sorts before src/lib.rs.
    assert_eq!(
        union,
        vec!["crate::helper::extra".to_owned(), "crate::run".to_owned()],
        "the two pages reconstruct the full set with no gap or dup",
    );

    client.cancel().await.ok();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn find_references_concise_is_a_subset_of_detailed() {
    let (root, _guard) = support::seed_tiny_project();
    let client = support::spawn_client(&root).await;

    let concise = refs(&client, object!({ "symbol": SYMBOL })).await;
    let detailed = refs(
        &client,
        object!({ "symbol": SYMBOL, "verbosity": "detailed" }),
    )
    .await;

    let concise_keys = row_keys(&concise);
    let detailed_keys = row_keys(&detailed);
    assert!(
        concise_keys.iter().all(|k| detailed_keys.contains(k)),
        "every concise field is present in detailed: {concise_keys:?} ⊄ {detailed_keys:?}",
    );
    assert!(
        detailed_keys.len() > concise_keys.len(),
        "detailed is a strict superset",
    );
    // Detailed restores the cryptic fields.
    let row = &detailed["references"][0];
    assert!(row.get("caller").is_some());
    assert!(row.get("byte_start").is_some());
    assert!(row.get("byte_end").is_some());

    // Efficiency: concise serializes to fewer bytes than detailed.
    let concise_len = serde_json::to_string(&concise).expect("ser").len();
    let detailed_len = serde_json::to_string(&detailed).expect("ser").len();
    assert!(
        concise_len < detailed_len,
        "concise ({concise_len}) must be smaller than detailed ({detailed_len})",
    );

    client.cancel().await.ok();
}

/// Sorted key set of the first reference row.
fn row_keys(out: &Value) -> Vec<String> {
    out["references"][0]
        .as_object()
        .expect("row object")
        .keys()
        .cloned()
        .collect()
}
