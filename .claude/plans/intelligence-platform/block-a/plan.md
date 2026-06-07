---
slug: intelligence-platform/block-a
title: Block A — deepen the brain (test-impact, API-surface diff, architecture fitness)
created: 2026-06-07
owners: [user, claude]
review: [user, codex?]
single_tier: false
tiers: [tier-01-test-impact, tier-02-api-surface-plumbing, tier-03-api-diff, tier-04-fitness]
---

<context>
Expands the Block A seed [src: .claude/plans/intelligence-platform/block-a-deepen-brain.md] into audited tiers. Inherits all arc constraints/tech [src: .claude/plans/intelligence-platform/plan.md].
Problem: the graph answers where/what/who-calls/impact but not three product-relevant questions — which tests a change affects, whether a diff is API-breaking, and whether the architecture still obeys its rules. All three are derivable from assets already in the graph; Block A adds the deriving use-cases, not new analysis machinery [src: arc plan.md AD2].
Success (measurable): three new deterministic `ariadne-graph` use-cases — `affected_tests`, `api_surface_diff`, `fitness_check` — each surfaced through MCP + CLI (+ warm catalog where it applies), each golden-tested against hand-computed expectations on the 15-language fixtures, with the ariadne_v2 self-index dogfood green.
In scope: test-impact reachability (A1), API-surface/semver diff (A2), architecture fitness engine (A3). Out of scope (this block): coverage ingest, data-flow/taint, cross-repo, any new heavy analysis framework [src: seed `<context>`; arc plan.md `<context>`].
</context>

<constraints>
- Deterministic — no LLM/embedding/inference; output byte-identical across runs (sorted collections, no clock/RNG), mirroring `diff_blast` [src: crates/ariadne-graph/src/diff_blast.rs:14-18; arc plan.md `<constraints>`].
- Hexagonal + TDD: `ariadne-core` declares ports; adapters implement; adapters never depend on each other; a failing test precedes implementation [src: CLAUDE.md `<rules>`; tests/architecture.rs].
- New analytics are `ariadne-graph` use-cases (methods on `GraphIndex` or free functions over its inputs), re-exported from the façade `lib.rs`. The CLAUDE.md ≤200-line cap is scoped to authored doc/spec files (skills, rules, plans, tiers, audits); `ariadne-graph` use-case source is not capped — precedent: `docgen_insights` 578, `docgen` 472 lines under prior PASS audits [src: CLAUDE.md `<rules>`; crates/ariadne-graph/src/lib.rs:1-49].
- Surfacing mirrors existing tools: MCP `tools/<name>.rs` + `types.rs` wire type + `server.rs` `#[tool]`, with both warm (catalog) and cold (storage) paths; CLI `commands/<name>.rs` in the clap tree [src: crates/ariadne-mcp/src/tools/diff_blast.rs; crates/ariadne-cli/src/commands/mod.rs].
- No `gix`/`tree-sitter`/`redb`/`prost` type crosses a crate's public API; adapters return owned `ariadne-core`/`ariadne-graph` types [src: crates/ariadne-git/src/lib.rs:1-10; docs/folder-layout.md rule 4].
- All v1 SLOs hold: cold full-index <60s, incremental p95 <500ms, query p95 <100ms, warm query p95 <10ms, <4GB RAM on 100K files [src: arc plan.md `<constraints>`].
- No new workspace dependency beyond those already pinned (toml 0.9, glob 0.3.3, gix =0.84.0) [src: crates/*/Cargo.toml]; a new dep stops and asks the user [src: CLAUDE.md `<rules>`].
</constraints>

<decisions>
**D1 — A1 derives test-impact from the static call graph only (no coverage ingest).** Reverse-reachability over existing call/ref edges is deterministic and golden-testable; lcov/llvm-cov adds a new dependency and a staleness/non-determinism surface that violates the determinism constraint and the seed scope-out [src: seed open_questions A1; arc plan.md `<constraints>`; test-impact technique https://martinfowler.com/articles/rise-test-impact-analysis.html]. *Rejected:* coverage ingest (deferred to a later block).
**D2 — A1 test classification is a pure `ariadne-graph` function over `attributes` + path, not a parser fact.** A symbol is a test root from its `Decl.attributes` (Rust `#[test]`, Java `@Test`) plus per-language path conventions (`*_test.go`, `*.test.ts`, `test_*.py`, `*Test.java`); these inputs are already on every record [src: crates/ariadne-parser/src/adapters/treesitter/facts.rs:99-106; crates/ariadne-daemon/src/domain/catalog.rs:48-53]. *Rejected:* per-parser test tagging — touches all 15 grammars + a record field, against D-AD2 [src: arc plan.md AD2].
**D3 — A2 represents a public symbol's signature as the normalized text of its declaration-header source slice (decl start → body-open delimiter), derived symmetrically by re-parsing both base and head blobs with tree-sitter.** `SymbolRecord` stores no signature text [src: crates/ariadne-core/src/domain/records.rs:37-59], so the parser adapter — where per-grammar body-delimiter knowledge already lives — slices the header and returns it on `PublicSymbol`; the classifier compares headers by equality. Both sides use the SAME tree-sitter fact path; stored SCIP-refined visibility is never mixed in, so no phantom visibility diffs arise. *Rejected:* (a) a persisted `signature_hash` field — a v7→v8 migration + 15 parsers, against AD2; (b) name+kind+visibility identity only — misses signature-change=major; (c) hashing the header — an unneeded step on a changed-file-bounded surface, and raw headers keep golden diffs readable [src: seed open_questions A2; https://doc.rust-lang.org/cargo/reference/semver.html].
**D4 — A2 reconstructs the base surface by re-parsing the base blobs of changed files only.** Only changed files can change the public surface, so a new `ariadne-git` `read_blobs_at(rev, paths)` fetches each changed file's base blob and `ariadne-parser::public_surface` re-extracts its surface — bounded to the diff, never stale, no new storage [src: arc plan.md AR2; gix idiom crates/ariadne-git/src/adapters/gix/diff.rs:148-160]. *Rejected:* a persisted prior-surface snapshot — adds a table + migration + staleness.
**D5 — A3 reads rules from a separate `ariadne-fitness.toml`; the engine is a pure `ariadne-graph` function over resolved inputs.** The TOML (toml 0.9) + path-glob (glob 0.3.3) parse and the glob→layer resolution happen at the CLI composition root; the pure engine receives a layer-assignment map + dependency rules + thresholds and emits violations, reusing `coupling`/`cycles` [src: crates/ariadne-graph/src/lib.rs:33-35; ArchUnit `layeredArchitecture` https://www.baeldung.com/java-archunit-intro]. *Rejected:* a `[fitness]` section in tool config — couples rules to config and grows `config.rs` (Ca 305).
**D6 — A2 runs entirely in the querying process (MCP server / CLI), never via the warm daemon, so the daemon stays git-free.** `api_surface_diff` is pure over two `PublicSymbol` lists and needs no warm graph, so — unlike `diff_blast` — there is no daemon leg: the MCP tool handler reads blobs (git) + extracts surfaces (parser) + classifies (graph) in-process. `PublicSymbol` lives in `ariadne-core` so `ariadne-parser` produces it and `ariadne-graph` consumes it with no cross-adapter dep. This requires `ariadne-mcp → ariadne-parser` (a driving→driven edge, permitted, mirroring the existing `ariadne-mcp → ariadne-git`; documented in an ADR), and keeps `ariadne-daemon ↛ ariadne-git` intact [src: tests/architecture.rs:108-154; docs/adr/0023]. *Rejected:* a `DaemonQuery::ApiSurfaceDiff` warm leg — would force git into the daemon, violating the tested invariant.
</decisions>

<architecture>
All three layer onto the current hexagonal system; no interior rewrite, no new crate.
- `ariadne-core` gains: `PublicSymbol { name, kind, visibility, signature }` domain type (shared so `ariadne-parser` produces it and `ariadne-graph` consumes it without a cross-adapter dep).
- `ariadne-graph` gains: `test_impact.rs` (`classify_test_symbols` + `affected_tests` on `GraphIndex`), `api_surface.rs` (pure `api_surface_diff(base, head) -> ApiDiffReport`/`SemverBump`), `fitness.rs` (`FitnessRules`/`fitness_check` engine). Each re-exported from `lib.rs`.
- `ariadne-git` gains: `read_blobs_at(repo_root, rev, &[path]) -> Vec<(String, Vec<u8>)>` (owned bytes, no `gix` type leaks). `ariadne-parser` gains: `public_surface(lang, &[u8]) -> Vec<PublicSymbol>` (parse → filter public `Decl`s → slice declaration header).
- A1/A3 surface as MCP tools over the warm/cold graph the existing tools query (A1 needs git for the diff — mcp already links it; A3 needs only the graph + a cheap rules-file read). A2 runs entirely in the querying process (D6): the MCP `api_surface_diff` handler + CLI `api-diff` command read base/head blobs (git) → `public_surface` (parser) → `api_surface_diff` (graph), no daemon leg. Requires `ariadne-mcp → ariadne-parser`.
- Surfaces: MCP `affected_tests`, `api_surface_diff`, `fitness_report`; CLI `affected-tests <spec>`, `api-diff <base>..<head>`, `fitness check` (non-zero exit on violation). Dataflow otherwise unchanged.
</architecture>

<tech_inventory>
| tech | version pinned | role | source verified this session |
|---|---|---|---|
| gix | =0.84.0 | base-blob read (`read_blobs_at`) reusing the repo's existing rev/tree/blob idiom | crates/ariadne-git/src/adapters/gix/diff.rs:72-78,148-160 ; https://docs.rs/gix/0.84.0 |
| tree-sitter (via ariadne-parser) | v1 pin | re-parse base/head blobs → `Decl` (visibility, attributes, def span) | crates/ariadne-parser/src/lib.rs:11-18 ; facts.rs:90-110 |
| toml | 0.9 | parse `ariadne-fitness.toml` | crates/ariadne-cli/Cargo.toml:22 |
| glob | 0.3.3 | layer path-glob matching (A3) | crates/ariadne-mcp/Cargo.toml; crates/ariadne-mcp/src/tools/search_code.rs |
| ariadne-parser → ariadne-mcp dep | workspace | new driving→driven edge so the MCP server runs `public_surface` in-process (D6); ADR in tier-03 | tests/architecture.rs:121-147 (driving→driven permitted; mcp→git precedent) |
| Cargo SemVer reference | n/a (spec) | A2 verdict taxonomy (removed/sig-change=major, added=minor) | https://doc.rust-lang.org/cargo/reference/semver.html |
</tech_inventory>

<risks>
| id | risk | likelihood | mitigation |
|---|---|---|---|
| BR1 | A2 declaration-header slicing is fuzzy for multi-line/macro decls per language | medium | per-language header-delimiter table in `api_surface.rs`; golden tests per fixture language; whole-span hashing rejected (over-reports body changes as breaks) |
| BR2 | A1 test classification misses a language convention → under-reports affected tests | medium | explicit per-`Lang` classification table; golden test asserting a known fixture test classifies in each of the 15 languages |
| BR3 | re-parsing base blobs on every `api-diff` risks the <500ms incremental SLO | low | bounded to changed files only (D4); measured on a multi-file diff in tier-03 verification |
| BR4 | base(tree-sitter) vs head(stored SCIP visibility) mismatch yields phantom diffs | medium | D3: both sides re-parsed via the same tree-sitter path; stored visibility never mixed into the comparison |
| BR5 | A3 engine duplicating `coupling`/`cycles` logic instead of reusing it | low | engine consumes existing `CouplingReport`/`CycleReport`; tier-04 forbids new metric code |
| BR6 | A2's MCP path tempts a daemon git leg, breaking the tested daemon-git-free invariant | medium | D6: `api_surface_diff` runs in the querying process only (no `DaemonQuery` variant); tier-03 verification asserts `cargo test --test architecture` stays green and `ariadne-daemon ↛ ariadne-git` |
</risks>

<verification>
Block A is done when all four tiers have audited PASS, and on the 15-language fixtures + ariadne_v2 self-index: (1) `affected-tests` returns the hand-verified test set for a seeded change and exits clean; (2) `api-diff` returns the correct none/patch/minor/major verdict for seeded removed/added/signature-changed public items; (3) `fitness check` flags a seeded layering violation with non-zero exit and passes (exit 0) on the clean self-index. All outputs deterministic (re-run byte-identical); every tier writes its failing test first [src: CLAUDE.md `<rules>` validation-by-execution + TDD]. Standard gates per tier: `cargo nextest run --workspace`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo fmt --all --check`, `cargo test --test architecture`.
</verification>

<sources>
- Block A seed + arc master: .claude/plans/intelligence-platform/block-a-deepen-brain.md ; .claude/plans/intelligence-platform/plan.md
- Test impact analysis: https://martinfowler.com/articles/rise-test-impact-analysis.html ; https://arxiv.org/pdf/1812.06286
- SemVer taxonomy: https://doc.rust-lang.org/cargo/reference/semver.html ; https://github.com/obi1kenobi/cargo-semver-checks
- Fitness functions / ArchUnit: https://www.baeldung.com/java-archunit-intro
- gix: https://docs.rs/gix/0.84.0 ; repo idiom crates/ariadne-git/src/adapters/gix/diff.rs
- Existing use-case + surfacing patterns: crates/ariadne-graph/src/diff_blast.rs ; crates/ariadne-mcp/src/tools/diff_blast.rs ; crates/ariadne-cli/src/commands/mod.rs
</sources>
