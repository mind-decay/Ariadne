---
block_id: block-a
title: Block A — deepen the brain (qualitatively smarter graph)
arc: intelligence-platform
order: 1
deps: []
status: seed   # seed → expand via /spec-plan into tiers
expand_with: /spec-plan .claude/plans/intelligence-platform/block-a-deepen-brain.md
---

<context>
This is a **seed plan**, not a tier set. It scopes Block A at the general level so a later `/spec-plan` session designs deep, audited tiers. Read the arc master for shared constraints/tech: `.claude/plans/intelligence-platform/plan.md`.
Problem: today's graph answers "where/what/who-calls/impact" but not the three product-relevant questions the user picked — "which tests does this change affect", "is this diff API-breaking", "does the architecture still obey its rules". All three are derivable from assets already in the graph; Block A adds the use-cases that derive them.
Success: three new deterministic `ariadne-graph` use-cases, each surfaced through MCP + CLI + the warm catalog, each golden-tested against hand-computed expectations on the 15-language fixtures.
Scope (in): test-impact reachability; API-surface/semver diff; architecture fitness-function engine. Scope (out, this block): data-flow/taint (stretch — see open questions); cross-repo; any new heavy analysis framework.
</context>

<candidate_capabilities>
Each bullet is a likely tier the `/spec-plan` expansion will detail. Described in general terms only.

**A1 — Test-impact reachability (→ product: test-impact selector).**
Classify test symbols per language (Rust `#[test]`/`#[cfg(test)]` already in `attributes`; others via path/framework conventions — `*_test.go`, `*.test.ts`, `test_*.py`, `*Test.java`) and compute, per changed symbol, the set of tests that transitively reach it via reverse call/ref-edge traversal — the standard call-graph test-impact technique [src: https://martinfowler.com/articles/rise-test-impact-analysis.html ; https://arxiv.org/pdf/1812.06286]. Builds on the existing call graph + `attributes` field + the same gix diff `diff_blast_radius` already consumes. Surface: `ariadne affected-tests <ref-range>` + MCP `affected_tests` + warm-catalog projection.

**A2 — API-surface / semver diff (→ feeds product: PR-risk bot).**
Extract the public surface (symbols with `Visibility::Public` + a signature hash) and classify the delta between two git refs as none/patch/minor/major per the Rust/Cargo SemVer taxonomy — removed/signature-changed = major, added = minor [src: https://doc.rust-lang.org/cargo/reference/semver.html ; https://github.com/obi1kenobi/cargo-semver-checks]. Bounded to files changed in the diff (only those can change the surface — see arc risk AR2), reusing the per-file derivation + gix base blobs. Builds on the existing `Visibility` enum [src: `crates/ariadne-core/src/domain/types/visibility.rs`]. Surface: `ariadne api-diff <base>..<head>` + MCP `api_surface_diff`.

**A3 — Architecture fitness-function engine (→ product: fitness dashboard).**
A declarative rules file (layers as path globs; allowed/forbidden dependency directions; thresholds on coupling, cycles, complexity) checked against the graph, productising the project's own `tests/architecture.rs` idea as config-driven fitness functions [src: "Building Evolutionary Architectures" (Ford/Parsons/Kua); ArchUnit `layeredArchitecture()` — https://www.baeldung.com/java-archunit-intro]. Builds on existing `coupling_report` + cycle detection in `weak_spots`. Surface: `ariadne fitness check` (CI gate, non-zero on violation) + MCP `fitness_report`.
</candidate_capabilities>

<existing_assets>
- Call/ref edges + reverse traversal already power `blast_radius`/`diff_blast_radius` [src: existing MCP tools, recon].
- `Visibility` enum + `attributes: Vec<String>` on `SymbolRecord` (post-v1 RD10) — the inputs A2/A1 need [src: `crates/ariadne-core/src/domain/types/visibility.rs`; post-v1-roadmap RD10].
- gix tree/blob diff already wired for diff-aware blast radius [src: post-v1-roadmap RD7/tier-14].
- `coupling_report` (Ca/Ce/I/A) + cycle/god-module detection in `weak_spots` — A3's measurement layer [src: existing MCP tools].
- Warm catalog projection pattern for new analytics [src: `crates/ariadne-daemon/src/domain/catalog.rs`; tier-15a precedent].
</existing_assets>

<open_questions>
Resolve these in the `/spec-plan` expansion (do not guess now):
- A1: static reachability only, or optionally ingest real coverage (lcov/llvm-cov) for precision? Default static (deterministic, no new dep) — confirm.
- A1: per-language test-symbol classification table — which conventions/attributes per language; where it lives (parser facts vs a graph-side classifier).
- A2: signature-hash representation per language (what counts as a "signature" for non-Rust); how `#[non_exhaustive]`-style "possibly-breaking" cases map to the verdict [src: cargo SemVer reference].
- A2: exact base-ref surface reconstruction path (per-file re-derive from gix blobs vs a stored prior snapshot) to honour the <500ms incremental SLO.
- A3: rules-file format + location (new `[fitness]` TOML section vs separate `ariadne-fitness.toml`); which thresholds are first-class.
- Stretch (defer unless asked): data-flow/taint as a fourth capability → unlocks a future security scanner; out of scope for this block.
</open_questions>

<verification_intent>
Golden/insta tests on the 15-language fixtures: `affected_tests` returns the hand-verified test set for a seeded change; `api_surface_diff` returns the correct verdict for removed/added/changed public items; `fitness check` flags a seeded layering violation and exits non-zero, and passes on the clean self-index. All deterministic; no LLM. Each tier TDD: failing test first [src: CLAUDE.md `<rules>`].
</verification_intent>

<sources>
- Test impact analysis: https://martinfowler.com/articles/rise-test-impact-analysis.html ; https://arxiv.org/pdf/1812.06286
- SemVer taxonomy: https://doc.rust-lang.org/cargo/reference/semver.html ; https://github.com/obi1kenobi/cargo-semver-checks
- Fitness functions / ArchUnit: https://www.baeldung.com/java-archunit-intro
- Existing `Visibility`: `crates/ariadne-core/src/domain/types/visibility.rs`
- Arc master + inherited constraints: .claude/plans/intelligence-platform/plan.md
</sources>
