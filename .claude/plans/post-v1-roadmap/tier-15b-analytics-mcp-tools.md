---
tier_id: tier-15b
title: Analytics MCP tools — hotspots, complexity, co_change (daemon-routed, cold fallback)
deps: [tier-15a, tier-13]
exit_criteria:
  - Three MCP tools — `hotspots`, `complexity`, `co_change` — are registered on `AriadneServer`, discoverable, each routing through the daemon client with the tier-09 cold-path fallback.
  - `ariadne-core` gains the matching `DaemonQuery`/`DaemonResponse` variants + mirror report DTOs; the daemon dispatches them through a new `queries/analytics.rs`.
  - `hotspots` ranks by churn×complexity at the requested grain (tier-13 `file_hotspots`/`symbol_hotspots`); `complexity` returns rows ranked by McCabe descending at file (Σ) or symbol grain; `co_change` returns coupling edges honoring the `CoChangeConfig` thresholds (tier-13 `co_change_report`).
  - A daemon/cold JSON-parity unit test (the `server.rs` `project_daemon` pattern) for each new arm, plus a spawned-server insta golden per tool.
  - handshake snapshots re-accepted at 16 tools; each new description carries the literal `Use when ` + a quoted trigger phrase.
  - `cargo nextest run -p ariadne-core -p ariadne-mcp -p ariadne-daemon` + architecture + clippy + fmt all green.
status: completed
completed: 2026-06-02
---

<context>
tier-13 built `file_hotspots`/`symbol_hotspots`/`co_change_report` as pure `ariadne-graph` use cases [src: crates/ariadne-graph/src/hotspot.rs:102,126; co_change.rs:74]; tier-15a loaded their inputs (churn, co-change, complexity) into both catalogs. This tier exposes three read-only analytics as MCP tools, following the exact daemon-routed + cold-fallback shape every v1 tool now uses [src: crates/ariadne-mcp/src/server.rs:184-456] and the 1:1 protocol-mirror convention [src: crates/ariadne-core/src/domain/daemon/{query,response,rows}.rs]. `complexity` has no graph use case — file complexity is the Σ of `SymbolRecord.complexity`, aggregated at this composition root per tier-13 D2. `diff_blast_radius` is tier-15c. Full context: plan.md.
</context>

<decisions>
- D1 — `complexity` is a handler-side aggregation, not a new graph use case. tier-13 D2 deferred file-complexity aggregation ("the composition root aggregates it in tier-15"); the handler folds `catalog.symbols` into per-file Σ or per-symbol rows and ranks descending. *Rejected:* a graph use case (a trivial fold; tier-13 explicitly placed it at the root).
- D2 — one `complexity` tool with a `grain: File|Symbol` input + a `prefix` scope (user-chosen shape). Mirrors `weak_spots`/`coupling_report` scope-prefix ergonomics [src: crates/ariadne-mcp/src/types.rs:198-205; server.rs:313-351]; one tool keeps the catalog at 16 here (17 after 15c) and matches `hotspots`' own grain split. The same `Grain` enum drives `hotspots`.
- D3 — `hotspots` builds the complexity map the use case needs from `catalog.symbols`: file grain → `BTreeMap<String,u32>` of per-file Σ; symbol grain → `BTreeMap<SymbolId,u32>` passthrough; then calls `file_hotspots`/`symbol_hotspots` with `catalog.churn`/`catalog.symbol_churn` [src: hotspot.rs:102-150]. `co_change` calls `co_change_report(&catalog.churn, &catalog.co_change, &cfg)` [src: co_change.rs:74-95].
- D4 — wire DTOs mirror the graph output types field-for-field, in `ariadne-core` daemon `response.rs`/`rows.rs`, exactly as the v1 reports do (tier-13 D1). The daemon projects graph→core wire; the cold tool projects graph→`types.rs` wire; `project_daemon`/`wire` keep both JSON-identical, guarded by a parity unit test [src: server.rs:484-528,530-583].
</decisions>

<files>
- crates/ariadne-core/src/domain/daemon/query.rs — modify: add `Hotspots { prefix, grain }`, `Complexity { prefix, grain }`, `CoChange { prefix, min_revs, min_shared_commits, min_degree }` + a `Grain` enum.
- crates/ariadne-core/src/domain/daemon/{response.rs,rows.rs} — modify: `HotspotReport`/`HotspotRow`, `ComplexityReport`/`ComplexityRow`, `CoChangeReport`/`CoChangeEdge` mirror DTOs + the matching `DaemonResponse` arms.
- crates/ariadne-daemon/src/domain/queries/analytics.rs — new: `hotspots`/`complexity`/`co_change` handlers over `WarmCatalog`.
- crates/ariadne-daemon/src/domain/{queries/mod.rs,dispatch.rs} — modify: declare `analytics`; route the three new queries [src: dispatch.rs:13-42].
- crates/ariadne-mcp/src/types.rs — modify: `Hotspot*`, `Complexity*`, `CoChange*` input/output/row types + `Grain` — all `JsonSchema`.
- crates/ariadne-mcp/src/tools/{hotspots,complexity,co_change}.rs + tools/mod.rs — new/modify: cold `handle` fns over `Catalog`.
- crates/ariadne-mcp/src/server.rs — modify: three `#[tool]` methods (daemon-route + cold fallback) + three `project_daemon` arms; descriptions per the discoverability template.
- crates/ariadne-cli/src/commands/query.rs — modify: `build_query`/`dispatch` arms for the three tools, the forced `project()` arms for the three new `DaemonResponse` variants (the enum is matched exhaustively), and `to_core_grain` mirroring `to_core_kinds` [src: audit F1 — forced exhaustive-match consequence].
- crates/ariadne-mcp/tests/ + snapshots/ — new: spawned-server goldens for the three tools; re-accepted handshake snapshots (16 tools).
- docs/codebase-overview.md — modify: list the three new tools (README + CLAUDE.md catalog finalized in 15c).
</files>

<steps>
1. Failing test first: a spawned-server golden (`support.rs` `spawn_client`, autospawn off → cold path) seeds the 15a fixture, calls `hotspots`/`complexity`/`co_change`, and asserts a stable insta golden. Red — the tools are unregistered [src: crates/ariadne-mcp/tests/support.rs:333-356; tier-09 cold-fallback harness].
2. Add the three `DaemonQuery` variants + `Grain`, the three `DaemonResponse` arms, and the mirror DTOs in `ariadne-core` daemon `query.rs`/`response.rs`/`rows.rs` (every public item doc-commented) [src: query.rs:37-112; response.rs:119-152].
3. Implement `queries/analytics.rs`: `hotspots` builds the grain complexity map from `catalog.symbols` + calls the tier-13 use case; `complexity` folds `catalog.symbols` to ranked rows; `co_change` calls `co_change_report`; each filters by `prefix` and projects to the core wire DTO [src: hotspot.rs:102-150; co_change.rs:74-95]. Wire into `queries/mod.rs` + `dispatch.rs`.
4. Implement the cold `tools/{hotspots,complexity,co_change}.rs` `handle` fns over `Catalog` — identical logic, `types.rs` output shape — so daemon and cold produce byte-identical JSON.
5. Add the three `#[tool]` methods to `server.rs`: build the `DaemonQuery`, `try_query_async` → `project_daemon`, else `catalog()` + cold `handle` + `wire`; add the three `project_daemon` arms; write each `description` to the v1 tier-15 template — what + `Use when …` + quoted triggers [src: server.rs:184-204,484-504; .claude/plans/ariadne-core/tier-15-mcp-discoverability.md `<spec>`].
6. Parity unit test in `server.rs` tests for each new arm (daemon DTO JSON == cold output JSON) [src: server.rs:585-643]. Hand-review every insta golden (no blind `--accept`); re-accept the handshake `tools_list`/`tools_descriptions` snapshots (now 16 tools) and assert each new description contains `Use when ` [src: tier-15-mcp-discoverability.md step 1].
7. Update `docs/codebase-overview.md`'s tool list. Run the full gate; steps 1/6 go green.
</steps>

<verification>
- `cargo nextest run -p ariadne-core -p ariadne-mcp -p ariadne-daemon` — the three tool goldens (cold path) + the daemon/cold parity unit tests + the re-accepted handshake snapshots all green; all v1 tool goldens unchanged.
- Manual (real run, not stub): start `ariadne daemon`, register Ariadne MCP, ask "what are the hotspots in this repo" and "what changes together with src/…"; confirm Claude selects `hotspots`/`co_change` and the warm daemon serves them (kill the daemon → cold fallback still answers). Spot-check the top hotspot against `git log` change-frequency × the file's symbol complexity.
- `cargo test --test architecture` (no new dep edge — `ariadne-mcp`/`ariadne-daemon` already depend on `ariadne-graph`), `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo fmt --all --check`, `RUSTDOCFLAGS=-D warnings cargo doc -p ariadne-core -p ariadne-mcp -p ariadne-daemon --no-deps` — green.
</verification>

<rollback>
`git checkout -- crates docs/codebase-overview.md` and `rm -f crates/ariadne-daemon/src/domain/queries/analytics.rs crates/ariadne-mcp/src/tools/{hotspots,complexity,co_change}.rs` plus the new snapshots. The three tools + their protocol variants are additive; v1 tools and 15a's projection are untouched.
</rollback>
