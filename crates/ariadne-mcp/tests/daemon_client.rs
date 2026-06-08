//! Tier-09 — the MCP server as a thin daemon client.
//!
//! These tests assert the three behaviours the tier promises: a query is
//! routed over IPC to the warm daemon (not a cold redb read); the server
//! prefers the daemon when one is reachable; and a missing daemon falls back
//! to the v1 cold path so every tool stays answerable.

mod support;

use ariadne_core::{DaemonQuery, DaemonResponse, SymbolSummary};
use ariadne_mcp::DaemonClient;
use rmcp::model::CallToolRequestParams;
use rmcp::object;

/// A `SymbolSummary` only a daemon could produce here — the cold catalog is
/// seeded empty, so any non-empty answer proves the daemon served it.
fn daemon_symbol(name: &str) -> SymbolSummary {
    SymbolSummary {
        id: Some(42),
        name: name.to_owned(),
        kind: "function".to_owned(),
        file: "src/util.rs".to_owned(),
        byte_start: Some(0),
        byte_end: Some(32),
    }
}

#[test]
fn daemon_client_round_trips_a_query_over_ipc() {
    let (root, _guard) = support::seed_empty_project();
    let _stub = support::spawn_stub_daemon(&root, |q| match q {
        DaemonQuery::FindDefinition { symbol } => DaemonResponse::Definition(daemon_symbol(symbol)),
        _ => DaemonResponse::Error("unexpected query".to_owned()),
    });

    let client = DaemonClient::new(root);
    let resp = client
        .try_query(
            0,
            DaemonQuery::FindDefinition {
                symbol: "crate::util::helper".to_owned(),
            },
        )
        .expect("daemon answered over the socket");

    match resp {
        DaemonResponse::Definition(s) => {
            assert_eq!(s.name, "crate::util::helper");
            assert_eq!(s.id, Some(42));
            assert_eq!(s.file, "src/util.rs");
        }
        other => panic!("expected Definition, got {other:?}"),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn server_serves_from_daemon_not_cold_read() {
    // Cold catalog is EMPTY, so `find_definition` would normally error with
    // "not found". The stub daemon resolves the symbol — a successful, correct
    // answer therefore proves the query was served by the daemon over IPC.
    let (root, _guard) = support::seed_empty_project();
    let _stub = support::spawn_stub_daemon(&root, |q| match q {
        DaemonQuery::FindDefinition { symbol } => DaemonResponse::Definition(daemon_symbol(symbol)),
        _ => DaemonResponse::Error("unexpected query".to_owned()),
    });

    let client = support::spawn_client(&root).await;
    let resp = client
        .call_tool(
            CallToolRequestParams::new("find_definition")
                .with_arguments(object!({ "symbol": "crate::util::helper" })),
        )
        .await
        .expect("call");
    let v: serde_json::Value = serde_json::from_str(&support::extract_text(&resp)).expect("decode");
    assert_eq!(v["name"], "crate::util::helper");
    assert_eq!(v["file"], "src/util.rs");

    client.cancel().await.ok();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn server_cold_fallback_when_daemon_unavailable() {
    // No daemon reachable + auto-spawn off → the server answers from the v1
    // cold redb path, returning the unchanged golden.
    let (root, _guard) = support::seed_tiny_project();
    let client = support::spawn_client(&root).await;
    let resp = client
        .call_tool(
            CallToolRequestParams::new("find_definition")
                .with_arguments(object!({ "symbol": "crate::util::helper" })),
        )
        .await
        .expect("call");
    let v: serde_json::Value = serde_json::from_str(&support::extract_text(&resp)).expect("decode");
    assert_eq!(v["name"], "crate::util::helper");
    assert_eq!(v["file"], "src/util.rs");
    assert_eq!(v["kind"], "function");

    client.cancel().await.ok();
}
