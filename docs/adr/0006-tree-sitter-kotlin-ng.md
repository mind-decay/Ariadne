# ADR-0006: Substitute tree-sitter-kotlin-ng for tree-sitter-kotlin

<status>
Accepted
Date: 2026-05-19
Decider: user
</status>

<context>
Tier-03 plan `<files>` and ADR-0002 row D2 name `tree-sitter-kotlin` as the
Kotlin grammar crate alongside seven other per-lang grammars. During
tier-03 build, the canonical `fwcd/tree-sitter-kotlin` 0.3.8 (latest
release) declares `tree-sitter ≥0.21, <0.23` in its public API
[src: <https://docs.rs/tree-sitter-kotlin>], while ADR-0002 row D2 and
tier-03 step 1 require `tree-sitter = "=0.26.8"` (published 2026-03-31,
verified in audit `tier-03-report.md` <checks_run>). Cargo refuses to
resolve a workspace that imports both crates simultaneously, so we cannot
ship Kotlin support against the v1 tree-sitter pin without replacing the
grammar crate.
</context>

<decision>
Substitute `tree-sitter-kotlin-ng = "=1.1.0"` (under the
`tree-sitter-grammars` org, owner `amaanq`) for `tree-sitter-kotlin`. The
`-ng` ("next-gen") fork is the rewritten grammar published against the
modern tree-sitter 0.24+ ABI [src:
<https://docs.rs/tree-sitter-kotlin-ng>] and exports the standard
`LANGUAGE: LanguageFn` constant used by every other grammar in this
adapter. ADR-0002 row D2 and `plan.md` `<tech_inventory>` row "tree-sitter
+ grammars …" are amended in lockstep with this ADR.
</decision>

<rationale>
- **Reliability.** `fwcd/tree-sitter-kotlin` last published a 0.3.x release
  pinning tree-sitter <0.23 and has not advanced to the 0.24+ ABI in over a
  year [src: <https://docs.rs/tree-sitter-kotlin>]; staying on it would
  fragment the tree-sitter pin across grammars and block any future bump.
  `tree-sitter-kotlin-ng` lives under the `tree-sitter-grammars` org, the
  same org that maintains the C# and Java grammars we already use
  [src: <https://github.com/tree-sitter-grammars/tree-sitter-kotlin>].
- **Maintainability.** Single tree-sitter major version across all
  grammars keeps the workspace lockfile coherent; one upgrade path covers
  every language. Per-lang crate pins live in `crates/ariadne-parser/
  Cargo.toml` and only bump through an ADR (CLAUDE.md `<rules>`).
- **Efficiency.** The `-ng` grammar exposes the same `LANGUAGE` constant
  shape as `tree-sitter-typescript`, `-javascript`, `-python`, `-rust`,
  `-go`, `-java`, `-c-sharp`, so `ParserRegistry::new()` registers it via
  the same code path with no per-lang special case [src: `crates/
  ariadne-parser/src/adapters/treesitter/registry.rs`].
- **Scalability.** Tier-04 (Salsa) and tier-05 (SCIP) compose against the
  per-lang `Language` value, not the grammar crate name; the substitution
  is invisible downstream of the registry.
</rationale>

<alternatives>
- **Stay on `tree-sitter-kotlin` and pin tree-sitter to <0.23.** Rejected
  — would force every other 0.24+ grammar to downgrade, losing parser
  improvements and breaking the proptest equivalence that depends on
  tree-sitter 0.26's `ParseOptions` API [src: <https://docs.rs/
  tree-sitter/0.26.8/tree_sitter/struct.ParseOptions.html>].
- **Drop Kotlin from v1.** Rejected — Kotlin is part of the v1 language
  set in `plan.md` `<context>` (TS/JS, Python, Rust, Go, Java/Kotlin, C#);
  removing it shrinks the contract.
- **Fork `tree-sitter-kotlin` ourselves.** Rejected — duplicates work
  already done upstream by `amaanq/tree-sitter-grammars`; adds vendoring
  burden with no offsetting maintainability gain.
</alternatives>

<consequences>
- `crates/ariadne-parser/Cargo.toml` pins `tree-sitter-kotlin-ng = "=1.1.0"`
  with a comment citing this ADR; the legacy `tree-sitter-kotlin` name is
  not present anywhere in the workspace.
- `plan.md` `<tech_inventory>` row "tree-sitter + grammars …" reads
  "kotlin-ng" instead of "kotlin"; ADR-0002 row D2 carries the same
  rename. Cross-reference: this ADR.
- Any future bump of `tree-sitter-kotlin-ng` requires a superseding ADR
  (CLAUDE.md `<rules>`).
- If the upstream rename reverses — i.e., the `fwcd` crate adopts the new
  ABI — a follow-up ADR may consolidate back; not anticipated for v1.
</consequences>

<sources>
- `[src: .claude/plans/ariadne-core/plan.md `<decisions>` D2, `<tech_inventory>`]`
- `[src: .claude/plans/ariadne-core/tier-03-parser.md step 1]`
- `[src: crates/ariadne-parser/Cargo.toml]`
- `[src: docs/adr/0002-tech-stack.md]`
- `[src: .claude/plans/ariadne-core/audit/tier-03-report.md F-list I3]`
- `[src: https://docs.rs/tree-sitter-kotlin-ng]`
- `[src: https://docs.rs/tree-sitter-kotlin]`
- `[src: https://github.com/tree-sitter-grammars/tree-sitter-kotlin]`
- `[src: https://docs.rs/tree-sitter/0.26.8/tree_sitter/struct.ParseOptions.html]`
- `[src: CLAUDE.md `<rules>` "Do not introduce a new dependency…"]`
</sources>
