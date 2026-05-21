---
tier_id: tier-05
audited: 2026-05-21
verdict: PASS
commit: d44f683913102f857382c6e83ca82db0600d37b6
---

<scope>
Audit of tier-05 (CLI extension mapping, framework autodetect, component-edge
resolution) of plan `js-framework-support`. Diff under review, scoped to the
tier's `<files>`:
- `crates/ariadne-cli/src/domain/mod.rs` — `lang_for_path` re-route + adds;
  multi-layer `parse_facts`; SFC synthesized `Component`; typed edge resolver.
- `crates/ariadne-cli/src/config.rs` — `package.json` framework-dep autodetect.
- `crates/ariadne-cli/tests/index_frameworks.rs` — new CLI integration test.
- `crates/ariadne-cli/fixtures/{react,vue,svelte,astro}/` — new fixture trees.
Diff is uncommitted working-tree state on top of HEAD `d44f683`. Cross-read
(read-only) of `ariadne-parser` `parse_file`/`facts.rs`/`injection.rs` and
`ariadne-core` `lang.rs`/`records.rs` to verify the layer + edge contracts.
</scope>

<checks_run>
- `cargo build --workspace` — green.
- `cargo nextest run -p ariadne-cli` — 11/11 pass (4 new framework index
  tests + 2 new config tests + 5 prior).
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` — clean.
- `cargo fmt --all --check` — clean.
- `cargo test --test architecture` — `architecture_invariants_hold` ok.
- `RUSTDOCFLAGS="-D warnings" cargo doc -p ariadne-cli` — clean (no broken
  intra-doc links among the many added doc comments).
- Real end-to-end run: built `ariadne` binary, `init`+`index` over each fixture
  tree in a tempdir. Results — vue `{langs:["vue"],symbols:4,edges:1,parse_failures:0}`,
  svelte `{["svelte"],4,1,0}`, astro `{["astro"],3,1,0}`, react `{["tsx"],2,1,0}`.
  Symbol counts independently reconciled against each fixture (synthesized SFC
  `Component` + `<script>`/frontmatter decls).
- Exit criteria 1-5: each verified — see `<verdict>`.
</checks_run>

<findings>
| id | category | severity | file:line | problem | fix |
|---|---|---|---|---|---|
| F1 | docs | INFO | `crates/ariadne-cli/src/domain/mod.rs:7-9` | Module-level doc still claims "Each worker caches a tree-sitter `Parser` … per `Lang`, so neither the grammar nor the fact query is rebuilt per file"; the tier removed the `parsers` cache and `parse_facts` now calls `ariadne_parser::parse_file`, which constructs a fresh `TreeSitterParser` per file. The `ThreadState` doc (lines 384-392) was updated to match; the module doc was not. | Reword the module doc to state only the `FactExtractor` is cached per `Lang`; the host/injected parsers are built per file by `parse_file`. |
| F2 | maintainability | INFO | `crates/ariadne-cli/src/domain/mod.rs:477-505` | `parse_facts` re-implements the per-layer extend → sort-by-byte → `dedup` merge of `ariadne_parser::extract_syntactic_facts` (facts.rs:369-392) verbatim across all five `SyntacticFacts` vectors. A future fact category added to `SyntacticFacts` is silently dropped here unless both copies are updated in lockstep — a divergence the struct already demonstrated when `renders`/`hooks` were added. | Optionally expose a parser-crate merge entry point that accepts caller-owned `FactExtractor`s, or accept the duplication with a cross-reference comment (already partially present). |
</findings>

<verdict>
PASS — zero FAIL findings; two INFO nits that do not gate.

Exit criteria, each independently verified:
1. `lang_for_path` (`domain/mod.rs:76-91`): `.tsx`→`Lang::Tsx`, `.vue`→`Lang::Vue`,
   `.svelte`→`Lang::Svelte`, `.astro`→`Lang::Astro`; `.jsx` stays on the
   `js|jsx|mjs|cjs`→`Lang::JavaScript` arm. `.tsx` removed from the TypeScript
   arm. ✓
2. React/TSX real run reports `langs:["tsx"]` (not `typescript`) with
   `symbols:2`; `parse_failures:0` confirms the file is no longer routed to the
   non-TSX grammar. ✓
3. Vue/Svelte/Astro real runs each report the matching lang tag, non-zero
   symbols, and `edges:1` — the persisted edge is a `Renders` edge in every
   case (integration test filters `EdgeKind::Renders` from redb and asserts
   `>=1`). The committer synthesizes a file-spanning `Component` symbol per SFC
   so a template render site has a graph `src` and a cross-file `<Child/>`
   resolves to `Child`'s SFC. ✓
4. `enabled_langs` autodetect: file signal handled transitively by the
   `lang_for_path` change in `Config::detect`'s walk; `package.json` signal
   handled by `detect_package_json_langs` (react/solid-js→javascript+tsx,
   vue/svelte/astro→own tag, both `dependencies` and `devDependencies`).
   Unit test `package_json_deps_enable_framework_langs` proves the dep-only
   path; `no_package_json_leaves_framework_langs_off` proves it stays a signal,
   never a default. ✓
5. `cargo nextest -p ariadne-cli`, `clippy -D warnings`, `cargo test --test
   architecture` all green; also `fmt --check` and rustdoc clean. ✓

Correctness spot-checks: the typed edge resolver (`resolve_edges`) generalizes
the prior call-resolution closure cleanly — `EdgeKey.kind` keeps `References` /
`Renders` / `UsesHook` edges distinct in the `seen` dedup set; the unresolved-
`src`/`dst` and self-loop drops apply uniformly, matching the plan's stated
best-effort policy. `enclosing_symbol` correctly selects the synthesized
whole-file `Component` for template render sites (SFC `<script>` decls never
span the template region). No smuggled dependency — `serde_json`/`tempfile`
were already declared; no new architectural pattern; hexagonal boundary intact
(`ariadne-cli` is the composition root, ADR-0007).
</verdict>

<next_steps>
- Verdict gate is satisfied; tier-05 may be committed.
- F1/F2 are INFO — optional. F1 is a one-line doc correction worth folding into
  the commit so the module header does not mislead.
- Not a tier-05 finding, but flagged for tier-09: removing the per-worker
  `tree_sitter::Parser` cache (an inherent consequence of routing through
  `ariadne_parser::parse_file`, which builds parsers per call) shifts parser
  construction onto the hot path. Plan `<risks>` R-SLO defers the cold-index
  SLO gate to tier-09 — that re-run should confirm cold index still holds <60s
  on the framework corpus.
- The tier `<verification>` calls for a real run over an external OSS
  Vue/Svelte/Astro/React repo; this audit substituted real end-to-end binary
  runs over the in-repo fixture trees (no repo clone available in-session).
  All four indexed with `parse_failures:0`. An external-repo spot-check remains
  a reasonable manual confirmation before release.
</next_steps>

<sources>
- tree-sitter `QueryCursor` / injection contract: https://docs.rs/tree-sitter/0.26.8/tree_sitter/struct.QueryCursor.html ; https://tree-sitter.github.io/tree-sitter/3-syntax-highlighting.html
- Conventional Commits v1.0.0: https://www.conventionalcommits.org/en/v1.0.0/
- Google eng-practices — review standard: https://google.github.io/eng-practices/review/reviewer/standard.html
- Plan + tier under review: `.claude/plans/js-framework-support/plan.md`,
  `.claude/plans/js-framework-support/tier-05-cli-detection.md`
</sources>
