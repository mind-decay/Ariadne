//! Tier-09 step 10 + Block-1 tier-03 — `refactor_suggestions` integration test
//! over a real spawned MCP server + redb fixture: the findings, plus the
//! per-sublist page cap, the single multi-list cursor round-trip (completeness
//! across the three lists), and the concise == detailed invariant (every
//! refactor row is name/metric only, so verbosity is a no-op).

mod support;

use rmcp::model::{CallToolRequestParams, JsonObject};
use rmcp::service::RunningService;
use rmcp::{RoleClient, object};
use serde_json::Value;

/// Call `refactor_suggestions` with `args` and return the parsed output object.
async fn refactor(client: &RunningService<RoleClient, ()>, args: JsonObject) -> Value {
    let resp = client
        .call_tool(CallToolRequestParams::new("refactor_suggestions").with_arguments(args))
        .await
        .expect("call");
    serde_json::from_str(&support::extract_text(&resp)).expect("decode")
}

/// A stable per-row key for each sublist (a god-module's path, a cycle-break's
/// edge, a misplaced symbol's name), in order — for union comparison.
fn keys(out: &Value, list: &str) -> Vec<String> {
    out[list]
        .as_array()
        .expect("array")
        .iter()
        .map(|r| match list {
            "god_modules" => r["module"].as_str().expect("module").to_owned(),
            "cycle_breaks" => format!(
                "{}->{}",
                r["from"].as_str().unwrap(),
                r["to"].as_str().unwrap()
            ),
            _ => r["symbol"].as_str().expect("symbol").to_owned(),
        })
        .collect()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn refactor_suggestions_lists_findings() {
    let (root, _guard) = support::seed_tiny_project();
    let client = support::spawn_client(&root).await;

    let resp = client
        .call_tool(CallToolRequestParams::new("refactor_suggestions").with_arguments(object!({})))
        .await
        .expect("call");
    let v: serde_json::Value = serde_json::from_str(&support::extract_text(&resp)).expect("decode");

    // All three suggestion lists are always present.
    assert!(v["god_modules"].is_array(), "god_modules must be a list");
    assert!(
        v["misplaced_symbols"].is_array(),
        "misplaced_symbols must be a list"
    );

    // The fixture's helper⇄extra cycle yields cycle-break proposals.
    let breaks = v["cycle_breaks"].as_array().expect("cycle_breaks list");
    assert!(!breaks.is_empty(), "expected cycle-break proposals");
    let pairs: Vec<(String, String)> = breaks
        .iter()
        .map(|b| {
            (
                b["from"].as_str().unwrap().to_owned(),
                b["to"].as_str().unwrap().to_owned(),
            )
        })
        .collect();
    assert!(
        pairs.iter().any(|(f, t)| {
            (f == "crate::util::helper" && t == "crate::helper::extra")
                || (f == "crate::helper::extra" && t == "crate::util::helper")
        }),
        "expected a helper⇄extra break edge, got {pairs:?}"
    );
    assert!(
        breaks[0]["rationale"]
            .as_str()
            .unwrap()
            .contains("Dependency-Inversion"),
        "rationale must cite the Dependency-Inversion Principle"
    );

    client.cancel().await.ok();
}

/// The three lists each cap at the per-sublist limit behind ONE shared
/// multi-list cursor; a `limit:1` round-trip reconstructs every un-capped list
/// with no gap or dup (completeness across sublists, tier-03). The god-module
/// fixture's high-efferent files give ≥2 god-module candidates, so that sublist
/// overflows `limit:1` and the page carries a cursor.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn refactor_caps_and_round_trips() {
    let (root, _guard) = support::seed_god_module_project();
    let client = support::spawn_client(&root).await;

    let full = refactor(&client, object!({ "limit": 50 })).await;
    let gods_full = keys(&full, "god_modules");
    let breaks_full = keys(&full, "cycle_breaks");
    let misplaced_full = keys(&full, "misplaced_symbols");
    let max = gods_full
        .len()
        .max(breaks_full.len())
        .max(misplaced_full.len());
    assert!(
        max >= 2,
        "fixture must overflow some sublist at limit:1 (gods {}, breaks {}, misplaced {})",
        gods_full.len(),
        breaks_full.len(),
        misplaced_full.len(),
    );
    assert!(
        full["next_cursor"].is_null(),
        "un-capped result has no cursor"
    );

    let mut gods_seen = Vec::new();
    let mut breaks_seen = Vec::new();
    let mut misplaced_seen = Vec::new();
    let mut cursor: Option<String> = None;
    let mut saw_cursor = false;
    loop {
        let mut args = object!({ "limit": 1 });
        if let Some(c) = &cursor {
            args.insert("cursor".into(), Value::String(c.clone()));
        }
        let page = refactor(&client, args).await;
        gods_seen.extend(keys(&page, "god_modules"));
        breaks_seen.extend(keys(&page, "cycle_breaks"));
        misplaced_seen.extend(keys(&page, "misplaced_symbols"));
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
        gods_seen, gods_full,
        "god_modules pages reconstruct the full list"
    );
    assert_eq!(
        breaks_seen, breaks_full,
        "cycle_breaks pages reconstruct the full list"
    );
    assert_eq!(
        misplaced_seen, misplaced_full,
        "misplaced_symbols pages reconstruct the full list",
    );

    client.cancel().await.ok();
}

/// Every refactor row is name/metric only, so concise (the default) equals
/// detailed byte-for-byte — verbosity is a no-op; the cap is the only economy
/// win (tier-03 exit criterion).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn refactor_concise_equals_detailed() {
    let (root, _guard) = support::seed_tiny_project();
    let client = support::spawn_client(&root).await;

    let concise = refactor(&client, object!({})).await;
    let detailed = refactor(&client, object!({ "verbosity": "detailed" })).await;
    assert_eq!(
        concise, detailed,
        "refactor concise == detailed (no cryptic fields)",
    );

    client.cancel().await.ok();
}
