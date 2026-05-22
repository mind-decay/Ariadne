---
tier_id: tier-04
title: Dead-code classification — per-language entry-point roots exclude false positives
deps: []
exit_criteria:
  - A per-language root classifier marks entry points (`main`, exported API, test fns, framework entrypoints).
  - `dead_symbols` excludes the root set before the fan-in=0 filter.
  - The 7-language fixture set produces zero `dead_symbols` hits on `main`/exported/`#[test]` symbols.
  - `cargo nextest run -p ariadne-graph` + architecture + clippy + fmt all green.
status: pending
---

<context>
v1 `dead_symbols` flags every symbol with graph fan-in=0. That false-positives on roots: a `weak_spots` self-run flags `main` in the Go fixture, `#[test]` functions, and fixture-local variables. v1 tier-14 explicitly left "per-language target classification" as future work. This tier adds a root classifier so genuine reachability roots are excluded before the fan-in test (plan RD4). Full context: plan.md.
</context>

<files>
- crates/ariadne-graph/src/dead_code.rs — modify: consult a root set before emitting a fan-in=0 candidate.
- crates/ariadne-graph/src/roots.rs — new: per-`Lang` entry-point classifier over symbol kind/name/visibility/attributes.
- crates/ariadne-graph/tests/ — modify: extend the dead-code golden with the root cases per language.
- crates/ariadne-graph/fixtures/ — modify/ensure each of the 7 languages has a fixture with a root symbol + a genuinely dead symbol.
</files>

<steps>
1. Failing test first (`ariadne-graph` tests): assert `dead_symbols` over the 7-language fixtures returns the genuinely dead symbol but NOT `main`/exported/`#[test]` symbols. Red — the classifier does not exist.
2. Read `dead_code.rs` + the `DeadCodeConfig` introduced in v1 tier-07 to locate the fan-in=0 filter and what symbol metadata is available (kind, visibility, attributes) [src: .claude/plans/ariadne-core/tier-07-graph-analytics.md].
3. Define `roots.rs`: `fn is_root(sym, lang) -> bool` covering — Rust: `fn main`, `pub` items in a lib crate root, `#[test]`/`#[bench]`, `#[no_mangle]`/`#[export_name]`; Go: `func main` in `package main`, `func Test*`/`Benchmark*`; Python: `if __name__ == "__main__"` targets, `__main__.py`; JS/TS: exported symbols, framework entrypoints (default export of a route/page/component); Java/C#: `public static main`, test-annotated methods; C/C++: `main`, non-`static` extern functions.
4. Each rule cites observable symbol metadata only — no heuristic name matching beyond documented language conventions. Where metadata is insufficient, record the gap in the tier audit rather than guessing.
5. `dead_code.rs`: skip any symbol where `is_root` is true before applying fan-in=0.
6. Extend the dead-code golden; keep `dead_symbols` false positives on *unreferenced non-root* symbols intact (still correct behaviour per v1 tier-14 scope).
</steps>

<verification>
- `cargo nextest run -p ariadne-graph` — dead-code golden green across all 7 languages.
- Manual: run `weak_spots` on the ariadne_v2 self-index; confirm `main`/test functions no longer appear in `dead_symbols` (compare against the pre-tier `weak_spots` output recorded in plan intake).
- `cargo test --test architecture`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check` — green.
</verification>

<rollback>
`git checkout -- crates/ariadne-graph`. The classifier is additive; reverting restores the pure fan-in=0 filter.
</rollback>
