---
slug: post-v1-roadmap
title: Ariadne post-v1 roadmap — close v1 deferrals, daemon/warm-graph, deeper analytics, wider reach
created: 2026-05-22
owners: [user, claude]
review: [user, codex?]
single_tier: false
tiers:
  - tier-01-go-native-scip
  - tier-02-redb-schema-migration
  - tier-03-astro-semantic
  - tier-04-symbol-metadata-enrichment
  - tier-05-dead-code-classification
  - tier-06-daemon-skeleton
  - tier-07-daemon-warm-graph
  - tier-08-daemon-watcher-live
  - tier-09-mcp-daemon-client
  - tier-10-cli-daemon-client-slo
  - tier-11-git-history-ingest
  - tier-12-cyclomatic-complexity
  - tier-13-hotspot-cochange-metrics
  - tier-14-diff-aware-blast-radius
  - tier-15-analytics-mcp-tools
  - tier-16-lsp-skeleton
  - tier-17-lsp-navigation
  - tier-18-lsp-hierarchy
  - tier-19-hierarchy-impl-mcp-tools
---

<context>
Problem: Ariadne shipped v1.0.0 (ariadne-core tiers 00–16 + js-framework tiers 01–09, all audited PASS). Four classes of gap remain — (A) deferred v1 scope, (B) cold per-session process model, (C) shallow static-only analytics, (D) Claude-only reach.
Solution: a tiered post-v1 roadmap closing all four. Block A finishes deferrals; Block B reverses the per-session model into a warm daemon; Block C adds history-aware analytics; Block D exposes Ariadne to any LSP editor.
In scope: native Go SCIP, redb schema migration, Astro semantic indexing, SymbolRecord metadata enrichment + per-language dead-code classification; daemon + warm in-RAM graph; git-history hotspots/churn/co-change, cyclomatic complexity, diff-aware blast radius; LSP server adapter + call/type-hierarchy + implementations.
Out of scope: LLM/embedding/semantic-search inside Ariadne — the MCP consumer (Claude/Codex) is the LLM and reasons over the deterministic graph [user]; cross-repo symbol resolution; Angular.
</context>

<constraints>
- Inherits ariadne-core plan.md D1–D14 and js-framework-support D1–D11, unchanged except D10 (reversed by RD5–RD6) and D11 (LLM clause hardened: out of scope, not deferred — see `<context>`).
- Pure-Rust on the critical path; no cgo, no Node/JVM in the `ariadne` binary [src: ariadne-core plan.md D5]. External SCIP indexers stay on PATH as subprocesses.
- Single static `ariadne` binary; the daemon is a subcommand mode, not a second binary [src: ariadne-core plan.md `<constraints>`].
- Hexagonal + TDD; failing test before implementation per tier [src: CLAUDE.md `<rules>`].
- SLOs hold: cold full-index <60s, incremental p95 <500ms, query p95 <100ms; <4GB RAM on 100K files [src: ariadne-core plan.md `<constraints>`]. Warm-query target tightens to p95 <10ms (RD6).
- Each tier ships an ADR when it makes an architectural decision; audit-gated per `.claude/hooks/audit-gate.sh`.
</constraints>

<decisions>
**RD1 — Go: native `scip-go`, drop the `lsif-go` fallback.** `scip-go` is the first-party Go SCIP indexer (`go install github.com/scip-code/scip-go/cmd/scip-go@latest` — its Go module path is `github.com/scip-code/scip-go`, not `sourcegraph/...`), invoked from repo root, flags `--module-path`/`--module-version`/`--go-version` [src: https://github.com/scip-code/scip-go ; `scip-go index --help`, scip-go v0.2.6]. Resolves risk R3 of the v1 plan. *Rejected:* keep `lsif-go`+`scip convert` (two-step, lossy LSIF intermediate, extra binary).

**RD2 — redb schema migration: registered `vN→vN+1` steps replace rebuild-on-mismatch.** v1 stores a schema version and returns `SchemaMismatch` → full rebuild (tier-02 hard limit). RD2 adds an ordered `MigrationRegistry` of pure transform steps run inside one redb `WriteTransaction` [src: https://docs.rs/redb/4.1.0/redb/struct.WriteTransaction.html]. No new dependency. *Rejected:* always rebuild (discards SCIP ingest cost on every format bump).

**RD3 — Astro semantic: extend the SFC bridge to `.astro` frontmatter.** The `.astro` frontmatter fence (`---`) is TypeScript; the v1 SFC bridge already extracts `<script>` TS regions for Vue/Svelte, runs `scip-typescript`, and remaps offsets [src: docs/adr/0013-scip-sfc-bridge.md]. RD3 reuses that path for the Astro frontmatter region. *Rejected:* a Volar→SCIP indexer (no verified path; large new surface — js-framework R-Astro).

**RD4 — Dead-code: per-language entry-point classifier.** v1 `dead_symbols` flags fan-in=0 symbols; it false-positives on roots (`main`, exported API, `#[test]`, framework entrypoints) — confirmed by self-`weak_spots` flagging `main`/test fns. RD4 computes a per-language root set, excluded before fan-in=0 [src: ariadne-core tier-14-analytics-quality.md "per-language target classification is future work"].

**RD5 — Daemon: long-running `ariadne daemon` mode; IPC via `interprocess` local socket.** Reverses v1 D10 (per-session stdio process). `interprocess` 2.4.2 abstracts Unix domain sockets / Windows named pipes behind one `local_socket` API, optional tokio feature [src: https://docs.rs/interprocess/2.4.2/interprocess/]. Protocol request/response types are pure → live in `ariadne-core`. Reversal recorded in ADR-0015. *Rejected:* TCP loopback (port conflicts, firewall prompts); D-Bus (Linux-only).

**RD6 — Warm graph: the daemon owns the in-RAM petgraph; mcp/cli/lsp are thin clients.** Eliminates the per-session redb cold read + graph rebuild. Cold-read mode is retained as an auto-fallback when no daemon is reachable, so v1 behaviour is never lost. Warm-query target p95 <10ms.

**RD7 — Git history: new driven adapter `ariadne-git` on `gix` 0.83.0.** Pure-Rust Git; commit walk via `head.ancestors().all()`, per-commit changed files via `gix-diff` tree-to-tree; Cargo itself depends on `gix` [src: https://lib.rs/crates/gix]. Built with read-only, no-network feature set (no curl/transport) to keep the critical path pure-Rust. *Rejected:* shelling out to `git` (breaks "no external runtime", parsing fragility); `git2` (libgit2 = C, violates D5).

**RD8 — Cyclomatic complexity: McCabe `M = decision-points + 1` from the tree-sitter CST.** Counted by walking branch nodes (`if`/`for`/`while`/`case`/`&&`/`||`/`?`) on CSTs Ariadne already holds [src: McCabe, "A Complexity Measure", IEEE TSE 1976; https://en.wikipedia.org/wiki/Cyclomatic_complexity]. No external dependency. *Rejected:* `rust-code-analysis` (heavy multi-grammar dep duplicating our parser).

**RD9 — LSP: new driving adapter `ariadne-lsp` on `async-lsp` 0.2.4.** `async-lsp` is tower-`Layer`-based, supports request snapshotting during preparation (Ariadne snapshots the graph then serves) and builds both servers and clients [src: https://lib.rs/crates/async-lsp]. The LSP adapter is a thin daemon client (RD6). *Rejected:* `tower-lsp` (original crate unmaintained); `lsp-server` (low-level sync, no middleware); `tower-lsp-server` (viable, but no snapshot-friendly `&mut self`/immutable-request split).

**RD10 — SymbolRecord metadata enrichment: `visibility` + `attributes`, prerequisite to RD4.** v1 `SymbolRecord` carries `canonical_name`/`kind` only [src: crates/ariadne-core/src/domain/records.rs:28-37], so the RD4 classifier cannot see Rust `pub`/`#[test]`, JS/TS exports, or Java/C# annotations. RD10 adds a public `Visibility` enum (`Public`/`Restricted`/`Private`/`Unknown` — a coarse lattice spanning ~10 language visibility models) and `attributes: Vec<String>` to `SymbolRecord`, threaded core→storage→parser→scip→cli→salsa. postcard is non-self-describing — struct field count and names are not on the wire [src: https://postcard.jamesmunns.com/wire-format] — so the change ships behind one redb `MigrationRegistry` step (RD2) that re-encodes the `SYMBOLS` table in place, no rebuild. Recorded in ADR-0014; tier-04 precedes tier-05. *Rejected:* raw per-language modifier strings (every consumer re-parses, no typed guarantee); rebuild-on-open (discards SCIP ingest cost — the failure RD2 fixed).
</decisions>

<architecture>
Three new crates, classified per the hexagonal invariant + ADR-0007 composition-root precedent:
- `ariadne-daemon` — driving adapter + long-running host. Owns the warm petgraph, the watcher event loop, and the `interprocess` IPC listener; composition root for daemon mode (wires storage/parser/scip/salsa/graph/watcher).
- `ariadne-git` — driven adapter. `gix`-backed history reader; depends only on `ariadne-core`.
- `ariadne-lsp` — driving adapter. `async-lsp` server; thin client to `ariadne-daemon`.

IPC topology: request/response wire types live in `ariadne-core/domain` (pure, no IO). The `interprocess` transport lives in `ariadne-daemon` (server side); driving adapters embed a thin `daemon_client` module. tier-06 ADR-0015 fixes the final topology — if per-adapter duplication exceeds one file, ADR-0015 introduces a shared `ariadne-ipc` crate with an explicit `tests/architecture.rs` exception (precedent: ADR-0007 carved out the composition root).

Warm dataflow (daemon mode): watcher → daemon invalidates Salsa input → re-derive parse/symbols/graph subset → update warm petgraph + write deltas to redb → IPC clients (mcp/cli/lsp) query the warm graph over the local socket. Cold dataflow (no daemon) = unchanged v1 path.

Analytics: `ariadne-git` feeds churn + co-change into new redb tables (schema bump via RD2); `ariadne-parser` computes complexity per symbol; `ariadne-graph` adds hotspot/co-change/diff-blast use cases consumed by both MCP tools and the LSP adapter.

Symbol metadata: tier-04 widens `SymbolRecord` with `visibility` + `attributes` (RD10), threaded core→storage→parser→scip→cli→salsa behind a redb v2→v3 migration step; the tier-05 RD4 dead-code classifier consumes it. No new crate.
</architecture>

<tech_inventory>
| tech | version pinned | role | tier | source verified this session |
|---|---|---|---|---|
| scip-go | latest via `go install` | native Go SCIP indexer (PATH) | 01 | https://github.com/scip-code/scip-go |
| redb | 4.1.0 (v1 pin) | schema-migration framework | 02, 04 | https://docs.rs/redb/4.1.0/redb/struct.WriteTransaction.html |
| postcard | 1.x (v1 pin) | non-self-describing codec — drives the v2→v3 schema migration | 04 | https://postcard.jamesmunns.com/wire-format |
| tree-sitter | 0.26.x (v1 pin) | visibility/attribute query captures; cyclomatic complexity from CST | 04, 12 | https://tree-sitter.github.io/tree-sitter/using-parsers/queries/1-syntax.html |
| interprocess | 2.4.2 | daemon IPC (local socket) | 06 | https://docs.rs/interprocess/2.4.2/interprocess/ |
| gix | 0.83.0 | git history + diff (pure-Rust) | 11 | https://lib.rs/crates/gix |
| async-lsp | 0.2.4 | LSP server adapter | 16 | https://lib.rs/crates/async-lsp |
</tech_inventory>

<risks>
| id | risk | likelihood | mitigation |
|---|---|---|---|
| R-A1 | `scip-go` needs the Go toolchain on PATH | low | tier-10 v1 CI image already bundles Go; `--module-path` skips `go` calls when absent |
| R-A2 | the v2→v3 `SYMBOLS` migration mis-decodes old records (postcard is positional) | low | the migration decodes via a frozen `SymbolRecordV2`; a round-trip test asserts every pre-migration record survives with its first four fields byte-identical |
| R-B1 | IPC crate topology collides with the adapter-isolation invariant | medium | ADR-0015 fixes topology; ADR-0007 precedent permits a justified `tests/architecture.rs` exception |
| R-B2 | warm graph drifts from on-disk state (staleness) | medium | watcher-fed invalidation; redb revision compared on every client connect; stale → daemon self-refreshes |
| R-B3 | daemon lifecycle: stale socket/pidfile, orphan process | medium | pidfile + liveness handshake; auto-spawn on miss; auto-reap on idle timeout |
| R-C1 | `gix` history walk slow on large repos | medium | configurable bounded commit depth; use the commit-graph file; walk once at index time, persist to redb |
| R-D1 | LSP UTF-16 positions vs Ariadne byte offsets | medium | explicit encoding-conversion layer in `ariadne-lsp`; property test round-trips offsets |
</risks>

<verification>
v1 SLOs and all v1 audits must stay green throughout; the ariadne_v2 self-index dogfood run must stay green.
- Block A: `scip-go` indexes `golang/example` with symbol + relationship counts ≥ the lsif-go baseline; a redb file at schema `vN-1` opens and migrates with data intact (no rebuild); a v2 redb file migrates to v3 with `SymbolRecord` records intact and `visibility`+`attributes` populated across the language fixtures; `.astro` frontmatter yields semantic edges in a golden; `dead_symbols` no longer flags `main`/exported/`#[test]` symbols across the 7-language fixtures.
- Block B: `ariadne daemon` starts; mcp/cli/lsp connect and query; warm query p95 <10ms; edit→watcher→warm-graph update p95 <500ms; daemon RSS <4GB on the 100K-file workload (R1 memory probe).
- Block C: `hotspots`/`complexity`/`co_change`/`diff_blast_radius` MCP tools pass insta goldens; diff-aware blast radius on a real branch equals the union of per-changed-file blast radius.
- Block D: `ariadne-lsp` passes an integration test (initialize → definition/references/hover/documentSymbol/callHierarchy) driven by a spawned LSP client; a manual VS Code session resolves a definition; `call_hierarchy`/`type_hierarchy`/`implementations` MCP tools pass goldens.
- Whole: `cargo nextest run --workspace` green; `cargo bench --workspace --no-run` green; every tier audit verdict PASS.
</verification>

<sources>
- scip-go (native Go SCIP indexer): https://github.com/scip-code/scip-go
- redb WriteTransaction: https://docs.rs/redb/4.1.0/redb/struct.WriteTransaction.html
- interprocess (local socket IPC): https://docs.rs/interprocess/2.4.2/interprocess/
- gix / gitoxide: https://lib.rs/crates/gix ; https://github.com/GitoxideLabs/gitoxide
- async-lsp: https://lib.rs/crates/async-lsp
- McCabe cyclomatic complexity: https://en.wikipedia.org/wiki/Cyclomatic_complexity (McCabe, "A Complexity Measure", IEEE TSE, 1976)
- postcard wire format (non-self-describing): https://postcard.jamesmunns.com/wire-format
- tree-sitter query syntax: https://tree-sitter.github.io/tree-sitter/using-parsers/queries/1-syntax.html
- SFC bridge precedent: docs/adr/0013-scip-sfc-bridge.md
- composition-root precedent: docs/adr/0007-cli-composition-root.md
- v1 plan: .claude/plans/ariadne-core/plan.md ; js-framework plan: .claude/plans/js-framework-support/plan.md
- Hexagonal Architecture (Cockburn, 2005): https://alistair.cockburn.us/hexagonal-architecture/
- LSP position encodings: https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/
</sources>
