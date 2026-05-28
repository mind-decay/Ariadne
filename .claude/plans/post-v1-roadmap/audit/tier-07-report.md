---
tier_id: tier-07
audited: 2026-05-29
verdict: PASS
commit: 623b4b247b6714c252b5e32a06a20db94d360d2d
---

# tier-07 audit — Daemon warm graph (in-RAM petgraph host + IPC query protocol)

<scope>
Reviewed the working-tree diff for tier-07 `<files>`:
- `crates/ariadne-core/src/domain/daemon/{mod,query,response,rows}.rs` — `DaemonRequest` (+`revision: u64`), `DaemonQuery` (Ping + 11 read queries), `DaemonResponse`, report/row payloads.
- `crates/ariadne-daemon/src/domain/{catalog,snapshot,dispatch}.rs` + `queries/{navigate,impact,health,docs}.rs` — `WarmCatalog`, `WarmSnapshot` (`ReadSnapshot` RAM mirror), dispatcher, per-query handlers.
- `crates/ariadne-daemon/src/adapters/ipc.rs` — warm-graph load + `RwLock` host + `serve_connection` staleness refresh + public `query`.
- `crates/ariadne-daemon/Cargo.toml`, `crates/ariadne-daemon/tests/{warm_graph,warm_analytics,support}.rs`, `docs/adr/0015-daemon-mode-ipc.md`.

Note: tier-07 sits on an **uncommitted tier-06** bundle (HEAD = tier-05 `623b4b2`); the CLI `daemon` subcommand files in the working tree are tier-06 scope (already audited PASS) and were excluded from this review. All six tier-07 `<files>` entries were touched; nothing outside them is tier-07-attributable.
</scope>

<checks_run>
- **fmt**: `cargo fmt --all --check` → clean (exit 0).
- **daemon tests**: `cargo nextest run -p ariadne-daemon` → **15/15 pass** (3 lifecycle unit + 2 tier-06 socket + 10 tier-07 parity/refresh).
- **architecture**: `cargo test --test architecture` → pass; `ariadne-daemon` is in `DRIVING_ADAPTERS`, and its new `ariadne-graph`/`ariadne-storage` deps are permitted for a driving adapter (only the composition root may depend on it).
- **clippy**: `cargo clippy --workspace --all-targets -- -D warnings` → exit 0, no warnings (full workspace compiles incl. all test targets).
- **Parity (read each handler vs v1 MCP)**: `navigate`/`impact`/`health`/`docs` handlers are faithful copies of the MCP `tools/*` handlers, substituting `WarmCatalog`+`WarmSnapshot` for `Catalog`+redb snapshot. Defaults (limit 64, depth 3, max_files 16, MAX_DEAD 16, MAX_PUBLIC_REFS 16, GOD_THRESHOLD 15, COMPONENT_KIND), sort/dedup order, and `<unknown>` placeholders all match. `WarmSnapshot` preserves storage `(src,kind,dst)` scan order in `out_idx`/`in_idx`/`file_edge_idx`, so edge-order-sensitive results (`find_references` last-edge-wins, `file_summary`) and byte-identical `docgen` markdown are preserved — confirmed by `doc_markdown_matches_cold`.
- **Staleness handshake**: `is_stale(rev) = rev > self.revision` correctly implements the exit criterion ("stale redb revision triggers a refresh") and risk R-B2; `stale_revision_triggers_refresh` proves no-refresh at the built revision and refresh+resolve after the index advances. Plan step-6 prose ("older → refresh") is loosely worded but the binding exit criterion + ADR-0015 + the implementation agree.
- **Real-index run**: started/served/stopped the daemon on an isolated `init`+`index`ed temp project (1 file/4 symbols/2 edges) → `Running` then clean `Stopped`, no residue; warm `GraphIndex` builds from a real redb snapshot, not just fixtures. Cold `blast_radius(helper)` → `run`, matching the warm parity test's golden shape.
- **Manual self-index step**: blocked — this session's MCP server (`ariadne serve --watch`, pid 53923) holds the single-open redb lock, so the daemon (and cold `ariadne status`) cannot open `.ariadne/index.redb`. The daemon surfaces this correctly as `DaemonError::Storage` and leaves no pidfile/socket residue. Not a tier-07 defect; the socket-level `blast_radius`-vs-cold-golden assertion is covered by `blast_radius_matches_cold_golden` over a real `interprocess` socket.
- **Security/codec**: framed postcard with a 64 MiB length cap (`MAX_FRAME`) guards a malformed length prefix from a huge allocation; codec confined to `adapters/codec.rs`, no codec leak past the hexagonal boundary.
</checks_run>

<findings>
| id | category | severity | location | problem | fix |
|---|---|---|---|---|---|
| F1 | plan_adherence | INFO | crates/ariadne-daemon/Cargo.toml:21-26 | `<files>` says add `ariadne-salsa`; only `ariadne-graph`+`ariadne-storage` added, salsa deferred to tier-08 with a documented rationale (tier-07 rebuilds from snapshot, never touches the query DB). | Intended scope reduction — adding an unused dep would violate the "no scope beyond the tier" rule; confirm the deferral when tier-08 lands. |
| F2 | plan_adherence | INFO | crates/ariadne-core/src/domain/daemon/query.rs:37-105 | Exit criterion says "carries every v1 read query"; `project_status` and `refactor_suggestions` (both read-capable MCP tools) have no `DaemonQuery` variant. Matches the plan's explicit step-2 enumeration, which omits them. | Confirm both are intentionally deferred to the client-wiring tiers (09/10); add variants there if the daemon must serve them. |
| F3 | reliability | INFO | crates/ariadne-daemon/src/adapters/ipc.rs:124-128 | A transient refresh failure (`load_catalog?`) returns `Err` before any response frame is written, so the client sees a dropped connection rather than a typed `DaemonResponse::Error`; the daemon itself stays alive (correct). | Optionally map a refresh error to `DaemonResponse::Error` so clients can distinguish a transient stale-rebuild miss from daemon death. |
</findings>

<verdict>
**PASS.** Zero FAIL findings. All five `<verification>`-derived gates re-ran green (`-p ariadne-daemon` 15/15, architecture, clippy `-D warnings`, fmt). The protocol carries every read query the plan enumerates and the daemon dispatches each to `ariadne-graph` against a warm `GraphIndex` held behind an `RwLock`; the staleness handshake refreshes on a newer client revision; daemon-served results are proven byte-identical to the v1 cold path across all 11 queries (incl. component-graph and docgen markdown). The warm-graph build path is independently exercised on a real populated index. The three INFO items are non-blocking: two are plan/exit-criterion wording tensions the implementation resolves consistently, one is an optional robustness nicety.
</verdict>

<next_steps>
None required to land tier-07. For tier-08/09/10 follow-up (not gating): (a) re-introduce `ariadne-salsa` to `ariadne-daemon` when watcher-fed invalidation lands (F1); (b) decide whether `project_status`/`refactor_suggestions` need `DaemonQuery` variants when the mcp/cli/lsp clients are wired (F2); (c) consider a typed refresh-failure response (F3).
</next_steps>

<sources>
- tier file: `.claude/plans/post-v1-roadmap/tier-07-daemon-warm-graph.md`
- sibling plan: `.claude/plans/post-v1-roadmap/plan.md` (RD5/RD6, R-B2)
- ADR: `docs/adr/0015-daemon-mode-ipc.md`
- v1 parity reference: `crates/ariadne-mcp/src/{catalog.rs,tools/*}`
- interprocess local socket: https://docs.rs/interprocess/2.4.2/interprocess/local_socket/index.html
- postcard wire format: https://postcard.jamesmunns.com/wire-format
- reviewer standard: https://google.github.io/eng-practices/review/reviewer/standard.html
</sources>
</content>
</invoke>
