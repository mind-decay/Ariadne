//! Tier-08 step 9 + Block-1 tier-03 — `blast_radius`: must/may dependents,
//! plus the per-sublist page cap, the single multi-list cursor round-trip
//! (completeness across both lists), and the concise projection that drops the
//! embedded `SymbolSummary` cryptic fields.

mod support;

use rmcp::model::{CallToolRequestParams, JsonObject};
use rmcp::service::RunningService;
use rmcp::{RoleClient, object};
use serde_json::Value;

/// Call `blast_radius` with `args` and return the parsed output object.
async fn blast(client: &RunningService<RoleClient, ()>, args: JsonObject) -> Value {
    let resp = client
        .call_tool(CallToolRequestParams::new("blast_radius").with_arguments(args))
        .await
        .expect("call");
    serde_json::from_str(&support::extract_text(&resp)).expect("decode")
}

/// The `name` field of every row in `out[key]`, in order.
fn names(out: &Value, key: &str) -> Vec<String> {
    out[key]
        .as_array()
        .unwrap_or(&Vec::new())
        .iter()
        .map(|r| r["name"].as_str().expect("name").to_owned())
        .collect()
}

/// Whether every row in `out[key]` carries `field` as a JSON key.
fn rows_have_field(out: &Value, key: &str, field: &str) -> bool {
    out[key]
        .as_array()
        .expect("array")
        .iter()
        .all(|r| r.as_object().expect("object").contains_key(field))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn blast_radius_returns_must_and_may_touch() {
    let (root, _guard) = support::seed_tiny_project();
    let client = support::spawn_client(&root).await;

    let resp = client
        .call_tool(
            CallToolRequestParams::new("blast_radius").with_arguments(object!({
                "symbol": "crate::util::helper",
                "depth": 3,
            })),
        )
        .await
        .expect("call");
    let v: serde_json::Value = serde_json::from_str(&support::extract_text(&resp)).expect("decode");
    let must: Vec<String> = v["must_touch"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["name"].as_str().unwrap().to_owned())
        .collect();
    let may: Vec<String> = v["may_touch"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["name"].as_str().unwrap().to_owned())
        .collect();
    assert!(!must.is_empty() || !may.is_empty());
    // crate::run calls helper; crate::main calls run → must transitively reach.
    let all: Vec<&String> = must.iter().chain(may.iter()).collect();
    assert!(all.iter().any(|n| *n == "crate::run"));

    client.cancel().await.ok();
}

/// The two dependent lists each cap at the per-sublist limit behind ONE shared
/// multi-list cursor; a `limit:1` round-trip reconstructs both un-capped lists
/// with no gap or dup (completeness across sublists, tier-03). On the fixture
/// `crate::util::helper` has two first-hop callers (`must_touch`) and one
/// transitive caller (`may_touch`), so `must_touch` overflows `limit:1` while
/// `may_touch` exhausts — exercising one truncated + one exhausted sublist
/// under a single cursor.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn blast_radius_caps_and_round_trips() {
    let (root, _guard) = support::seed_tiny_project();
    let client = support::spawn_client(&root).await;

    let full = blast(
        &client,
        object!({ "symbol": "crate::util::helper", "limit": 50, "verbosity": "detailed" }),
    )
    .await;
    let must_full = names(&full, "must_touch");
    let may_full = names(&full, "may_touch");
    assert_eq!(must_full.len(), 2, "helper has two first-hop callers");
    assert_eq!(may_full.len(), 1, "helper has one transitive caller");
    assert!(
        full["next_cursor"].is_null(),
        "un-capped result has no cursor"
    );

    let p1 = blast(
        &client,
        object!({ "symbol": "crate::util::helper", "limit": 1, "verbosity": "detailed" }),
    )
    .await;
    assert_eq!(
        names(&p1, "must_touch").len(),
        1,
        "must page caps at the limit"
    );
    assert_eq!(
        names(&p1, "may_touch").len(),
        1,
        "may page (1 item) is exhausted"
    );
    let cursor = p1["next_cursor"]
        .as_str()
        .expect("must_touch overflows → one shared cursor");
    assert!(
        p1["note"].as_str().expect("steer").contains("must_touch"),
        "note names the truncated list",
    );

    let p2 = blast(
        &client,
        object!({ "symbol": "crate::util::helper", "limit": 1, "cursor": cursor, "verbosity": "detailed" }),
    )
    .await;
    assert!(p2["next_cursor"].is_null(), "last page carries no cursor");
    assert!(
        names(&p2, "may_touch").is_empty(),
        "may_touch was exhausted on page 1; its offset is past the end → empty",
    );

    let mut must_union = names(&p1, "must_touch");
    must_union.extend(names(&p2, "must_touch"));
    assert_eq!(
        must_union, must_full,
        "must_touch pages reconstruct the full list"
    );
    let mut may_union = names(&p1, "may_touch");
    may_union.extend(names(&p2, "may_touch"));
    assert_eq!(may_union, may_full, "may_touch fully delivered, no dup");

    client.cancel().await.ok();
}

/// Concise (the default) drops the embedded `SymbolSummary` cryptic fields
/// (`id`, `byte_start`, `byte_end`) from the echo + both lists; detailed keeps
/// them. The semantic `name` survives in both, and the two share the same rows
/// (concise ⊂ detailed) (tier-03).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn blast_radius_concise_drops_cryptic_fields() {
    let (root, _guard) = support::seed_tiny_project();
    let client = support::spawn_client(&root).await;

    let concise = blast(&client, object!({ "symbol": "crate::util::helper" })).await;
    let detailed = blast(
        &client,
        object!({ "symbol": "crate::util::helper", "verbosity": "detailed" }),
    )
    .await;

    assert!(
        !rows_have_field(&concise, "must_touch", "id"),
        "concise must_touch omits the cryptic id",
    );
    assert!(
        !concise["symbol"].as_object().unwrap().contains_key("id"),
        "concise echo omits the cryptic id",
    );
    assert!(
        rows_have_field(&detailed, "must_touch", "id"),
        "detailed must_touch keeps the id (lossless superset)",
    );
    assert!(
        detailed["symbol"]
            .as_object()
            .unwrap()
            .contains_key("byte_start"),
        "detailed echo keeps byte offsets",
    );
    // The semantic identity (name) is identical across verbosities.
    assert_eq!(
        names(&concise, "must_touch"),
        names(&detailed, "must_touch"),
        "concise ⊂ detailed: same rows, fewer fields",
    );

    client.cancel().await.ok();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn blast_radius_resolved_symbol_with_no_callers_echoes_target() {
    let (root, _guard) = support::seed_tiny_project();
    let client = support::spawn_client(&root).await;

    // `crate::main` is the fixture's only fan_in=0 symbol: it resolves in
    // the graph but has zero inbound edges.
    let resp = client
        .call_tool(
            CallToolRequestParams::new("blast_radius").with_arguments(object!({
                "symbol": "crate::main",
            })),
        )
        .await
        .expect("call");
    let v: serde_json::Value = serde_json::from_str(&support::extract_text(&resp)).expect("decode");

    // The resolved target is echoed via the `symbol` field — proof the
    // symbol was found, distinct from a not-found error.
    assert_eq!(
        v["symbol"]["name"].as_str(),
        Some("crate::main"),
        "symbol field must echo the resolved target"
    );
    // Zero inbound edges → both touch sets empty (a true "no dependents"
    // answer, not an absent-symbol failure).
    assert!(
        v["must_touch"]
            .as_array()
            .expect("must_touch array")
            .is_empty(),
        "no callers → empty must_touch"
    );
    assert!(
        v["may_touch"]
            .as_array()
            .expect("may_touch array")
            .is_empty(),
        "no callers → empty may_touch"
    );

    client.cancel().await.ok();
}
