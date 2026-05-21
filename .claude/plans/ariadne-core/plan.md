---
slug: ariadne-core
title: Ariadne v1 — Rust code-intelligence platform with persisted graph + MCP surface for Claude
created: 2026-05-19
owners: [user, claude]
review: [user, codex?]
single_tier: false
tiers:
  - tier-00-foundations
  - tier-01-workspace
  - tier-02-storage
  - tier-03-parser
  - tier-04-salsa
  - tier-05-scip-ingest
  - tier-06-watcher
  - tier-07-graph-analytics
  - tier-08-mcp-server
  - tier-09-docgen-refactor
  - tier-10-cli-e2e
  - tier-11-c-cpp-indexing
  - tier-12-parallel-cold-index
  - tier-13-cold-index-slo
  - tier-14-analytics-quality
  - tier-15-mcp-discoverability
  - tier-16-setup-command
---

<context>
Problem: Claude opens every session blind to the repo; ad-hoc grep + read burns context and yields shallow guesses about impact, coupling, and architecture.
Solution: Local-first, embedded code-intelligence service that keeps an incrementally-updated semantic graph of any project (multi-language) and exposes it to Claude through MCP tools.
In scope (v1): semantic indexing of TS/JS, Python, Rust, Go, Java/Kotlin, C#/.NET via SCIP; syntactic indexing of any tree-sitter language; blast-radius, plan-assist, coupling/cohesion, dead code, cycles, doc-gen, refactor suggestions; per-project `.ariadne/` index; MCP stdio server.
Out of scope (v1): cross-repo symbol resolution; hosted/server deployment; LLM-mediated analysis (opt-in post-v1); IDE plugins beyond MCP.
</context>

<constraints>
- Repo scale: 100K files / 10M LOC ceiling [user]. Cold full index <60s; incremental update <500ms p95; query <100ms p95.
- Single static binary `ariadne`; pure-Rust deps preferred; no cgo.
- Tests first per tier; realistic fixtures from real OSS repos, no mocks at module boundaries [src: CLAUDE.md `<rules>`].
- No runtime network egress; SCIP indexers vendored on PATH.
- Memory ceiling: <4GB RAM on 100K-file workload; verified per tier with criterion + heaptrack.
- Single workspace + native monorepos (Cargo/pnpm/uv workspaces) [user]; cross-repo deferred post-v1.
</constraints>

<decisions>
**D1 — Core language: Rust** [user]. Native tree-sitter, single static binary, no GC pauses on hot path.

**D2 — Parsing backbone: tree-sitter.** Incremental updates sub-ms; battle-tested in rust-analyzer, zed, neovim; >40 grammars including all v1 langs [src: https://github.com/tree-sitter/tree-sitter].
*Rejected:* hand-written parsers (write-time prohibitive); ANTLR (no incremental, JVM); LSPs as primary parser (latency, install footprint, no syntactic graph for "any extension").

**D3 — Semantic interchange: SCIP protobuf.** Industry standard replacing LSIF; language-agnostic; native indexers exist for all v1 langs except Go [src: https://scip-code.org, https://github.com/sourcegraph/scip, https://sourcegraph.com/blog/announcing-scip]. v1 indexers: `rust-analyzer --scip`, `scip-typescript`, `scip-python`, `scip-java`, `scip-clang`, `scip-dotnet`. Go via `lsif-go` + `scip lsif-to-scip` (risk R3).
*Rejected:* LSIF (deprecated); rolling own per-lang resolvers (cost, breaks "any stack").

**D4 — Incremental compute: Salsa.** Query-graph with durability levels + early cutoff; proven at rust-analyzer scale [src: https://github.com/salsa-rs/salsa, https://rust-analyzer.github.io/blog/2023/07/24/durable-incrementality.html].
*Risk-aware:* rust-analyzer hit a 4x memory regression after a 2025 Salsa migration [src: https://github.com/rust-lang/rust-analyzer/issues/19402] → cap Salsa table cardinality, add per-table memory probes (tier-04), assign high durability to stdlib/vendor inputs.

**D5 — Persisted storage: redb.** Pure-Rust embedded, ACID, MVCC concurrent readers, stable on-disk format with upgrade promise; ~2.6x faster individual writes than RocksDB in published benches [src: https://github.com/cberner/redb, https://www.redb.org/post/2023/06/16/1-0-stable-release/].
*Rejected:* KuzuDB (project archived 2026 — unmaintained risk [src: https://github.com/kuzudb/kuzu]); RocksDB (cgo, build complexity); DuckDB (graph traversal weaker than in-RAM walk); Neo4j (server, ops).

**D6 — Live graph: petgraph DiGraph.** Tarjan SCC (cycles), Cooper et al. dominators (blast-radius), BFS/DFS [src: https://docs.rs/petgraph/latest/petgraph/algo/dominators/index.html]. Snapshots persisted to redb; live graph rebuilt on cold start.

**D7 — File watcher: notify-rs + notify-debouncer-full.** Cross-platform FSEvents/inotify/RDCW; used by rust-analyzer, zed, watchexec [src: https://github.com/notify-rs/notify]. `.gitignore` via `ignore` crate.

**D8 — Integration surface: MCP server via `rmcp` 1.7.0, stdio transport.** Claude Code natively supports MCP stdio; `#[tool]`/`#[tool_router]` macros remove boilerplate [src: https://docs.rs/rmcp, https://github.com/modelcontextprotocol/rust-sdk]. No daemon in v1 — server spawned per Claude session, redb cold-read expected <100ms.
*Rejected:* long-running daemon (lifecycle, install); hook-only (no on-demand queries); MCP+daemon combo (complexity unjustified pre-feedback).

**D9 — Test stack: `cargo-nextest` + `insta` + `proptest` + `rstest` + `criterion`.** Nextest 3x faster than `cargo test` on large suites [src: https://nexte.st]; insta for graph-output snapshots; proptest for incrementality invariants; rstest for fixture parametrization; criterion for perf gates. Failing test first per tier.

**D10 — Operating model: per-project `.ariadne/` directory, on-demand stdio MCP process.** Git-friendly (gitignored), isolated per project, no cross-repo state. Daemon mode deferred.

**D11 — Analysis mode: static-first; LLM hooks deferred.** Afferent/efferent coupling + instability index, Tarjan SCC for cycles, fan-in=0 for dead-code, dominator tree for blast-radius — deterministic graph metrics [src: https://win.tue.nl/~aserebre/2IS55/2009-2010/10.pdf]. LLM-augmented opt-in post-v1.

**D12 — Architecture inspiration: Meta Glean (units + ownership).** Per-file unit isolation enables O(changes) re-derive at <10% query overhead [src: https://glean.software/blog/incremental/, https://engineering.fb.com/2024/12/19/developer-tools/glean-open-source-code-indexing/]. We adopt the per-file unit boundary; we do not adopt Glean's stacked-DB strategy (Salsa already handles).

**D13 — Architectural style: Hexagonal / Ports & Adapters + TDD.** Domain (pure types, traits, use cases) lives in `ariadne-core` + `ariadne-graph` + `ariadne-salsa`; IO is pushed to adapter crates (`ariadne-storage` redb, `ariadne-parser` tree-sitter, `ariadne-scip` subprocess+protobuf, `ariadne-watcher` notify-rs); driving adapters are `ariadne-mcp` (MCP) and `ariadne-cli` (CLI). Rationale: enables in-memory adapters for unit tests, real adapters for integration; matches Rust's trait-based DI idiom; testing-first is the original Cockburn motivation ("driven by users, programs, automated test or batch scripts") [src: https://alistair.cockburn.us/hexagonal-architecture/, https://www.howtocodeit.com/guides/master-hexagonal-architecture-in-rust]. Enforced by tier-00 `tests/architecture.rs` invariant (ariadne-core depends on nothing in-workspace) and clippy/deny gates.
*Rejected:* Clean Architecture (extra layers without payoff for a stateless analytics pipeline); DDD (no rich business domain — entities/events/aggregates are overkill); pipeline-first (couples logic to phases, weakens IO isolation).

**D14 — Commit convention: Conventional Commits v1.0.0 + per-crate scopes, enforced via cocogitto.** Format: `<type>(<scope>)<!>: <subject>` [src: https://www.conventionalcommits.org/en/v1.0.0/]. Types: `feat`, `fix`, `docs`, `style`, `refactor`, `perf`, `test`, `build`, `ci`, `chore`, `revert`. Scopes = crate names without `ariadne-` prefix: `core`, `storage`, `parser`, `scip`, `graph`, `salsa`, `watcher`, `mcp`, `cli`, `e2e`, plus cross-cutting `docs`, `ci`, `deps`. Subject ≤72 chars, imperative mood. Breaking change marker `!` after scope OR `BREAKING CHANGE:` footer. Enforcement: cocogitto (pure-Rust CLI, single binary) [src: https://github.com/cocogitto/cocogitto, https://docs.cocogitto.io/]: (a) `cog install-hook commit-msg` → local commit-msg hook rejects malformed messages; (b) CI job `cog check origin/main..HEAD` blocks invalid commits on PR; (c) GitHub `amannn/action-semantic-pull-request` validates PR title (for squash merges). Changelog auto-generated by `cog changelog` consumed by tier-10 release pipeline.
*Rejected:* Gitmoji (no semver inference); free-form (no auto-changelog, no semver gating); commitlint-rs (smaller scope than cocogitto, no changelog/bump tooling); JS commitlint (drags Node dependency into a pure-Rust toolchain).
</decisions>

<architecture>
Hexagonal layout — interior = pure domain + use cases, exterior = adapters per IO concern (see tier-00 docs/architecture.md, docs/folder-layout.md, ADR-0001):

```
                 driving (inbound) adapters
              ┌──────────────┬──────────────┐
              │ ariadne-cli  │ ariadne-mcp  │
              └──────┬───────┴───────┬──────┘
                     ▼               ▼
        ┌───────── use cases / orchestration ─────────┐
        │  ariadne-graph (analytics)                  │
        │  ariadne-salsa (incremental query DB)       │
        │  ariadne-watcher (driving event loop)       │
        └──────────────────┬──────────────────────────┘
                           ▼
                ariadne-core  (DOMAIN: types, ids, ports)
                           ▲
        ┌──────────────────┴──────────────────────────┐
        │       driven (outbound) adapters             │
        │  ariadne-storage (redb)                      │
        │  ariadne-parser  (tree-sitter)               │
        │  ariadne-scip    (protobuf + subprocess)     │
        └──────────────────────────────────────────────┘
```

Per-crate internal layout (canonical, enforced by tier-00 `docs/folder-layout.md`):
```
crates/ariadne-<name>/
  Cargo.toml
  src/
    lib.rs              façade — re-exports only, no logic
    domain/             pure core (types, ports, use cases), zero IO
    adapters/           IO impls of ports, one file per external tech
    errors.rs           thiserror enum (anyhow only inside cli/e2e)
  tests/                integration — real adapters, real fixtures
  benches/              criterion benches
  fixtures/             test data (license-clean)
```
ariadne-core: domain-only (no `adapters/`). Adapter crates depend on ariadne-core; never on each other. Driving adapters depend on use-case crates + ariadne-core. tier-00 ships `tests/architecture.rs` invariant + cargo-deny rule to enforce.

On-disk project index:
```
.ariadne/                  per-project (gitignored)
  index.redb               symbols, files, edges, parse-cache, SCIP docs
  config.toml              enabled langs, indexer paths, ignore patterns
  logs/
```

Dataflow: watcher → invalidate file input → Salsa re-derives parse/symbols/graph subset → writes deltas to redb → MCP read txn serves queries from in-RAM petgraph + redb.
</architecture>

<tech_inventory>
| tech | role | pin in tier | source verified this session |
|---|---|---|---|
| Rust stable + MSRV | core | tier-01 | n/a |
| tree-sitter + grammars (ts/js/py/rs/go/java/kotlin-ng¹/c-sharp) | CST + incremental | tier-03 | https://github.com/tree-sitter/tree-sitter |
| tree-sitter-c 0.24, tree-sitter-cpp 0.23 | C/C++ syntactic grammars | tier-11 | https://crates.io/crates/tree-sitter-c ; https://docs.rs/tree-sitter-cpp ; docs/adr/0008-c-cpp-syntactic-indexing.md |
| salsa | incremental query DB | tier-04 | https://github.com/salsa-rs/salsa |
| redb | embedded ACID kv | tier-02 | https://github.com/cberner/redb |
| petgraph | in-RAM graph + algos | tier-07 | https://docs.rs/petgraph |
| notify + notify-debouncer-full | fs watcher | tier-06 | https://github.com/notify-rs/notify |
| ignore | gitignore semantics | tier-06 | https://docs.rs/ignore |
| prost | SCIP protobuf decode | tier-05 | https://github.com/sourcegraph/scip/blob/main/scip.proto |
| rmcp = 1.7.0 | MCP server | tier-08 | https://docs.rs/rmcp |
| cargo-nextest, insta, proptest, rstest, criterion | tests | tier-01 | https://nexte.st, https://insta.rs, https://proptest-rs.github.io |
| External SCIP indexers (on PATH): rust-analyzer, scip-typescript, scip-python, scip-java, scip-clang, scip-dotnet, lsif-go, scip CLI | semantic per lang | tier-05 | https://github.com/sourcegraph/scip |

¹ `tree-sitter-kotlin-ng` (amaanq, `tree-sitter-grammars` org) substitutes for the legacy `tree-sitter-kotlin` crate, which pins tree-sitter <0.23 and is incompatible with the v1 0.26.x pin [src: docs/adr/0006-tree-sitter-kotlin-ng.md].
</tech_inventory>

<risks>
| id | risk | likelihood | mitigation |
|---|---|---|---|
| R1 | Salsa memory bloat (mirrors rust-analyzer #19402) | medium | per-table memory probes (tier-04); cap derived-query memoization; flush low-durability tables on idle |
| R2 | tree-sitter CST RAM dominance | medium | persist serialized CST to redb; LRU N most-recent in RAM |
| R3 | Go lacks first-party SCIP indexer | high | tier-05 ships `lsif-go` + `scip lsif-to-scip` adapter; fallback: gopls LSP shim |
| R4 | redb file-format upgrade between versions | low | on-disk schema version; rebuild from source on mismatch |
| R5 | rmcp API churn between minor versions | medium | pin `rmcp = "=1.7.0"`; integration test in tier-08 against fixed schema |
| R6 | SCIP indexers diverge in symbol grammar | medium | tier-05 normalizes to canonical form; per-language golden fixtures |
| R7 | Watcher misses events on macOS under load | medium | union with periodic gitignore-aware scan; reconcile by content-hash |
| R8 | 60s cold-index SLO unachievable on 100K files | high | tier-10 measured 442.8s/55,527 files; tier-12 parallelised parse but the gate still failed at 84.3s/121,100 files; tier-13 streams the redb commit behind parse + reuses the tree-sitter `Query` per worker to close the residual gap — a non-lossy lever exhaustion escalates to the user, not a silent miss [src: tier-12, tier-13, ADR-0009, ADR-0010] |
| R9 | incremental + query p95 SLOs unverified at 100K scale (`slo.rs` panics at the cold stage first) | medium | tier-13 fixes cold-index so the `slo` gate reaches both stages, then verifies incremental p95 < 500ms (named risk: the `apply.rs` SYMBOLS full-scan on file delete) and query p95 < 100ms at 121K [src: tier-13] |
</risks>

<verification>
v1 is "done" when all of:
- E2E in tier-10 indexes ≥3 real OSS repos (one per major lang) totaling ≥100K files; asserts cold <60s + incremental p95 <500ms + 100 sampled queries p95 <100ms.
- All MCP tools (`list_symbols`, `find_definition`, `find_references`, `blast_radius`, `file_summary`, `plan_assist`, `coupling_report`, `weak_spots`, `doc_for`) pass insta golden fixtures.
- Proptest 100 random edit sequences: state divergence between full rebuild and incremental = 0.
- `cargo nextest run --workspace` green; `cargo bench --workspace` within budgets.
- Manual session: launch Claude Code with Ariadne MCP, run "blast radius of X" on a fixture repo, output matches golden.
</verification>

<sources>
- tree-sitter: https://github.com/tree-sitter/tree-sitter ; tree-sitter-c: https://crates.io/crates/tree-sitter-c ; tree-sitter-cpp: https://crates.io/crates/tree-sitter-cpp
- rayon (data-parallel iterator, map_init): https://docs.rs/rayon ; ignore (parallel walk): https://docs.rs/ignore
- peak-RSS measurement (/usr/bin/time): https://www.baeldung.com/linux/process-peak-memory-usage
- Salsa: https://github.com/salsa-rs/salsa ; https://rust-analyzer.github.io/blog/2023/07/24/durable-incrementality.html
- rust-analyzer memory regression: https://github.com/rust-lang/rust-analyzer/issues/19402
- SCIP: https://scip-code.org ; https://github.com/sourcegraph/scip ; https://sourcegraph.com/blog/announcing-scip
- redb: https://github.com/cberner/redb ; https://www.redb.org/post/2023/06/16/1-0-stable-release/
- petgraph: https://docs.rs/petgraph
- notify-rs: https://github.com/notify-rs/notify
- rmcp: https://docs.rs/rmcp ; https://github.com/modelcontextprotocol/rust-sdk
- Glean: https://glean.software/blog/incremental/ ; https://engineering.fb.com/2024/12/19/developer-tools/glean-open-source-code-indexing/
- nextest: https://nexte.st ; insta: https://insta.rs ; proptest: https://proptest-rs.github.io ; criterion: https://github.com/bheisler/criterion.rs
- Software metrics: https://win.tue.nl/~aserebre/2IS55/2009-2010/10.pdf
- Anthropic XML prompting: https://platform.claude.com/docs/en/build-with-claude/prompt-engineering/claude-prompting-best-practices
- Hexagonal Architecture (Cockburn, 2005): https://alistair.cockburn.us/hexagonal-architecture/
- Hexagonal in Rust (How To Code It): https://www.howtocodeit.com/guides/master-hexagonal-architecture-in-rust
- cargo-dist: https://opensource.axo.dev/cargo-dist/
- cargo-deny: https://embarkstudios.github.io/cargo-deny/
- rustfmt: https://rust-lang.github.io/rustfmt/ ; clippy: https://rust-lang.github.io/rust-clippy/master/
- lefthook: https://lefthook.dev/
- Conventional Commits v1.0.0: https://www.conventionalcommits.org/en/v1.0.0/
- cocogitto: https://github.com/cocogitto/cocogitto ; https://docs.cocogitto.io/
- semantic PR title action: https://github.com/amannn/action-semantic-pull-request
- tree-sitter QueryCursor (Query/cursor reuse): https://docs.rs/tree-sitter/0.26.8/tree_sitter/struct.QueryCursor.html
- redb WriteTransaction (per-batch commit): https://docs.rs/redb/4.1.0/redb/struct.WriteTransaction.html
</sources>
