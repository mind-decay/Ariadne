---
tier_id: tier-05
title: Dead-code classification — per-language entry-point roots exclude false positives
deps: [tier-04]
exit_criteria:
  - A per-language root classifier marks entry points from `SymbolRecord` visibility + attributes + `Lang`, not name heuristics.
  - "`dead_symbols` excludes the root set before the graph fan-in=0 filter."
  - The ariadne-mcp `Catalog` exposes per-symbol `Lang`; `weak_spots` runs the classifier on the production path.
  - The language fixture set produces zero `dead_symbols` hits on `main`/exported/`#[test]` symbols.
  - "`cargo nextest run -p ariadne-graph -p ariadne-mcp` + architecture + clippy + fmt all green."
status: completed
completed: 2026-05-27
---

<context>
v1 `dead_symbols` flags every symbol with graph fan-in=0; it false-positives on roots — a `weak_spots` self-run flags `main` in the Go fixture, `#[test]` functions, and fixture-local variables. v1 tier-14 left "per-language target classification" as future work. tier-04 now gives every `SymbolRecord` a `Visibility` + `attributes`; per-symbol `Lang` is file-level, joined via the MCP `Catalog`. This tier adds the root classifier on that metadata (plan RD4). Full context: plan.md.
</context>

<files>
- crates/ariadne-graph/src/roots.rs — new: per-`Lang` entry-point classifier over visibility + attributes + kind + name.
- crates/ariadne-graph/src/dead.rs — modify: consult the root set before emitting a fan-in=0 candidate.
- crates/ariadne-graph/tests/ — modify: extend the dead-code golden with root cases per language via synthetic in-graph fixtures (`Sym` rows over `Visibility`/`attributes`/`Lang`), isolating the classifier from parser/SCIP indirection.
- crates/ariadne-mcp/src/catalog.rs — modify: `SymbolMeta` carries `lang`/`visibility`/`attributes`; the builder joins `defining_file` -> `FileRecord.lang`.
- crates/ariadne-mcp/src/tools/weak_spots.rs — modify: build the root set from catalog metadata so `weak_spots` runs the classifier.
</files>

<steps>
1. Failing test first (`ariadne-graph` tests): assert `dead_symbols` over the language fixtures returns the genuinely dead symbol but NOT `main`/exported/`#[test]` symbols. Red — `roots.rs` does not exist.
2. Read `dead.rs` to locate the fan-in=0 filter and the `DeadCodeConfig` `entry_points`/`exported`/`tests` sets [src: crates/ariadne-graph/src/dead.rs:14-67; .claude/plans/ariadne-core/tier-07-graph-analytics.md].
3. Define `roots.rs` — `is_root(visibility, attributes, lang, kind, name) -> bool` reading tier-04 metadata:
   - Rust: `Visibility::Public` items; `attributes` contains `test`/`bench`/`no_mangle`/`export_name`; `fn main`.
   - Go: `Visibility::Public` (uppercase-exported) package-level funcs; `Test*`/`Benchmark*` in `_test.go`; `func main` [src: https://go.dev/ref/spec#Exported_identifiers].
   - Python: `attributes` contains an entrypoint decorator; module-level `__main__` targets.
   - JS/TS/TSX: `Visibility::Public` (exported) symbols; framework default-export entrypoints.
   - Java/C#: `Visibility::Public static main`; `attributes` contains a test annotation (`Test`/`Fact`/`Theory`).
   - C/C++: `main`; `Visibility::Public` (non-`static` extern) functions.
4. Each rule reads `SymbolRecord` visibility/attributes; name matching is used only where a language's sole signal is a documented convention (`main`, Go `Test*`). Record any residual metadata gap in the tier audit, not as a guess.
5. `dead.rs`: skip a symbol when `is_root` is true, before the fan-in=0 test; non-root unreferenced symbols still flag (correct per v1 tier-14 scope).
6. `ariadne-mcp` `catalog.rs`: `SymbolMeta` gains `lang`/`visibility`/`attributes`; the catalog builder joins `defining_file` -> `FileRecord.lang` so every `SymbolMeta` exposes `Lang` [src: crates/ariadne-mcp/src/catalog.rs:23-67].
7. `weak_spots.rs`: build the root set from catalog metadata and pass it into `dead_code`, so the production `weak_spots` path excludes roots [src: crates/ariadne-mcp/src/tools/weak_spots.rs:73-86].
8. Extend the dead-code golden with the per-language root + dead cases.
</steps>

<verification>
- `cargo nextest run -p ariadne-graph` — the dead-code golden is green across every language fixture.
- `cargo nextest run -p ariadne-mcp` — the `weak_spots` golden is green; the classifier runs on the production path.
- Manual: run `weak_spots` on the ariadne_v2 self-index; confirm `main`/test functions no longer appear in `dead_symbols`, comparing against the pre-tier `weak_spots` output (plan RD4 intake evidence).
- `cargo test --test architecture`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check` — green.
</verification>

<rollback>
`git checkout -- crates/ariadne-graph crates/ariadne-mcp`. The classifier is additive; reverting restores the pure fan-in=0 filter.
</rollback>
