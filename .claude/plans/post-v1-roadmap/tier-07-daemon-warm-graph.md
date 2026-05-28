---
tier_id: tier-07
title: Daemon warm graph — in-RAM petgraph host + IPC query protocol
deps: [tier-06]
exit_criteria:
  - The daemon builds and holds the in-RAM `GraphIndex` after startup.
  - The IPC protocol carries every v1 read query; the daemon dispatches each to `ariadne-graph`.
  - A client connecting with a stale redb revision triggers a daemon graph refresh before the reply.
  - Daemon-served query results are byte-identical to the v1 cold-path goldens.
  - `cargo nextest run -p ariadne-daemon` + architecture + clippy + fmt all green.
status: completed
completed: 2026-05-29
---

<context>
tier-06 gave the daemon a socket and a `Ping`. This tier makes it useful: on startup it builds the warm `GraphIndex` (`ariadne-graph::build_from_snapshot`) and answers real queries over IPC (plan RD6). The watcher loop (tier-08) and the mcp/cli clients (tier-09/10) build on this protocol. Full context: plan.md.
</context>

<files>
- crates/ariadne-core/src/domain/ — modify: extend `DaemonRequest`/`DaemonResponse` with one variant per v1 read query (`list_symbols`, `find_definition`, `find_references`, `blast_radius`, `file_summary`, `plan_assist`, `coupling_report`, `weak_spots`, `doc_for*`, `project_status`, `refactor_suggestions`) + a `revision` handshake field.
- crates/ariadne-daemon/src/domain/ — modify: query dispatch mapping requests to `ariadne-graph` use cases.
- crates/ariadne-daemon/Cargo.toml — modify: add `ariadne-graph`, `ariadne-salsa`, `ariadne-storage` (daemon is the warm-mode composition root, ADR-0007).
- crates/ariadne-daemon/src/adapters/ipc.rs — modify: route framed requests to the dispatcher.
- crates/ariadne-daemon/tests/ — new: protocol round-trip + parity-vs-cold-goldens tests.
- docs/adr/0015-daemon-mode-ipc.md — modify: resolve the deferred IPC-topology question.
</files>

<steps>
1. Failing test first (`ariadne-daemon` tests): start the daemon against a redb fixture, send a `blast_radius` request, assert the response equals the v1 `ariadne-graph` golden for that fixture. Red — the protocol has no query variants.
2. Extend `DaemonRequest`/`DaemonResponse` in `ariadne-core` — one variant per v1 read query, mirroring the existing MCP tool inputs/outputs so no new result shapes are invented. Add a `revision: u64` handshake to every request.
3. Resolve the topology question in ADR-0015: protocol types stay in `ariadne-core`; transport stays in `ariadne-daemon`; clients (tier-09/10/16) embed a thin `daemon_client` module. If client duplication later exceeds one file, ADR-0015 may introduce a shared `ariadne-ipc` crate with an explicit `tests/architecture.rs` exception (precedent ADR-0007). Record the chosen shape now.
4. On daemon startup: open redb, call `ariadne-graph::build_from_snapshot` to construct the warm `GraphIndex`, keep it behind an `RwLock` (MVCC reads, exclusive refresh).
5. Implement the dispatcher: each `DaemonRequest` variant maps to the matching `ariadne-graph` use case against the warm graph; serialize the result into `DaemonResponse`.
6. Handshake: if a request's `revision` is older than the redb revision the daemon last built from, refresh the warm graph before answering (risk R-B2); newer is impossible (clients never lead the daemon).
7. Parity tests: for each query, assert the daemon response equals the v1 cold-path golden on the same fixture.
</steps>

<verification>
- `cargo nextest run -p ariadne-daemon` — protocol round-trip + parity-vs-cold goldens green for every query.
- Manual: start the daemon on the ariadne_v2 self-index; issue a `blast_radius` over the socket; compare to the v1 golden.
- `cargo test --test architecture`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check` — green.
</verification>

<rollback>
`git checkout -- crates docs/adr/0015-daemon-mode-ipc.md`. tier-06 (`Ping` skeleton) remains usable.
</rollback>

<amendment date="2026-05-29">
Post-audit (F2 resolution, user decision): added the two remaining v1 read tools —
`project_status` (`DaemonQuery::ProjectStatus` → `ProjectStatusReport`) and
`refactor_suggestions` (`DaemonQuery::RefactorSuggestions { prefix }` → `RefactorReport`).
The original step-2 enumeration omitted them; the exit criterion ("carries every v1
read query") is now literally satisfied. `WarmCatalog` gained a `root` field (threaded
from `serve`'s `project_root`) to back `project_status`. `refactor_suggestions` mirrors
`ariadne-mcp/src/tools/refactor.rs` against the warm graph + `WarmSnapshot` (god threshold
8.0). Parity tests `project_status_matches_cold` + `refactor_suggestions_matches_cold` added.
This closes audit F2; F1 (salsa) + F3 (typed refresh error) handled separately.
</amendment>
