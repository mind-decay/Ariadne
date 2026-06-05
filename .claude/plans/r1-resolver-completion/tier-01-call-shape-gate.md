---
tier_id: tier-01
title: Capture call shape; gate the cross-crate unambiguous-global tier to free calls
deps: []
exit_criteria:
  - "A spike test seeds a Method-shaped (and a Path-shaped) callee with one unique cross-crate definition and asserts NO edge; it is red on the current resolver, green after the gate"
  - "resolve_edges fires the unambiguous-global fallback only when the call site's CallKind is Free; same-file/same-crate resolution and renders/hooks are unchanged"
  - "Per-language parser tests assert each grammar's call sub-patterns map to the right CallKind (Free/Method/Path)"
  - "scoped_resolution.rs recall test (beta→alpha, seeded Free) and same-crate test stay green; ambiguous-no-edge test stays green"
  - "Fresh dogfood re-index: ariadne-e2e and ariadne-cli cross-crate afferent near-zero, instability > 0.7 (printed from architecture_section); index twice → identical edge set"
  - "cargo nextest run -p ariadne-parser -p ariadne-salsa -p ariadne-daemon, architecture test, clippy, fmt all green; memory_report() delta reported"
status: completed
completed: 2026-06-04
---

<context>
Completes the partial R1 fix. The resolver's cross-crate `unambiguous-global`
fallback fires for every call shape; a `Method`/`Path` callee (`socket.connect()`,
`Foo::new()`) whose bare name has one workspace definition binds cross-crate —
the phantom. Capture the shape each grammar already separates into distinct call
sub-patterns and refuse the cross-crate fallback for non-free shapes (plan D1/D2).
Full rationale + rejected alternatives: plan.md. The held docgen tier-03
working-tree changes are stashed for this tier (plan R4); tier-02 lands them.
</context>

<files>
- crates/ariadne-parser/src/adapters/treesitter/facts.rs — add `CallKind {Free,
  Method, Path}`; add `kind: CallKind` to `CallSite`; in `extract`, dispatch on
  the `call.{free,method,path}` capture-name suffix instead of matching the lone
  `call.callee` [src: facts.rs:122-128,342-389].
- crates/ariadne-parser/.../queries/{rust,typescript,tsx,javascript,python,go,
  kotlin,java,csharp,c,cpp}.scm — relabel call captures per the shape table in `<steps>`.
- crates/ariadne-salsa/src/derived.rs — add `kind_byte: u8` to `CallRaw`
  [src: derived.rs:79-84].
- crates/ariadne-cli/src/domain/mod.rs + crates/ariadne-daemon/src/domain/facts.rs
  — in `convert_facts`, set `kind_byte` from the parser `CallKind`; add a
  `call_kind_byte` helper beside `decl_kind_tag` [src: mod.rs:474-522; facts.rs:76-124].
- crates/ariadne-salsa/src/derive.rs — `FileFacts.calls` carries the decoded
  `CallKind`; `resolve_edges` gates the `unambiguous` fallback to `Free`
  [src: derive.rs:79-95,220-278]; db.rs `build_changeset` threads `kind_byte`
  [src: db.rs:300-320].
- crates/ariadne-salsa/tests/{scoped_resolution,incremental,derivation}.rs +
  crates/ariadne-parser fact tests — add `kind`/`kind_byte` to seeds; new
  shape-gate tests.
</files>

<steps>
1. SPIKE (red first). In `scoped_resolution.rs`, add a test that seeds a unique
   workspace `connect` in `crates/crate_b` and a caller in `crates/crate_a` whose
   call site is Method-shaped; assert the caller has NO `References` edge. Add the
   Path-shaped twin. Run → these are GREEN-wrong today (current resolver binds via
   `unambiguous`) because the seed has no shape yet — so first thread `kind`
   through the seed helper (step 5) defaulting Method, confirm the test is RED
   against the un-gated resolver, pinning derive.rs:240-247 as the branch.
2. PARSER shape capture. Add `pub enum CallKind { Free, Method, Path }` (Copy) and
   `pub kind: CallKind` on `CallSite`. In `extract`, replace the
   `name == "call.callee"` arm with: strip prefix `call.`, match `free`/`method`/
   `path` → the `CallKind`, carry it onto the pushed `CallSite`
   [src: facts.rs:342-389].
3. RELABEL captures (capture-name = shape). Per grammar:
   - rust: `(identifier)`→`@call.free`; `(scoped_identifier name:(identifier))`→
     `@call.path`; `(field_expression field:(field_identifier))`→`@call.method`.
   - typescript/tsx/javascript: `(identifier)`→`@call.free`;
     `(member_expression property:(property_identifier))`→`@call.method`.
   - python: `(identifier)`→`@call.free`; `(attribute attribute:(identifier))`→`@call.method`.
   - go: `(identifier)`→`@call.free`; `(selector_expression field:(field_identifier))`→`@call.method`.
   - csharp: `(identifier)`→`@call.free`; `(member_access_expression name:(identifier))`→`@call.method`.
   - cpp: `(identifier)`→`@call.free`; `(field_expression …)`→`@call.method`;
     `(qualified_identifier name:(identifier))`→`@call.path`.
   - c: `(identifier)`→`@call.free`; `(field_expression …)`→`@call.method`.
   - kotlin: leading `(expression (identifier))`→`@call.free` (only shape today).
   - java: split `(method_invocation !object name:(identifier))`→`@call.free` and
     `(method_invocation object:(_) name:(identifier))`→`@call.method`
     [src: tree-sitter negated-field syntax, plan tech_inventory].
4. BOUNDARY mapping. Add `fn call_kind_byte(k: &CallKind) -> u8` (Free=0, Method=1,
   Path=2) at each composition root; set `CallRaw.kind_byte` in both `convert_facts`
   [src: mod.rs:498-504; facts.rs:100-106]. Add `kind_byte: u8` to `CallRaw`.
5. SALSA gate. Decode `kind_byte` into a derive-local `CallKind` when building
   `FileFacts.calls` (now `(String, CallKind, (u32,u32))`) in `build_changeset`
   [src: db.rs:300-320]. In `resolve_edges`, give the `resolve` closure a
   `cross_crate_ok: bool`; apply `.or_else(unambiguous)` only when `cross_crate_ok`.
   Calls pass `cross_crate_ok = matches!(kind, CallKind::Free)`; renders and hooks
   pass `true` [src: derive.rs:228-247,262-274]. Update the three seed helpers /
   `CallRaw` literals in the salsa tests (default `Free`, so the existing recall +
   same-crate tests are unchanged); seed the new step-1 tests Method/Path.
6. PARSER tests. Add/extend per-language fact tests asserting the captured
   `CallKind` for a free call, a member/method call, and (rust/cpp) a path call.
7. VERIFY locally. `cargo nextest run -p ariadne-parser -p ariadne-salsa
   -p ariadne-daemon`; `cargo test --test architecture`; clippy; fmt. Report the
   `ariadne-salsa` `memory_report()` delta vs HEAD.
8. DOGFOOD. Stop the daemon; `cargo run -p ariadne-cli -- index <repo>` twice;
   confirm identical edge counts (determinism). Print crate Ca/Ce/I from
   `architecture_section`; confirm `ariadne-e2e`/`ariadne-cli` cross-crate afferent
   near-zero and I > 0.7. Record the residual cross-crate edge count and, if any
   remain, classify them by shape (plan R1) in the audit notes.
9. Re-accept any purely edge-driven `ariadne-graph`/`ariadne-salsa` snapshot churn
   after review (NOT the rendering re-enable — that is tier-02). Commit the
   resolver fix only (parser + salsa + tests).
</steps>

<verification>
- `cargo nextest run -p ariadne-parser -p ariadne-salsa -p ariadne-daemon` →
  spike Method/Path no-edge tests green; recall (beta→alpha) + same-crate +
  ambiguous-no-edge tests green; per-shape parser tests green; warm==cold /
  incremental==fresh parity suites green.
- `cargo run -p ariadne-cli -- index <repo>` twice → identical edge set
  (determinism); `architecture_section` shows e2e/cli cross-crate afferent
  near-zero, instability > 0.7.
- `cargo test --test architecture`; `cargo clippy --workspace --all-targets
  --all-features -- -D warnings`; `cargo fmt --all --check`; `cargo deny check`.
- `memory_report()` delta for `ariadne-salsa` reported and < 256MB/table.
- Fail loudly: a surviving e2e/cli cross-crate afferent edge that is Method/Path
  shaped is a hard fail (gate leak); a dropped beta→alpha edge is a hard fail
  (over-gating).
</verification>

<rollback>
`git checkout -- crates/ariadne-parser crates/ariadne-salsa crates/ariadne-cli/src/domain/mod.rs crates/ariadne-daemon/src/domain/facts.rs`
then `cargo run -p ariadne-cli -- index <repo>` to restore the prior edge set.
The stashed docgen tier-03 changes remain stashed; no docgen file is touched here.
</rollback>
</content>
