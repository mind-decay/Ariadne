---
tier_id: tier-18
title: LSP hierarchy — call hierarchy, type hierarchy, implementations
deps: [tier-17]
exit_criteria:
  - `ariadne-graph` gains `call_hierarchy`, `type_hierarchy`, and `implementations` graph use cases.
  - The LSP server answers `callHierarchy/{incoming,outgoing}Calls`, `typeHierarchy/{super,sub}types`, `textDocument/implementation`.
  - Each use case is golden-tested independently of the LSP layer.
  - `cargo nextest run -p ariadne-graph -p ariadne-lsp` + architecture + clippy + fmt all green.
status: pending
---

<context>
This tier adds the three hierarchy queries. The graph algorithms live in `ariadne-graph` so tier-19 can reuse them for MCP tools — the LSP layer is just one consumer. Call hierarchy walks call edges; type hierarchy walks inheritance/implementation edges; `implementations` answers "who implements this trait/interface" (plan RD9, Block D). Full context: plan.md.
</context>

<files>
- crates/ariadne-graph/src/hierarchy.rs — new: `call_hierarchy`, `type_hierarchy`, `implementations` use cases over the edge graph.
- crates/ariadne-core/src/domain/ — modify: hierarchy result types (call tree node, type relation, impl list).
- crates/ariadne-lsp/src/adapters/server.rs — modify: register the three hierarchy request groups.
- crates/ariadne-graph/tests/ — new: hierarchy goldens.
- crates/ariadne-lsp/tests/ — new: LSP hierarchy integration goldens.
</files>

<steps>
1. Failing test first (`ariadne-graph` tests): over a fixture with a known call chain and a trait with two impls, assert `call_hierarchy` returns the chain, `type_hierarchy` the super/sub set, `implementations` both impls. Red — `hierarchy.rs` does not exist.
2. Read the v1 `EdgeKind` set (8 variants) to identify which edges encode calls vs inheritance/implementation [src: .claude/plans/ariadne-core/tier-07-graph-analytics.md].
3. `call_hierarchy`: directed traversal of call edges — incoming = predecessors, outgoing = successors — one level per LSP request (the client expands lazily); guard against cycles.
4. `type_hierarchy`: traverse inheritance/implementation edges — supertypes upward, subtypes downward; guard against cycles (diamond inheritance).
5. `implementations`: for a trait/interface symbol, collect every symbol joined by an implementation edge.
6. LSP wiring: `callHierarchy/prepare` then `incomingCalls`/`outgoingCalls`; `typeHierarchy/prepare` then `supertypes`/`subtypes`; `textDocument/implementation` [src: https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/]. Map results via the tier-16 position layer.
7. Goldens at both layers — `ariadne-graph` use case goldens and LSP integration goldens.
</steps>

<verification>
- `cargo nextest run -p ariadne-graph -p ariadne-lsp` — hierarchy use-case goldens + LSP integration goldens green.
- Manual: in VS Code, "Show Call Hierarchy" and "Go to Implementations" on a self-index symbol resolve correctly.
- `cargo test --test architecture`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check` — green.
</verification>

<rollback>
`git checkout -- crates/ariadne-graph crates/ariadne-core crates/ariadne-lsp`. Navigation (tier-17) remains intact.
</rollback>
