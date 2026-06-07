//! Tier-08 step 1 — golden handshake test. Spawns the `ariadne-mcp` bin,
//! drives the rmcp initialize handshake, lists tools, and asserts the
//! sorted tool names + schema set match an insta golden snapshot.
//!
//! Tier-15 extends this binary with discoverability coverage: a content
//! assertion that every `#[tool]` description carries a `Use when ` clause
//! and the server instructions mention `grep`, plus two regression
//! snapshots (`tools_descriptions`, `server_instructions`) locking the
//! rewritten strings against silent drift.

mod support;

use std::collections::BTreeMap;

/// Every Ariadne `#[tool]` exposed over MCP. Tier-15 asserts the full set
/// is present and each description follows the discoverability template.
/// Tier-15b adds the three Block-C analytics tools (`hotspots`, `complexity`,
/// `co_change`), taking the catalog to 16; tier-15c adds `diff_blast_radius`,
/// taking it to 17; tier-07 adds `search_code`, taking it to 18; tier-08 adds
/// `read_symbol`, taking it to 19; block-a tier-01 adds `affected_tests`,
/// taking it to 20.
const EXPECTED_TOOLS: usize = 20;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn handshake_lists_expected_tools() {
    let (root, _guard) = support::seed_tiny_project();
    let client = support::spawn_client(&root).await;

    let tools = client.list_all_tools().await.expect("list_all_tools");
    let mut by_name: BTreeMap<String, serde_json::Value> = BTreeMap::new();
    for tool in tools {
        let schema = serde_json::to_value(&tool.input_schema).expect("schema json");
        by_name.insert(tool.name.into_owned(), schema);
    }

    let golden = serde_json::to_string_pretty(&by_name).expect("serialize golden");
    insta::assert_snapshot!("tools_list", golden);

    client.cancel().await.ok();
}

/// Tier-15 step 1 — failing-first contract. Each tool description must
/// carry the literal `Use when ` clause and the server instructions must
/// name `grep`; both fail against the tier-08 strings before the rewrite.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn handshake_descriptions_carry_when_and_triggers() {
    let (root, _guard) = support::seed_tiny_project();
    let client = support::spawn_client(&root).await;

    let tools = client.list_all_tools().await.expect("list_all_tools");
    assert_eq!(
        tools.len(),
        EXPECTED_TOOLS,
        "expected all {EXPECTED_TOOLS} Ariadne tools",
    );

    for tool in &tools {
        let desc = tool
            .description
            .as_deref()
            .unwrap_or_else(|| panic!("tool `{}` has no description", tool.name));
        assert!(
            !desc.is_empty(),
            "tool `{}` description is empty",
            tool.name
        );
        assert!(
            desc.contains("Use when "),
            "tool `{}` description lacks a `Use when ` clause: {desc}",
            tool.name,
        );
    }

    let info = client.peer_info().expect("server peer info");
    let instructions = info
        .instructions
        .as_deref()
        .expect("server instructions present");
    assert!(
        instructions.contains("grep"),
        "server instructions never mention `grep`: {instructions}",
    );

    client.cancel().await.ok();
}

/// Tier-01 step 3 — failing-first contract for D2. Every listed tool must
/// carry `_meta {"anthropic/alwaysLoad": true}` so Claude Code keeps the tool
/// always-loaded even when a consumer's `.mcp.json` lacks the server-level
/// `alwaysLoad` flag; fails against the tier-15 tools (no `_meta`) before the
/// per-tool `meta = always_load_meta()` attribute lands.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn handshake_tools_carry_always_load_meta() {
    let (root, _guard) = support::seed_tiny_project();
    let client = support::spawn_client(&root).await;

    let tools = client.list_all_tools().await.expect("list_all_tools");
    assert_eq!(
        tools.len(),
        EXPECTED_TOOLS,
        "expected all {EXPECTED_TOOLS} Ariadne tools",
    );

    for tool in &tools {
        let meta = tool
            .meta
            .as_ref()
            .unwrap_or_else(|| panic!("tool `{}` carries no `_meta`", tool.name));
        assert_eq!(
            meta.get("anthropic/alwaysLoad"),
            Some(&serde_json::Value::Bool(true)),
            "tool `{}` `_meta` lacks `anthropic/alwaysLoad: true`: {meta:?}",
            tool.name,
        );
    }

    client.cancel().await.ok();
}

/// Tier-15 step 4 — regression snapshot of the name→description map.
/// Locks the rewritten `#[tool]` strings against silent drift.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn handshake_snapshots_tool_descriptions() {
    let (root, _guard) = support::seed_tiny_project();
    let client = support::spawn_client(&root).await;

    let tools = client.list_all_tools().await.expect("list_all_tools");
    let mut by_name: BTreeMap<String, String> = BTreeMap::new();
    for tool in tools {
        let desc = tool
            .description
            .map(std::borrow::Cow::into_owned)
            .expect("tool description");
        by_name.insert(tool.name.into_owned(), desc);
    }

    let golden = serde_json::to_string_pretty(&by_name).expect("serialize golden");
    insta::assert_snapshot!("tools_descriptions", golden);

    client.cancel().await.ok();
}

/// Tier-15 step 4 — regression snapshot of the server `instructions`
/// string injected into every session's context.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn handshake_snapshots_server_instructions() {
    let (root, _guard) = support::seed_tiny_project();
    let client = support::spawn_client(&root).await;

    let info = client.peer_info().expect("server peer info");
    let instructions = info
        .instructions
        .clone()
        .expect("server instructions present");
    // Claude Code truncates server instructions at a 2KB cap; the Search/Read
    // line (tier-09) must keep the total under it [src: plan.md `<constraints>`
    // 2KB cap].
    assert!(
        instructions.len() <= 2048,
        "server instructions are {} bytes, over the 2KB cap",
        instructions.len(),
    );
    insta::assert_snapshot!("server_instructions", instructions);

    client.cancel().await.ok();
}
