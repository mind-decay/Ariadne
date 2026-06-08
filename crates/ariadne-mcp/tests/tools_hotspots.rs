//! Tier-15b — `hotspots` cold-path golden.
//!
//! Seeds the Block-C analytics fixture (`seed_analytics_project`), spawns the
//! `ariadne-mcp` binary with daemon autospawn off (so the routing path is the
//! cold fallback), and locks the churn × complexity ranking at both grains
//! against an `insta` golden. The same fixture backs the warm daemon
//! projection, so cold and warm output are byte-identical.

mod support;

use rmcp::model::{CallToolRequestParams, JsonObject};
use rmcp::service::RunningService;
use rmcp::{RoleClient, object};
use serde_json::Value;

/// Call `hotspots` with `args` and return the parsed output object.
async fn hotspots(client: &RunningService<RoleClient, ()>, args: JsonObject) -> Value {
    let resp = client
        .call_tool(CallToolRequestParams::new("hotspots").with_arguments(args))
        .await
        .expect("call");
    serde_json::from_str(&support::extract_text(&resp)).expect("decode")
}

/// File paths from a file-grain `hotspots` output's `rows`, in order.
fn file_seq(out: &Value) -> Vec<String> {
    out["rows"]
        .as_array()
        .expect("rows array")
        .iter()
        .map(|r| r["file"].as_str().expect("file").to_owned())
        .collect()
}

/// A `limit`-1 file-grain page over the two-file fixture round-trips:
/// page-1 ∪ page-2 equals the un-capped (score desc, file asc) set, no dup/gap.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn hotspots_caps_and_round_trips() {
    let (root, _guard) = support::seed_analytics_project();
    let client = support::spawn_client(&root).await;

    let full = hotspots(&client, object!({ "grain": "file", "limit": 50 })).await;
    let full_seq = file_seq(&full);
    assert_eq!(full_seq, vec!["src/alpha.rs", "src/beta.rs"]);

    let p1 = hotspots(&client, object!({ "grain": "file", "limit": 1 })).await;
    assert_eq!(file_seq(&p1), vec!["src/alpha.rs"]);
    let cursor = p1["next_cursor"].as_str().expect("page-1 has a cursor");
    assert!(
        p1["note"]
            .as_str()
            .expect("steer")
            .contains("Showing 1 of 2")
    );

    let p2 = hotspots(
        &client,
        object!({ "grain": "file", "limit": 1, "cursor": cursor }),
    )
    .await;
    assert!(p2["next_cursor"].is_null(), "last page carries no cursor");

    let mut union = file_seq(&p1);
    union.extend(file_seq(&p2));
    assert_eq!(
        union, full_seq,
        "the two pages reconstruct the full set, no gap or dup"
    );

    client.cancel().await.ok();
}

/// Symbol-grain concise (the default) omits the embedded symbol's cryptic
/// id/offset fields, keeping the semantic name/kind/file; detailed restores
/// them as a lossless superset (tier-02 D3 / exit criterion).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn hotspots_symbol_grain_concise_omits_symbol_ids() {
    let (root, _guard) = support::seed_analytics_project();
    let client = support::spawn_client(&root).await;

    let concise = hotspots(&client, object!({ "grain": "symbol" })).await;
    let sym = &concise["rows"][0]["symbol"];
    assert!(sym.get("name").is_some(), "concise keeps the semantic name");
    assert!(sym.get("kind").is_some(), "concise keeps kind");
    assert!(sym.get("file").is_some(), "concise keeps file");
    assert!(
        sym.get("id").is_none(),
        "concise omits the cryptic symbol id"
    );
    assert!(sym.get("byte_start").is_none(), "concise omits byte_start");
    assert!(sym.get("byte_end").is_none(), "concise omits byte_end");

    let detailed = hotspots(
        &client,
        object!({ "grain": "symbol", "verbosity": "detailed" }),
    )
    .await;
    let dsym = &detailed["rows"][0]["symbol"];
    assert!(dsym.get("id").is_some(), "detailed restores the symbol id");
    assert!(
        dsym.get("byte_start").is_some(),
        "detailed restores byte_start"
    );
    assert!(dsym.get("byte_end").is_some(), "detailed restores byte_end");

    client.cancel().await.ok();
}

/// File grain ranks `src/alpha.rs` (churn 9 × Σ-complexity 7) above
/// `src/beta.rs` (churn 4 × Σ-complexity 3).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn hotspots_file_grain_ranks_by_churn_times_complexity() {
    let (root, _guard) = support::seed_analytics_project();
    let client = support::spawn_client(&root).await;

    let resp = client
        .call_tool(
            CallToolRequestParams::new("hotspots").with_arguments(object!({ "grain": "file" })),
        )
        .await
        .expect("call");
    let v: serde_json::Value = serde_json::from_str(&support::extract_text(&resp)).expect("decode");

    // Strongest hotspot first: alpha (score 1.0) before beta.
    assert_eq!(v["rows"][0]["file"], "src/alpha.rs");
    assert_eq!(v["rows"][0]["score"], 1.0);
    assert_eq!(v["rows"][1]["file"], "src/beta.rs");

    let golden = serde_json::to_string_pretty(&v).expect("serialize golden");
    insta::assert_snapshot!("hotspots_file_grain", golden);

    client.cancel().await.ok();
}

/// Symbol grain ranks `crate::alpha` (churn 5 × complexity 7) above
/// `crate::beta` (churn 2 × complexity 3).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn hotspots_symbol_grain_ranks_by_churn_times_complexity() {
    let (root, _guard) = support::seed_analytics_project();
    let client = support::spawn_client(&root).await;

    let resp = client
        .call_tool(
            CallToolRequestParams::new("hotspots").with_arguments(object!({ "grain": "symbol" })),
        )
        .await
        .expect("call");
    let v: serde_json::Value = serde_json::from_str(&support::extract_text(&resp)).expect("decode");

    assert_eq!(v["rows"][0]["symbol"]["name"], "crate::alpha");
    assert_eq!(v["rows"][0]["score"], 1.0);
    assert_eq!(v["rows"][1]["symbol"]["name"], "crate::beta");

    let golden = serde_json::to_string_pretty(&v).expect("serialize golden");
    insta::assert_snapshot!("hotspots_symbol_grain", golden);

    client.cancel().await.ok();
}
