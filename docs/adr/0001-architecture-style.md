# ADR-0001: Hexagonal architecture with TDD

<status>
Accepted
Date: 2026-05-19
Decider: user
</status>

<context>
Ariadne is a multi-crate Rust system: it parses source via tree-sitter, drives external SCIP indexers, persists into redb, runs Salsa-incremental analytics, and serves both an MCP stdio API and a CLI. Each surface has different durability, latency, and concurrency profiles. We need an architectural style that:

1. lets us swap an IO implementation without churning domain logic (e.g., in-memory `Storage` for unit tests, real `redb` adapter for integration);
2. keeps the domain unit-testable without spinning up real subprocesses or filesystems;
3. encodes the boundary as a compiler-checkable invariant in a Rust workspace;
4. survives growth from 6 to 10+ crates without devolving into ad-hoc layering.

The architectural lenses fixed by [CLAUDE.md `<rules>`](../../CLAUDE.md) are scalability, reliability, efficiency, maintainability. Delivery speed is not a tradeoff axis.
</context>

<decision>
Adopt **Hexagonal Architecture (Ports & Adapters)** ([Cockburn 2005](https://alistair.cockburn.us/hexagonal-architecture/)) with **Test-Driven Development** as the development discipline. Domain crates (`ariadne-core`, `ariadne-graph`, `ariadne-salsa`) own port traits and pure use cases. Driven adapter crates (`ariadne-storage`, `ariadne-parser`, `ariadne-scip`) implement those ports against external technologies. Driving adapter crates (`ariadne-cli`, `ariadne-mcp`, `ariadne-watcher`) call into use cases.
</decision>

<rationale>
- **Maintainability** — Hexagonal in Rust maps onto trait-based DI without runtime cost; the boundary is a `dyn Port` or a generic parameter, not a framework abstraction `[src: https://www.howtocodeit.com/guides/master-hexagonal-architecture-in-rust]`. The "one external tech per adapter file" rule under [docs/folder-layout.md](../folder-layout.md) keeps blast radius of any dependency upgrade local.
- **Reliability** — Cockburn's original motivation is testability under "users, programs, automated tests or batch scripts" `[src: https://alistair.cockburn.us/hexagonal-architecture/]`. TDD per-tier (failing test first, real adapters at module boundaries — no mocks `[src: ../../.claude/plans/ariadne-core/plan.md]`) catches regressions at the seam where most defects live.
- **Scalability** — Adapters can be parallelized (Salsa across files; `ariadne-scip` across languages) without touching domain code. Ports allow future drivers (HTTP, daemon) to be added without rewriting analytics.
- **Efficiency** — No layered overhead: a port call lowers to a vtable dispatch or monomorphized inline. No reflection, no DI container.
</rationale>

<alternatives>
- **Clean Architecture (Uncle Bob's concentric rings)** — rejected. Adds extra layers (entities / use cases / interface adapters / frameworks) that buy little for a stateless analytics pipeline; the "interactor" layer is redundant when the use cases already live in their own crate `[src: ../../.claude/plans/ariadne-core/plan.md D13]`.
- **Domain-Driven Design** — rejected. Aggregates / value objects / domain events presume rich business invariants we do not have. Our domain entities are `FileId`, `SymbolId`, `Edge` — flat data, no transactional consistency rules `[src: ../../.claude/plans/ariadne-core/plan.md D13]`.
- **Pipeline-first / layered (parse → resolve → graph → query)** — rejected. Couples logic to phases and forces IO concerns into each phase. Hexagonal lets phases remain pure functions that compose; IO is pushed to the perimeter.
</alternatives>

<consequences>
- `ariadne-core` has zero in-workspace dependencies. Enforced by `tests/architecture.rs` walking `cargo metadata` (tier-00 step 1) and by `cargo deny` bans.
- Adapter crates depend only on `ariadne-core`; they never depend on each other. The single exception is `ariadne-salsa` depending on `ariadne-storage` to invoke persistence within a query; this is recorded in [docs/folder-layout.md](../folder-layout.md).
- `src/lib.rs` is a façade — `pub use` only, no logic. Reviewer can audit dependency direction by reading `lib.rs` alone.
- TDD becomes a pull request gate. Each tier's PR adds a failing test first commit; the [PR template](../../.github/PULL_REQUEST_TEMPLATE.md) requires the checkbox.
- Replacing redb with another store (sled, RocksDB) becomes a single-crate change; same for tree-sitter ↔ alternative parser.
- Future architectural changes that violate the port boundary require a superseding ADR; the audit gate (`/spec-audit`) treats violations as hard fails.
</consequences>

<sources>
- [Cockburn — Hexagonal Architecture](https://alistair.cockburn.us/hexagonal-architecture/)
- [How To Code It — Master Hexagonal Architecture in Rust](https://www.howtocodeit.com/guides/master-hexagonal-architecture-in-rust)
- [`.claude/plans/ariadne-core/plan.md` D13](../../.claude/plans/ariadne-core/plan.md)
- [`docs/architecture.md`](../architecture.md)
- [`docs/folder-layout.md`](../folder-layout.md)
</sources>
