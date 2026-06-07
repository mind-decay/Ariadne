---
slug: intelligence-platform
title: Ariadne intelligence-platform arc — deepen the brain (A), broaden into products (B), platformize (C)
created: 2026-06-07
owners: [user, claude]
review: [user, codex?]
single_tier: false
blocks: [block-a-deepen-brain, block-b-reach-products, block-c-platform-foundation]
next_step: run /spec-plan on each block seed file (A, then B, then C) to expand it into detailed, audited tiers
---

<context>
Problem: Ariadne shipped v1 + the post-v1 roadmap (daemon/warm-graph, git analytics, complexity, diff-blast, SCIP-driven edges; LSP tiers 16–19 deferred). The graph is deep but **headless, single-machine, agent-only** — the only surfaces are MCP stdio + CLI [src: recon — `crates/ariadne-mcp`, `crates/ariadne-cli`; no http/grpc dep in any `Cargo.toml`]. Its strengths (15 languages, precise SCIP edges, warm incremental graph, history analytics) are an unexploited foundation.
Success (one sentence): a three-block arc that makes the graph qualitatively smarter (A), turns it into four shippable products (B), and exposes it as a substrate other projects embed (C) — each block a seed plan a later `/spec-plan` session deepens into audited tiers.
This file is the **arc master**: shared context, constraints, cross-cutting decisions, the full tech inventory, and pointers to the three block seed files. The blocks hold the per-block scope; neither this file nor the block files commit tiers — `/spec-plan` per block does that.
In scope: the three block seed files below. Out of scope (this arc): data-flow/taint + security scanner (future, noted in A as stretch); cross-repo/monorepo federation (left open by the user, not committed); graph-export to external stores (deprioritised by the user).
</context>

<constraints>
- Inherits all v1 + post-v1 invariants: pure-Rust on the critical path, no cgo/Node/JVM in the `ariadne` binary [src: .claude/plans/ariadne-core/plan.md D5; post-v1-roadmap plan.md `<constraints>`].
- Single static `ariadne` binary; every new surface (http server, explorer host, review CLI, future gRPC) is a subcommand mode or a driving adapter wired at a composition root, never a second binary [src: ariadne-core plan.md `<constraints>`; ADR-0007 composition-root precedent].
- Hexagonal + TDD: `ariadne-core` declares ports; adapters implement; adapters never depend on each other; failing test before implementation [src: CLAUDE.md `<rules>`; tests/architecture.rs].
- Deterministic — no in-product LLM/embedding/inference; the MCP/API consumer is the LLM [src: post-v1-roadmap plan.md `<context>`; memory feedback_no_llm_features].
- Local-first: no hosted/multi-tenant SaaS in this arc; network surfaces bind loopback by default [user, Q4].
- All v1 SLOs hold: cold full-index <60s, incremental p95 <500ms, query p95 <100ms, warm query p95 <10ms, <4GB RAM on 100K files [src: post-v1-roadmap plan.md `<constraints>`].
- Each authored tier (in the per-block expansions) ships an ADR when it makes an architectural decision; audit-gated per `.claude/hooks/audit-gate.sh` [src: CLAUDE.md `<workflow>`].
</constraints>

<decisions>
**AD1 — Sequence A → B → C; C is decided now but expanded later.** The user set this order (Q1): deepen the brain first, broaden into products next, platformize last. A's capabilities are the inputs B's products compose; C's public surface is shaped by what A/B expose, so committing C tiers now would be speculation [src: user Q1; spec-plan `<anti_patterns>` — no tiers that depend on un-built state]. Each block is a separate `/spec-plan` seed (user instruction).
**AD2 — Build on the existing graph, add no analysis engine A doesn't need.** Test-impact, API-surface-diff, and fitness checks are all derivable from assets already in the graph (call/ref edges, `Visibility`, `attributes`, git diff, coupling/cycle detection) — so Block A adds use-cases, not new heavy machinery [src: recon — `Visibility` enum at `crates/ariadne-core/src/domain/types/visibility.rs`; existing `diff_blast_radius`/`hotspots`/`coupling_report` tools; AD aligns with rule "no features beyond the tier"].
**AD3 — The web explorer reuses the existing deterministic SVG emitters, not a new viz stack.** `architecture_svg`, `module_svg`, `render_svg` already emit deterministic, well-formed SVG [src: `crates/ariadne-graph/src/docgen.rs`, `crates/ariadne-graph/src/diagram.rs`; tests `*_svg_is_deterministic_and_well_formed`]. B serves these over HTTP with minimal vanilla JS for interactivity — no Node build, honouring the no-Node constraint. *Rejected:* a Rust/WASM SPA framework (Leptos/Dioxus) or a JS graph lib via npm — both add a build toolchain the SVG path makes unnecessary.
**AD4 — Every new surface is a thin daemon client, mirroring mcp/cli.** The HTTP API (B) and the future gRPC API (C) embed the existing `daemon_client` pattern and query the warm catalog; cold fallback is retained [src: `crates/ariadne-cli/src/adapters/daemon_client.rs`; post-v1-roadmap RD6]. Preserves the warm-graph SLO and the adapter-isolation invariant.
**AD5 — C's plugin system is WASM-sandboxed, not dynamic-linked.** A single static binary forbids `dylib` plugins; WASM via `wasmtime` runs untrusted third-party analytics sandboxed, pure-Rust, no recompile [src: https://docs.wasmtime.dev/api/wasmtime/ — component model default feature, `Linker`, `bindgen!`; https://crates.io/crates/wasmtime]. Tree-sitter language grammars stay compile-time registered. Decided here; detailed in block C.
</decisions>

<architecture>
The arc layers strictly onto the current hexagonal system; no interior rewrite.
- Block A = new **use-cases in `ariadne-graph`** (`test_impact`, `api_surface_diff`, `fitness`) + thin `Visibility`/`attributes`/git inputs already present, surfaced through existing MCP + CLI adapters and the warm catalog projection. No new crate.
- Block B = new **driving adapters**: an HTTP host (`ariadne-http`, axum) that is a thin daemon client and serves the explorer static assets + existing SVG; a `review` CLI subcommand composing A's use-cases; a GitHub Action (repo YAML) wrapping the CLI. The explorer is static assets, not a crate.
- Block C = a **versioned public API** (HTTP reusing B's host + gRPC via tonic/existing prost), an **embeddable SDK** (semver'd facade over `ariadne-core`/`ariadne-graph`, guarded by cargo-public-api), and a **WASM plugin host** (wasmtime) for third-party analytics. New surfaces, all adapters; SDK is a re-export facade, not new logic.
Dataflow is unchanged: watcher → daemon invalidates Salsa → warm petgraph → clients query over the local socket. A's use-cases read that graph; B/C add new client transports in front of it.
</architecture>

<tech_inventory>
| tech | version pinned | block | role | source verified this session |
|---|---|---|---|---|
| axum | 0.8.9 (2026-04-14) | B, C | loopback HTTP/JSON host + static serving | https://docs.rs/crate/axum/latest ; https://github.com/tokio-rs/axum/blob/main/examples/static-file-server/src/main.rs |
| tower-http | 0.6.11 (2026-05-18) | B | `ServeDir`/`ServeFile` static + SPA fallback | https://docs.rs/crate/tower-http/latest ; https://benw.is/posts/serving-static-files-with-axum |
| tonic | 0.14.6 (2026-05-07) | C | gRPC server over existing prost | https://docs.rs/crate/tonic/latest |
| wasmtime | 45.0.1 (2026-06-05) | C | sandboxed WASM analytics plugins | https://docs.wasmtime.dev/api/wasmtime/ ; https://crates.io/crates/wasmtime |
| cargo-public-api | 0.52.0 | C | CI guard on the SDK public surface | https://github.com/cargo-public-api/cargo-public-api ; https://docs.rs/crate/cargo-public-api/latest |
| GitHub Actions (`gh pr comment`) | n/a (platform) | B | post the PR-risk report; `permissions: pull-requests: write` | https://docs.github.com/actions ; https://github.com/cli/cli/issues/8374 |
| (existing) tree-sitter / gix / redb / salsa / prost | v1 pins | A | call graph, git diff, storage, incremental, protobuf | post-v1-roadmap plan.md `<tech_inventory>` |
</tech_inventory>

<risks>
| id | risk | likelihood | mitigation |
|---|---|---|---|
| AR1 | block seed files drift into speculative tiers, violating "general sense only" | medium | each block file lists *candidate capabilities*, not committed tiers; tier cuts are produced only by the per-block `/spec-plan` |
| AR2 | API-surface-diff (A) needs the symbol surface at two refs, risking a full re-index of the base | medium | bound to files changed in the diff — only changed files can change the public surface; reuse per-file derivation + gix base blobs (resolved in block A `/spec-plan`) |
| AR3 | HTTP/explorer surface widens the attack/footprint area | medium | read-only, loopback-bound, off by default; no auth needed while local-first (constraint) |
| AR4 | C's WASM plugin ABI is large new surface | medium | C is last; ABI fixed by an ADR in block C's `/spec-plan`; ships behind a feature flag |
| AR5 | a product (PR bot) implies CI/hosted creep | low | the CLI does all analysis locally; the Action is thin `gh` glue, no service |
</risks>

<verification>
Arc-level: every v1 + post-v1 audit stays green and the ariadne_v2 self-index dogfood stays green throughout. Each block is proven by its own `/spec-plan` expansion's `<verification>`; the arc is "done" when all three blocks have audited PASS tiers. Per-block proof intent: A — golden tests that affected-tests, api-diff verdict, and fitness violations match hand-computed expectations on the 15-language fixtures; B — the explorer renders the self-index in a browser (real run), the HTTP API answers each endpoint, the review CLI emits a correct report on a real branch and the Action posts it; C — a sample external crate links the SDK, a sample gRPC client queries the API, and a sample WASM plugin runs sandboxed. No block is "done" on type-check alone [src: CLAUDE.md `<rules>` validation-by-execution].
</verification>

<blocks>
Run these in order; each opens a fresh planning session that designs deep tiers for one block:
- A: `/spec-plan .claude/plans/intelligence-platform/block-a-deepen-brain.md`
- B: `/spec-plan .claude/plans/intelligence-platform/block-b-reach-products.md` (after A's tiers land)
- C: `/spec-plan .claude/plans/intelligence-platform/block-c-platform-foundation.md` (after B's tiers land)
</blocks>

<sources>
- post-v1 roadmap (precedent + inherited decisions): .claude/plans/post-v1-roadmap/plan.md
- v1 plan: .claude/plans/ariadne-core/plan.md
- axum / static serving: https://docs.rs/crate/axum/latest ; https://github.com/tokio-rs/axum/blob/main/examples/static-file-server/src/main.rs
- tower-http: https://docs.rs/crate/tower-http/latest
- tonic: https://docs.rs/crate/tonic/latest
- wasmtime: https://docs.wasmtime.dev/api/wasmtime/
- cargo-public-api: https://github.com/cargo-public-api/cargo-public-api
- SemVer taxonomy: https://doc.rust-lang.org/cargo/reference/semver.html ; https://github.com/obi1kenobi/cargo-semver-checks
- Test impact analysis: https://martinfowler.com/articles/rise-test-impact-analysis.html ; https://arxiv.org/pdf/1812.06286
- Fitness functions: "Building Evolutionary Architectures" (Ford/Parsons/Kua); ArchUnit https://www.baeldung.com/java-archunit-intro
- GitHub Actions PR comment: https://docs.github.com/actions
- Hexagonal Architecture (Cockburn, 2005): https://alistair.cockburn.us/hexagonal-architecture/
</sources>
