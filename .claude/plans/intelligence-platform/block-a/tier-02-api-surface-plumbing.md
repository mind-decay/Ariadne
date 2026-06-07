---
tier_id: tier-02
title: A2 plumbing — PublicSymbol type, git base-blob read, parser public-surface extractor
deps: []
exit_criteria:
  - "`cargo nextest run --workspace` green; new failing-first tests now pass"
  - "git unit test: `read_blobs_at(rev, paths)` returns the exact base-blob bytes for a seeded temp repo, and an absent path is skipped (not an error)"
  - "parser unit test: `public_surface(lang, bytes)` returns exactly the public symbols (name, kind, visibility, declaration-header text) for a snippet in each of the 15 fixture languages, excluding private/restricted decls"
  - "no `gix`/`tree-sitter` type appears in any public signature; `cargo test --test architecture` green (core dep-free; git/parser still core-only)"
  - "`cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo fmt --all --check` green"
status: completed
completed: 2026-06-07
---

<context>
Delivers the reusable, surfacing-free plumbing A2 needs (plan D3/D4/D6): a shared `PublicSymbol` domain type in `ariadne-core`, a git base-blob reader, and a parser public-surface extractor. The classifier and the MCP/CLI surfaces are tier-03. Both base and head surfaces are produced by the SAME tree-sitter path here, so tier-03 compares like-for-like (no SCIP-mix phantom diffs). Full context: ./plan.md.
</context>

<files>
- `crates/ariadne-core/src/domain/types/public_symbol.rs` (new) + `types/mod.rs` + core façade re-export — `pub struct PublicSymbol { name: String, kind: String, visibility: Visibility, signature: String }` (`signature` = normalized declaration-header text). Owned, `serde`-derived to match sibling records [src: crates/ariadne-core/src/domain/records.rs:37-59].
- `crates/ariadne-git/src/adapters/gix/blobs.rs` (new) + `adapters/gix/mod.rs` + `lib.rs` re-export — `read_blobs_at(repo_root, rev, &[String]) -> Result<Vec<(String, Vec<u8>)>, GitError>`.
- `crates/ariadne-parser/src/adapters/treesitter/surface.rs` (new) + `lib.rs` re-export — `public_surface(lang: Lang, bytes: &[u8]) -> Result<Vec<PublicSymbol>, ParserError>`.
- Tests: inline `#[cfg(test)]` in each new module (git mirrors the existing temp-repo test style; parser uses fixture snippets per language).
</files>

<steps>
1. Write failing tests first (TDD): a git test seeding a 2-commit temp repo asserts `read_blobs_at("HEAD~1", [path])` returns the old bytes and a non-existent path is omitted; a parser test asserts `public_surface` on a Rust snippet returns the `pub fn` with its signature header and omits a private `fn` [src: CLAUDE.md TDD rule].
2. Add `PublicSymbol` to `ariadne-core` (zero new deps — `serde` only, already used) and re-export from the façade; keep core hermetic [src: tests/architecture.rs:85-92].
3. Implement `read_blobs_at`: `gix::open` → `rev_parse_single(rev)` → `find_commit` → `commit.tree()` → per path `tree.lookup_entry_by_path` → `blob_bytes(repo, object_id)`; collect `(path, bytes)` sorted by path, skipping absent paths; map errors to `GitError`. No `gix` type crosses the signature — exactly the existing `head_blob_bytes` idiom generalized to an arbitrary rev [src: crates/ariadne-git/src/adapters/gix/diff.rs:72-78,148-160; https://docs.rs/gix/0.84.0].
4. Implement `public_surface`: `parse_file(lang, bytes)` → `extract_syntactic_facts` → keep `Decl`s with `visibility == Visibility::Public` → build `PublicSymbol` per decl: `kind` from `DeclKind`, `signature` = bytes `def_byte_range.start ..` up to the language's body-open delimiter, whitespace-normalized [src: crates/ariadne-parser/src/lib.rs:11-18; facts.rs:90-110].
5. Define the per-`Lang` body-open delimiter table inline with a one-line `[src: …]` per family: brace languages (Rust, Go, Java, Kotlin, C#, C, C++, JS, TS, Svelte/Vue/Astro/Solid/React script blocks) cut at the first `{`; Python cuts at the trailing `:`. A decl with no delimiter (e.g. a bare `const`) takes its whole `def_byte_range`. Document the heuristic and its limits (BR1).
6. Sort each `public_surface` result by `(name, kind)` for determinism.
</steps>

<verification>
- `cargo nextest run --workspace` — green, including the new tests (red before steps 3/4).
- Git: `read_blobs_at` returns byte-exact base content for a seeded commit; absent path skipped, not errored.
- Parser: per-language snippet yields exactly the public symbols + correct header text; private/restricted decls excluded.
- `cargo test --test architecture` green — `ariadne-core` stays dep-free, `ariadne-git`/`ariadne-parser` stay core-only, no `gix`/`tree-sitter` type leaks (grep the new public signatures).
- `cargo clippy ... -D warnings`, `cargo fmt --all --check` green.
Fail loudly: any leaked adapter type, wrong header slice, or non-deterministic order is a hard fail — root-cause, never weaken the assert [src: CLAUDE.md `<rules>`].
</verification>

<rollback>
Single-commit tier, purely additive (one new type + two new functions + their re-exports). Revert the commit or `git restore` the listed files; no migration, no schema change to undo.
</rollback>
