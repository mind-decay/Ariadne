---
block_id: block-c
title: Block C — platformize as a foundation others build on
arc: intelligence-platform
order: 3
deps: [block-a, block-b]
status: seed   # seed → expand via /spec-plan into tiers
expand_with: /spec-plan .claude/plans/intelligence-platform/block-c-platform-foundation.md
---

<context>
Seed plan, not a tier set — scopes Block C generally for a later `/spec-plan`. Shared constraints/tech: `.claude/plans/intelligence-platform/plan.md`.
Problem: after A makes the graph smarter and B proves products on it, the graph should be a substrate **other projects embed** — the user's "build other products on this foundation" goal. The user picked three interfaces (Q2): a language-agnostic API, an embeddable Rust SDK, and a plugin system (graph-export was deprioritised).
Success: a non-Rust client queries the graph over a stable versioned API; a sample external Rust crate links the SDK; a third-party WASM analytics plugin runs sandboxed without recompiling `ariadne`.
Run C's `/spec-plan` after B's tiers land — the public API surface depends on what A/B expose, so the tier shape is decided then (arc AD1).
Scope (in): versioned public API (HTTP + gRPC); embeddable semver'd Rust SDK; WASM plugin host. Scope (out): hosted/multi-tenant (local-first); cross-repo federation (left open by the user, separate arc if pursued).
</context>

<candidate_capabilities>
Likely tiers, general terms only. Tech is pinned now; tier breakdown is the expansion's job.

**C1 — Versioned language-agnostic API (HTTP + gRPC).**
Promote B1's loopback HTTP into a **stable, versioned** read API (explicit `/v1` namespace + schema), and add a gRPC surface on tonic 0.14.6 reusing the prost stack the SCIP layer already builds [src: https://docs.rs/crate/tonic/latest ; existing `ariadne-scip` prost usage, recon]. Lets TS/Python/web/Go clients consume the graph. Both transports are thin daemon clients (AD4); gRPC `.proto` lives beside the existing SCIP protos.

**C2 — Embeddable Rust SDK.**
A semver'd public facade over `ariadne-core` + `ariadne-graph` so Rust products link Ariadne as a library (lowest-overhead consumer). The public surface is frozen and guarded in CI by cargo-public-api 0.52.0, which lists + diffs the rustdoc-JSON public API and fails on unintended breakage [src: https://github.com/cargo-public-api/cargo-public-api ; https://docs.rs/crate/cargo-public-api/latest]. Facade is re-exports only — no new logic, honouring the `lib.rs` façade rule [src: CLAUDE.md `<architecture>`].

**C3 — Plugin system (WASM analytics + language registration).**
Third-party analytics as sandboxed WASM components via wasmtime 45.0.1 — component model (default feature), `Linker` for host functions, `bindgen!` for the ABI — so untrusted plugins run safely with no recompile and no `dylib` (single static binary holds) [src: https://docs.wasmtime.dev/api/wasmtime/ ; AD5]. A defined plugin ABI (graph-read host functions in, findings out) fixed by an ADR. Tree-sitter language grammars stay compile-time registered (separate, narrower extension point).
</candidate_capabilities>

<existing_assets>
- prost protobuf stack already built for SCIP — tonic reuses it [src: `crates/ariadne-scip`, recon].
- B1's axum HTTP host — C1 versions it rather than starting over [src: block B].
- `daemon_client` thin-client pattern for both transports [src: `crates/ariadne-cli/src/adapters/daemon_client.rs`].
- `lib.rs`-as-façade convention — the SDK formalises it with a semver guard [src: CLAUDE.md `<architecture>`].
</existing_assets>

<open_questions>
Resolve in the `/spec-plan` expansion:
- C1: API versioning + stability policy; which queries are public-v1 vs internal; gRPC `.proto` schema ownership (pure wire types in `ariadne-core`, like the IPC types).
- C1: is gRPC required day one, or HTTP-v1 first with gRPC as a later tier?
- C2: which crate is the published SDK facade (new `ariadne` umbrella crate vs promoting `ariadne-graph`); the exact frozen public surface; nightly-toolchain CI step cargo-public-api needs [src: cargo-public-api docs].
- C3: the WASM plugin ABI (WIT world); capability model (read-only graph access only?); plugin discovery/loading + feature flag; trust/signing policy.
- Federation hook: leave seams for an eventual cross-repo arc (the user did not lock single-repo) without building it here.
</open_questions>

<verification_intent>
Real-run gates: a sample non-Rust client (e.g. a small script) queries `/v1` and the gRPC endpoint and gets correct results; a sample external Rust crate compiles against the SDK and the cargo-public-api CI guard fails on a deliberate breaking change; a sample WASM plugin loads, runs sandboxed, and returns findings the host renders — none of it recompiling `ariadne`. Deterministic; no LLM. Each tier TDD [src: CLAUDE.md `<rules>`].
</verification_intent>

<sources>
- tonic gRPC: https://docs.rs/crate/tonic/latest
- wasmtime (component model, Linker, bindgen!): https://docs.wasmtime.dev/api/wasmtime/ ; https://crates.io/crates/wasmtime
- cargo-public-api: https://github.com/cargo-public-api/cargo-public-api ; https://docs.rs/crate/cargo-public-api/latest
- existing prost/SCIP stack: `crates/ariadne-scip`
- Arc master + inherited constraints: .claude/plans/intelligence-platform/plan.md
</sources>
