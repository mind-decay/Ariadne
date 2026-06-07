---
tier_id: tier-04
audited: 2026-06-07
verdict: PASS
commit: 0af641eb20fe515e34782d60fa539ff1169b7c58
---

<scope>
Block A tier-04 (A3) — declarative architecture-fitness engine: `ariadne-fitness.toml`
→ `ariadne fitness check` + MCP `fitness_report`. Diff is uncommitted in the working
tree, scoped against HEAD `0af641e`.
Reviewed exactly the tier's `<files>`:
- new: `crates/ariadne-graph/src/fitness.rs` (engine, 422 L), `crates/ariadne-cli/src/commands/fitness.rs`,
  `crates/ariadne-cli/tests/fitness.rs`, `crates/ariadne-mcp/src/tools/fitness_report.rs`,
  `crates/ariadne-mcp/tests/tools_fitness_report.rs`, `docs/adr/0028-fitness-rules-format.md`,
  `ariadne-fitness.toml`.
- modified: graph `lib.rs` (re-export), cli `main.rs`/`commands/mod.rs`, mcp
  `server.rs`/`types.rs`/`tools/mod.rs`/`Cargo.toml` (+`toml = "0.9"`), mcp `handshake.rs`
  + two snapshots (tool count 21→22), `Cargo.lock`.
Index freshness confirmed via `project_status` (rev 22, not stale); cross-crate edges
spot-checked with `find_references`/`blast_radius`.
</scope>

<checks_run>
- `cargo fmt --all --check` → exit 0.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` → exit 0, 0 warnings.
- `cargo test --test architecture` → `architecture_invariants_hold` ok (mcp gained only the
  external `toml` crate, no new cross-crate workspace edge; daemon stays git-free).
- `cargo nextest run --workspace` → 509 passed, 0 failed, 19 skipped.
- Targeted `-E 'test(fitness)'` → all 10 fitness tests ran (not skipped) and passed: 6 graph
  unit (forbidden→1, clean→0, file-pair dedupe, cycle threshold, instability threshold,
  determinism), 2 cli e2e (seeded-violation non-zero + byte-identical re-run; clean exit 0),
  2 mcp e2e (real rmcp server: forbidden flagged; clean passes).
- Real run: `ariadne fitness check --root .` (cold, AUTOSPAWN=0) → exit 0, `{"ok": true,
  "violations": []}`; two consecutive runs byte-identical (determinism).
- Engine API reuse verified: `cat.symbols`/`paths`/`path_of`/`meta_of`/`graph`, `ModuleSpec`,
  `cycle_report().cycles[].members`, `coupling_report().rows[].instability` all match
  signatures in `catalog.rs`/`coupling.rs`/`cycles.rs` (BR5 — no new metric code).
- Parity (exit criterion #5): cli `fitness::run` calls `ariadne_mcp::tools::fitness_report::handle`
  (cli/commands/fitness.rs:39) — the identical function the MCP `#[tool]` calls (server.rs),
  over a catalog built the same way; parity is by construction and the mcp e2e exercises it.
- Glob binding verified (not vacuous): standalone glob 0.3.3 test confirms
  `crates/ariadne-cli/**` matches root-relative paths; `cat.paths` are root-relative.
  Probe forbidding `composition-root → driven` returns 0 — consistent with the resolver's
  documented cross-crate edge coverage (see INFO-1), not a glob failure.
</checks_run>

<findings>
| id | category | severity | location | problem | fix |
|----|----------|----------|----------|---------|-----|
| INFO-1 | correctness | INFO | `crates/ariadne-graph/src/fitness.rs:131-173`; `ariadne-fitness.toml:45-73` | The forbidden-dependency check fires only where the graph holds a resolved symbol→symbol edge crossing a layer; cross-crate *type/struct/import-only* usages produce no such edge (e.g. `RedbStorage`/`GraphIndex` have empty blast radius), so the dogfood gate is a *weaker* signal than `tests/architecture.rs` (which checks declared crate deps). The clean self-index pass is truthful (the forbidden directions are genuinely edge-empty), but assurance is narrower than "encodes this repo's hexagonal layers" implies. | Out of tier scope (D5/BR5 reuse the existing graph). If completeness matters later, drive layer rules off import/Cargo-dep edges in a follow-up; for now note the limitation where the gate is documented. |
| INFO-2 | plan_adherence | INFO | `crates/ariadne-cli/Cargo.toml` (unchanged); `crates/ariadne-cli/src/commands/fitness.rs` | The plan `<files>`/step 4 said cli gains `glob = "0.3.3"` and parses/resolves rules itself; the implementation instead delegates entirely to the mcp shared `handle`, so cli adds neither `glob` nor `toml`. A literal divergence from the written `<files>`. | None needed — the shared-handle approach (ADR-0027 precedent, parity by construction) is justified in the code comments and is the cleaner design; adding an unused dep would be worse. |
</findings>

<verdict>
PASS. Zero FAIL findings. Every `<exit_criteria>` item independently re-verified:
(1) workspace nextest green; (2) graph unit tests prove forbidden-edge→one-violation and
clean→none plus thresholds; (3) `fitness check` exits 0 on the clean self-index; (4) a seeded
forbidden dependency exits non-zero, lists the violation, and re-runs byte-identical;
(5) MCP `fitness_report` is parity-by-construction with the cli (shared handle) and passes its
e2e, while `cargo test --test architecture`, clippy `-D warnings`, and `fmt --check` are green
and ADR-0028 is present. The engine is deterministic (sorted violations, total-order key),
reuses existing coupling/cycle analytics with no new metric code, and leaks no adapter types.
The two INFO items are non-blocking: INFO-1 is an inherited substrate property the plan
explicitly builds on; INFO-2 is a justified, improving deviation.
</verdict>

<next_steps>
None blocking. Optional, for a future tier (not this one): consider an import/Cargo-dep-edge
source for layer rules so the fitness gate matches `tests/architecture.rs` completeness
(INFO-1), and add a one-line caveat near the `ariadne-fitness.toml` thresholds noting the
gate's edge-based reach. Tier-04 may be committed.
</next_steps>

<sources>
- `[src: .claude/plans/intelligence-platform/block-a/tier-04-fitness.md]` — tier under review
- `[src: .claude/plans/intelligence-platform/block-a/plan.md D5, BR5]` — engine scope/reuse
- `[src: docs/adr/0028-fitness-rules-format.md]` — rules format contract
- `[src: docs/adr/0027-mcp-parser-dependency.md]` — shared-handle precedent (parity)
- `[src: crates/ariadne-graph/src/{coupling.rs,cycles.rs}]` — reused analytics
- `[src: https://www.baeldung.com/java-archunit-intro]` — ArchUnit layered-architecture model
- `[src: https://docs.rs/glob/0.3.3]` — `Pattern::matches` `**` semantics (verified standalone)
- `[src: https://google.github.io/eng-practices/review/reviewer/standard.html]` — code-health bar
</sources>
