---
tier_id: tier-02
audited: 2026-06-07
verdict: PASS
commit: ad59c2f6f3ec78c92ad3a402d00e62c46979b239
---

<scope>
A2 plumbing tier — shared `PublicSymbol` domain type, git base-blob reader,
and parser public-surface extractor. Audited the working tree (tier-02 is
built but uncommitted; HEAD is the prior SCIP commit). Scoped `<files>`:
- `crates/ariadne-core/src/domain/types/public_symbol.rs` (new) + `types/mod.rs` + core `lib.rs` façade re-export.
- `crates/ariadne-git/src/adapters/gix/blobs.rs` (new) + `gix/mod.rs` + git `lib.rs` re-export.
- `crates/ariadne-parser/src/adapters/treesitter/surface.rs` (new) + `treesitter/mod.rs` + parser `lib.rs` re-export.

The working tree also carries tier-01 (test-impact) changes — including the
`AffectedTestsReport` re-export co-located in `ariadne-core/src/lib.rs` — and
no tier-03 files. Tier-01's diff is out of this audit's scope (separate
PASS report already on file) and does not bear on tier-02's correctness; the
full workspace builds and tests green with both tiers present.
</scope>

<checks_run>
All `<verification>` commands re-run on the working tree:
- `cargo fmt --all --check` — clean (no output, exit 0).
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` — clean, exit 0.
- `cargo test --test architecture` — `architecture_invariants_hold ... ok` (1 passed). Confirms `ariadne-core` stays dep-free and `ariadne-git`/`ariadne-parser` stay core-only; tier-02 adds no new cross-crate edge (the `mcp → parser` edge is tier-03, correctly absent here).
- `cargo nextest run --workspace` — 487 passed, 19 skipped, 0 failed (1 slow: the tier-11 `warm_apply_equals_fresh_rebuild` daemon perf test at 95s, unrelated to tier-02, passed).
- Targeted: `cargo nextest run -p ariadne-git blobs` — 2/2 pass (`reads_exact_base_blob_at_prior_revision`, `skips_absent_path_without_error`).
- Targeted: `cargo nextest run -p ariadne-parser surface` — 4/4 pass.

Evidence pass:
- Read every changed file end-to-end and every symbol the new code depends on: `blob_bytes` (returns owned `Vec<u8>`, no gix leak), the free `parse_file(host_lang, &ParserRegistry, &[u8], Option<&ParsedFile>, &[InputEdit])` (matches the 5-arg call site), `extract_syntactic_facts`, `Decl` (field `def_byte_range: (u32,u32)`, `visibility`, `name`, `kind`), `DeclKind` (all 14 variants map in `kind_tag`, `Other(s)` handled), `GitError::{Open,Revspec,Diff}` (all exist), `Visibility` (derives `Copy` — the `decl.visibility` field-copy out of `&Decl` is sound).
- No-leak check: `read_blobs_at` signature is `(&Path, &str, &[String]) -> Result<Vec<(String, Vec<u8>)>, GitError>`; `public_surface` is `(Lang, &[u8]) -> Result<Vec<PublicSymbol>, ParserError>`. Neither exposes a `gix`/`tree_sitter` type — exit criterion 4 met by inspection and by the green architecture test [src: docs/folder-layout.md rule 4].
- 15-fixture coverage: `per_language_public_surface_excludes_non_public` covers rust, go, java, kotlin, csharp, c, cpp, javascript, typescript, react(`Tsx`), solid(`Tsx`), python, vue, svelte, astro — the 15 fixture languages, each asserting public-only output with exact `(name, kind, signature)` tuples and `Visibility::Public` per row [src: tier `exit_criteria`; surface.rs:181-283].
- Determinism: both functions sort their output (`read_blobs_at` by path; `public_surface` by `(name, kind)`), no clock/RNG — matches the block-a determinism constraint [src: plan.md `<constraints>`].
- C/Python/Cpp-member empty-or-partial surfaces are the documented, plan-consistent consequence of `Unknown`-exclusion (step 4: keep `visibility == Public`); the `Unknown`-as-public policy is explicitly deferred to tier-05 and tracked by BR2 [src: surface.rs:22-27; plan.md D3, BR2].
- `gix` `read_blobs_at` reuses the repo's `rev_parse_single → find_commit → tree → lookup_entry_by_path → blob_bytes` idiom; error mapping (open→Open, rev/commit→Revspec, tree/lookup→Diff) is internally consistent [src: gix 0.84.0; crates/ariadne-git/src/adapters/gix/diff.rs idiom].

Observation (non-gating): nextest flagged the first inline parser unit test `LEAK` while still reporting it `passed`. Confirmed pre-existing grammar-load global-state artifact, not a tier-02 logic defect — sibling `facts_*` integration tests do not leak, and the leak attaches to whichever parser unit test loads grammars first. No resource owned by the new code is leaked.
</checks_run>

<findings>
| id | category | severity | location | problem | fix |
|----|----------|----------|----------|---------|-----|
| — | — | — | — | No defects found. | — |
</findings>

<verdict>
PASS. All five `<verification>` gates re-run green; all five `exit_criteria`
independently verified. The implementation matches `<steps>` 1–6 and decisions
D3/D4/D6: `PublicSymbol` lives dep-free in `ariadne-core`; both blob reader and
surface extractor return owned types with no `gix`/`tree-sitter` leak; the
surface is Public-only, sorted, and symmetric (same tree-sitter path for any
side). Hexagonal invariants hold (architecture test green; no new cross-adapter
edge). Tests are realistic, behavior-asserting, and fail loud. Zero FAIL, zero
INFO.
</verdict>

<next_steps>
None for tier-02. The tier is additive and ships as-is. Tier-03 will add the
`mcp → parser` edge (its own ADR + architecture-test update) and must measure
the BR3 re-parse cost on a multi-file diff; the `Unknown`-as-public surfacing
gap (C/Python) is owned by tier-05, not a tier-02 regression.
</next_steps>

<sources>
- Tier file: .claude/plans/intelligence-platform/block-a/tier-02-api-surface-plumbing.md
- Sibling plan: .claude/plans/intelligence-platform/block-a/plan.md (D3/D4/D6, BR1/BR2, `<constraints>`)
- Repo: crates/ariadne-core/src/domain/types/{public_symbol.rs,mod.rs,visibility.rs}; crates/ariadne-git/src/adapters/gix/{blobs.rs,mod.rs,line_hunks.rs}; crates/ariadne-parser/src/adapters/treesitter/{surface.rs,facts.rs,incremental.rs,registry.rs,mod.rs}
- gix: https://docs.rs/gix/0.84.0
- Folder-layout rule 4 (no adapter type leaks): docs/folder-layout.md
- Review standard: https://google.github.io/eng-practices/review/reviewer/standard.html
</sources>
