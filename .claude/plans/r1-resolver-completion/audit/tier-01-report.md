---
tier_id: tier-01
audited: 2026-06-05
verdict: PASS
commit: a2f6b45f8e984731b2eb1e14a7277db27c441504
---

<scope>
Audited tier-01 (`call-shape-gate`) of `r1-resolver-completion`: capture each call
site's syntactic shape (`Free`/`Method`/`Path`) in the parser, mirror it as a u8
on the salsa `CallRaw` input at the cli/daemon composition roots, and gate the
cross-crate `unambiguous-global` fallback in `resolve_edges` to `Free` calls only.

Diff scoped to the tier `<files>` (working tree vs HEAD `a2f6b45`; the tier is
`status: completed` but uncommitted — the audit gates the commit). Changed:
`ariadne-parser` facts.rs + lib.rs + 14 `.scm` grammars + 11 fact snapshots + new
`tests/call_shape.rs`; `ariadne-salsa` derive.rs/derived.rs/db.rs + 3 test files;
`convert_facts` in cli `domain/mod.rs` and daemon `domain/facts.rs`. No file
outside the tier scope was touched. The held docgen tier-03 render stays withheld
(step 9 / D4 defer it to tier-02) — `architecture_section` is unchanged.
</scope>

<checks_run>
- Read every changed source file end-to-end; read new `call_shape.rs` in full.
- `cargo nextest run -p ariadne-parser -p ariadne-salsa -p ariadne-daemon` →
  **103 passed, 0 failed** (1 leaky, benign). Includes both spike no-edge tests,
  the beta→alpha recall test, same-crate, ambiguous-no-edge, and the
  fresh==incremental / warm==cold parity suites.
- `cargo nextest run -p ariadne-parser --test call_shape` → **9 passed** (rust,
  cpp, c, ts, js, python, go, csharp, java; kotlin correctly omitted as inert).
- `cargo test --test architecture` → 1 passed (hexagonal invariants hold).
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` → clean.
- `cargo fmt --all --check` → clean. `cargo deny check` → advisories/bans/
  licenses/sources ok (no new dependency; only pre-existing license-not-
  encountered warnings).
- Determinism dogfood: `ariadne index . --fresh` twice → **3317 edges both runs**
  (identical: files 383, symbols 3624).
- Gate-effect dogfood (decisive): stashed the gate, re-indexed at HEAD resolver →
  **3953 edges** pre-gate; restored → **3317** post-gate = **636 phantom
  cross-crate edges removed**. Restored gated index + daemon afterward; reverted
  the `docs/codebase-overview.{md,svg}` churn the `doc` probe produced.
- Memory: `mem` reports no table over the 256 MiB budget; the `warm_graph_tables_
  stay_within_the_per_table_budget` probe passed. Delta is +1 byte/call site (u8).
- Structural proof (read_symbol `resolve_edges`): for Method/Path,
  `cross_crate_ok=false` ⇒ `resolved = same_file.or_else(same_crate)`, both bound
  to the caller's own crate ⇒ a Method/Path edge can never be cross-crate. The
  hard-fail "Method/Path cross-crate survivor" is structurally impossible.
</checks_run>

<findings>
| id | category | severity | location | problem | fix |
|---|---|---|---|---|---|
| F1 | plan_adherence | INFO | tier-01-call-shape-gate.md:31-32 vs queries/kotlin.scm | The `<files>` brace list omits `kotlin.scm`, yet `<steps>` step 3 requires its relabel and the diff includes it (`@call.callee`→`@call.free`). | Add `kotlin` to the tier `<files>` grammar list; no code change (the relabel is correct and inert — kotlin's only shape is free). |
</findings>

<verdict>
**PASS.** Zero FAIL findings. The implementation matches D1–D3 exactly: shape is
read from each grammar's own call sub-pattern by capture-name suffix (D2), crosses
into salsa as a `u8` mirror mapped at the composition root with no `ariadne-parser`
dep (D3), and gates the cross-crate fallback to `Free` (D1). Hexagonal boundary,
determinism, and warm==cold/incremental==fresh parity are preserved. Every
`<verification>` command re-ran green. The gate removes 636 real phantom edges
while the unambiguous-global recall edge (beta→alpha) survives; per-language
snapshots confirm correct shape labels (e.g. ts `Math.sqrt`→Method, bare imported
`join`→Free; rust `Type::new`→Path). The recall/precision boundary the gate trades
away (Free-call name collisions) is the documented R1 limit (SCIP territory), not a
gate leak. Exit-criterion #5's "instability >0.7 printed from architecture_section"
is intentionally deferred to tier-02 (render re-enable); its substance — e2e/cli
phantom cross-crate afferent removed — is verified via the 636-edge drop and the
structural impossibility of a Method/Path cross-crate survivor.
</verdict>

<next_steps>
None blocking. Optional: fix F1 (add kotlin to the tier `<files>` list). The work
is ready to commit as the resolver fix only (parser + salsa + tests); tier-02 lands
the held docgen render and flips docgen tier-03 to completed.
</next_steps>

<sources>
- Gate logic: crates/ariadne-salsa/src/derive.rs:252-328 (resolve_edges, cross_crate_ok)
- Shape capture: crates/ariadne-parser/src/adapters/treesitter/facts.rs:122-160,373-405
- Composition-root mapping: crates/ariadne-cli/src/domain/mod.rs:499-535; crates/ariadne-daemon/src/domain/facts.rs:101-137
- Tests: crates/ariadne-parser/tests/call_shape.rs; crates/ariadne-salsa/tests/scoped_resolution.rs
- tree-sitter negated-field `!object`: https://tree-sitter.github.io/tree-sitter/using-parsers/queries/1-syntax.html
- salsa Update auto-impl (u8, not fieldless enum): https://docs.rs/salsa/0.26.2/salsa/trait.Update.html
- Precedent: docs/adr/0024-scoped-call-resolution.md
</sources>
</content>
