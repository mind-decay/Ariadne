# ADR-0002: v1 tech stack

<status>
Accepted
Date: 2026-05-19
Decider: user
</status>

<context>
Tier-by-tier work in [`.claude/plans/ariadne-core/plan.md`](../../.claude/plans/ariadne-core/plan.md) pins twelve technology decisions (D1–D12) plus the architectural-style choice (D13) covered by [ADR-0001](0001-architecture-style.md) and the commit convention (D14) covered by [ADR-0003](0003-commit-convention.md). This ADR records the surface area of the **tech** stack for a new contributor who needs one canonical place to see *what* we use *where* and *why we did not pick the alternatives*. Per-decision rationale and full citation list live in the plan; this ADR mirrors the choices in normative form.
</context>

<decision>
The v1 tech stack is fixed as follows. Adding or replacing any item requires a superseding ADR.

| ID | Concern | Choice | Where it lives | Tier |
| --- | --- | --- | --- | --- |
| D1 | Implementation language | Rust (stable + pinned MSRV) | every crate | tier-01 |
| D2 | Parsing backbone | tree-sitter + per-lang grammars | `ariadne-parser` | tier-03 |
| D3 | Semantic interchange | SCIP protobuf + external indexers | `ariadne-scip` | tier-05 |
| D4 | Incremental compute | Salsa | `ariadne-salsa` | tier-04 |
| D5 | Embedded storage | redb | `ariadne-storage` | tier-02 |
| D6 | In-RAM graph + algorithms | petgraph (Tarjan SCC, dominators, BFS/DFS) | `ariadne-graph` | tier-07 |
| D7 | File watcher | notify + notify-debouncer-full + `ignore` | `ariadne-watcher` | tier-06 |
| D8 | Integration surface | MCP stdio via `rmcp = "=1.7.0"` | `ariadne-mcp` | tier-08 |
| D9 | Test stack | `cargo-nextest`, `insta`, `proptest`, `rstest`, `criterion` | every crate | tier-01 |
| D10 | Operating model | per-project `.ariadne/`; on-demand MCP process; no daemon | runtime | tier-08, tier-10 |
| D11 | Analysis mode | static-first (deterministic graph metrics); LLM hooks deferred | `ariadne-graph` | tier-07 |
| D12 | Per-file unit isolation | Glean-style file units for O(changes) re-derive | `ariadne-salsa` | tier-04 |
</decision>

<rationale>
- **Scalability** — Tree-sitter incremental parse keeps re-parse cost sub-ms per edited file `[src: https://github.com/tree-sitter/tree-sitter]`. Salsa with high-durability stdlib/vendor inputs caps derivation work to the changed file set `[src: https://rust-analyzer.github.io/blog/2023/07/24/durable-incrementality.html]`. petgraph in-RAM walk beats a graph DB for 100K-file workloads `[src: ../../.claude/plans/ariadne-core/plan.md D6]`.
- **Reliability** — redb is ACID with stable on-disk format and an upgrade promise `[src: https://www.redb.org/post/2023/06/16/1-0-stable-release/]`. Single pure-Rust dep tree avoids cgo/JNI footguns `[src: ../../.claude/plans/ariadne-core/plan.md D5, D14]`. nextest is 3x faster than `cargo test` and yields per-test isolation `[src: https://nexte.st]`.
- **Efficiency** — SCIP replaces LSIF as the industry standard `[src: https://sourcegraph.com/blog/announcing-scip]`; indexers exist for every v1 language except Go (R3, mitigated by `lsif-go` + `scip lsif-to-scip`). `rmcp` 1.7.0 macros remove transport boilerplate `[src: https://docs.rs/rmcp]`.
- **Maintainability** — Locking versions per crate (Cargo.toml) plus `cargo deny` license + ban rules guarantees a reproducible toolchain. Single static binary `ariadne` simplifies distribution.
</rationale>

<alternatives>
- **Hand-written parsers / ANTLR / LSPs as primary parser** — rejected for D2. Write-time prohibitive; ANTLR is JVM-based with no incremental mode; LSPs add install footprint and offer no syntactic graph for arbitrary extensions `[src: ../../.claude/plans/ariadne-core/plan.md D2]`.
- **LSIF / rolling own per-lang resolvers** — rejected for D3. LSIF is deprecated; per-lang resolvers cost too much and would break "any stack" `[src: ../../.claude/plans/ariadne-core/plan.md D3]`.
- **KuzuDB, RocksDB, DuckDB, Neo4j** — rejected for D5. KuzuDB project archived 2026; RocksDB requires cgo; DuckDB graph traversal weaker than in-RAM; Neo4j is a server with ops overhead `[src: ../../.claude/plans/ariadne-core/plan.md D5]`.
- **Long-running daemon or hook-only integration** — rejected for D8. Per-session MCP stdio avoids lifecycle management; daemon mode deferred pending feedback `[src: ../../.claude/plans/ariadne-core/plan.md D8]`.
</alternatives>

<consequences>
- `cog.toml` `[packages]` enumerates the crates above; commits/PRs are validated against the scope list ([ADR-0003](0003-commit-convention.md)).
- `cargo deny`'s `[bans]` rule encodes the dependency direction; introducing a new top-level external dependency requires updating `deny.toml` allowlist + a justification in the PR.
- v1 risk register (R1–R8 in plan) tracks the load-bearing assumptions of this stack. Each tier touching Salsa or in-RAM graph reports `memory_report()` deltas per [CLAUDE.md `<rules>`](../../CLAUDE.md).
- Go's SCIP gap (R3) is the one fragile point; if `lsif-go` + `scip lsif-to-scip` regresses, tier-05 falls back to a gopls shim. That fallback is in scope; bringing in a new vendor outside the table is out of scope without a superseding ADR.
- Cross-repo resolution, IDE plugins beyond MCP, and LLM-mediated analysis are explicitly out of v1 `[src: ../../.claude/plans/ariadne-core/plan.md <context>]` — adding them needs both a new ADR and a new tier.
</consequences>

<sources>
- [`.claude/plans/ariadne-core/plan.md`](../../.claude/plans/ariadne-core/plan.md)
- [ADR-0001 — Architecture style](0001-architecture-style.md)
- [ADR-0003 — Commit convention](0003-commit-convention.md)
- [tree-sitter](https://github.com/tree-sitter/tree-sitter)
- [Salsa](https://github.com/salsa-rs/salsa); [durable incrementality](https://rust-analyzer.github.io/blog/2023/07/24/durable-incrementality.html)
- [SCIP](https://github.com/sourcegraph/scip); [announcing SCIP](https://sourcegraph.com/blog/announcing-scip)
- [redb](https://github.com/cberner/redb)
- [petgraph](https://docs.rs/petgraph)
- [notify-rs](https://github.com/notify-rs/notify)
- [rmcp](https://docs.rs/rmcp); [MCP Rust SDK](https://github.com/modelcontextprotocol/rust-sdk)
- [cargo-nextest](https://nexte.st); [insta](https://insta.rs); [proptest](https://proptest-rs.github.io)
- [Glean (Meta)](https://glean.software/blog/incremental/)
</sources>
