//! Tier-02 — lazy cold catalog.
//!
//! Proves the in-RAM [`Catalog`] is built lazily: `build_server` leaves it
//! unbuilt (session-open no longer reads the full index), a reachable daemon
//! answers tool calls without ever building it, and the first cold access
//! builds it exactly once and answers from the seeded redb index.
//!
//! The warm-path test drives the *real* `#[tool]` routing over an in-process
//! tokio duplex pipe — the same daemon-first decision the stdio binary runs —
//! so a tool call exercises the production route-or-build path. The catalog
//! cell is observed through a clone of the server (the cell is an
//! `Arc<OnceCell<…>>` shared across clones), so the handle moved into the
//! server task and the observing handle see the same state.
//!
//! The cold async build path (`self.catalog().await` inside a tool arm) is
//! additionally exercised end-to-end by
//! `daemon_client::server_cold_fallback_when_daemon_unavailable`, which runs
//! the real binary with auto-spawn disabled.

mod support;

use std::sync::Arc;

use ariadne_core::{DaemonQuery, DaemonResponse, SymbolSummary};
use ariadne_mcp::tools::list_symbols;
use ariadne_mcp::types::ListSymbolsInput;
use ariadne_mcp::{AriadneServer, ServeOpts, build_server};
use rmcp::model::CallToolRequestParams;
use rmcp::service::RunningService;
use rmcp::{RoleClient, RoleServer, ServiceExt, object};

/// Connect a client/server pair over an in-process duplex pipe and run the
/// MCP initialize handshake. Returns the connected client peer plus the
/// running server handle; both must be kept alive for the duration of the
/// calls.
async fn connect(
    server: AriadneServer,
) -> (
    RunningService<RoleClient, ()>,
    RunningService<RoleServer, AriadneServer>,
) {
    let (client_io, server_io) = tokio::io::duplex(64 * 1024);
    // Spawn the server handshake concurrently — initialize is bidirectional,
    // so the client `serve` below cannot complete until the server is reading.
    let server_fut = tokio::spawn(async move { server.serve(server_io).await });
    let client = ().serve(client_io).await.expect("client initialize");
    let running = server_fut
        .await
        .expect("server task join")
        .expect("server initialize");
    (client, running)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reachable_daemon_answers_without_building_catalog() {
    // Cold catalog seeded EMPTY; the stub daemon resolves the query. A correct
    // answer proves the daemon served it over IPC, and the still-unbuilt cell
    // proves the cold catalog was never constructed on the warm path.
    let (root, _guard) = support::seed_empty_project();
    let _stub = support::spawn_stub_daemon(&root, |q| match q {
        DaemonQuery::ListSymbols { .. } => DaemonResponse::Symbols(vec![SymbolSummary {
            id: Some(42),
            name: "crate::from_daemon".to_owned(),
            kind: "function".to_owned(),
            file: "src/x.rs".to_owned(),
            byte_start: Some(0),
            byte_end: Some(8),
        }]),
        _ => DaemonResponse::Error("unexpected query".to_owned()),
    });

    let server = build_server(&ServeOpts::new(&root))
        .await
        .expect("build server");
    assert!(
        !server.catalog_built(),
        "build_server must not build the catalog eagerly"
    );
    let observer = server.clone();

    let (client, _running) = connect(server).await;
    let resp = client
        .call_tool(
            CallToolRequestParams::new("list_symbols")
                .with_arguments(object!({ "query": "from_daemon" })),
        )
        .await
        .expect("call list_symbols");
    let v: serde_json::Value =
        serde_json::from_str(&support::extract_text(&resp)).expect("decode list_symbols");
    assert_eq!(
        v[0]["name"], "crate::from_daemon",
        "daemon served the answer"
    );
    assert!(
        !observer.catalog_built(),
        "a daemon-served tool call must not build the cold catalog"
    );

    client.cancel().await.ok();
}

#[tokio::test]
async fn cold_access_builds_catalog_once_and_answers() {
    // `build_server` leaves the catalog unbuilt; the first cold access builds
    // it exactly once (the same `Catalog::build` the cold-fallback tool arm
    // triggers) and the result answers correctly from the seeded index.
    let (root, _guard) = support::seed_tiny_project();
    let server = build_server(&ServeOpts::new(&root))
        .await
        .expect("build server");
    assert!(
        !server.catalog_built(),
        "build_server must not build the catalog eagerly"
    );

    let cat = server.catalog_arc().await;
    assert!(
        server.catalog_built(),
        "the first cold access built the catalog"
    );
    let cat_again = server.catalog_arc().await;
    assert!(
        Arc::ptr_eq(&cat, &cat_again),
        "the catalog is built exactly once (OnceCell idempotent)"
    );

    // Exercise the exact handler the cold-fallback arm calls, against the
    // lazily-built catalog.
    let out = list_symbols::handle(
        &cat,
        &ListSymbolsInput {
            query: "helper".to_owned(),
            kind: None,
            limit: Some(64),
        },
    );
    let names: Vec<&str> = out.iter().map(|s| s.name.as_str()).collect();
    assert!(
        names.iter().any(|n| n.contains("helper")),
        "cold catalog answered from the seeded index: {names:?}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_cold_access_shares_one_catalog_build() {
    // Race-free contract (INFO-1): many first callers hitting an unbuilt
    // server must share the single `get_or_try_init` build `catalog_arc` now
    // routes through, never each running `Catalog::build`. The observable
    // invariant is that every returned handle is the same `Arc` instance.
    let (root, _guard) = support::seed_tiny_project();
    let server = build_server(&ServeOpts::new(&root))
        .await
        .expect("build server");
    assert!(
        !server.catalog_built(),
        "build_server must not build the catalog eagerly"
    );

    let mut handles = Vec::new();
    for _ in 0..8 {
        let s = server.clone();
        handles.push(tokio::spawn(async move { s.catalog_arc().await }));
    }
    let mut cats = Vec::new();
    for h in handles {
        cats.push(h.await.expect("task join"));
    }

    let first = &cats[0];
    for c in &cats[1..] {
        assert!(
            Arc::ptr_eq(first, c),
            "all concurrent cold accesses share one catalog build"
        );
    }
    assert!(
        server.catalog_built(),
        "the race-free build populated the shared cell"
    );
}
