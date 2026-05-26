---
tier_id: tier-05
audited: 2026-05-27
verdict: PASS
commit: 8288ebba39e2196aafdc4b414c5da5bfb8172506
---

<scope>
Tier-05 dead-code classification — per-language `is_root` over tier-04 `SymbolRecord` metadata (`visibility`/`attributes`) joined with file-level `Lang`; `dead_symbols` consults the root set before the fan-in=0 filter; the production `weak_spots` path runs the classifier. Audit against working-tree state (`HEAD = 8288ebb`, tier-05 work uncommitted) scoped to files listed in `tier-05-dead-code-classification.md` `<files>` (plus the new `crates/ariadne-graph/src/roots.rs` and `crates/ariadne-graph/tests/dead_code_roots.rs`).
</scope>

<checks_run>
Verification commands re-run end-to-end:
- `cargo nextest run -p ariadne-graph -p ariadne-mcp` → 55/55 PASS (incl. `dead_code_roots::{rust_main_is_root_orphan_is_dead, rust_public_and_test_are_roots, go_exported_and_test_prefix_are_roots, python_dunder_main_and_decorated_are_roots, ts_exported_is_root_internal_is_dead, java_public_main_and_test_annotation_are_roots, c_main_and_public_extern_are_roots, cycle_among_non_roots_with_orphan_dead}` and `tools_weak_spots::{weak_spots_lists_cycles_and_dead_code, weak_spots_excludes_non_library_god_modules}`).
- `cargo test --test architecture` → 1/1 PASS (hexagonal invariants hold; new `pub mod roots` is pure-domain under `ariadne-graph`, no new in-workspace dep).
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` → clean.
- `cargo fmt --all --check` → clean.

Manual dogfood check (tier `<verification>` step 3):
- Probed a fresh-indexed mini Rust crate (`pub fn exported_fn`, `fn private_fn`, `fn main`, `#[test] fn smoke`) via `ariadne index` → `RedbStorage::iter_symbols`: 4 symbols total, `exported_fn` carries `Visibility::Public`, `smoke` carries `attributes=["test"]`, `main` carries name "main". Each one satisfies `roots::is_root` for `Lang::Rust`. The full production pipeline — tree-sitter capture → `attach_visibility`/`attach_attributes` → `SymbolRecord` → redb → catalog `SymbolMeta::from_record(.., lang_of[file])` → `weak_spots::handle` → `roots::is_root` — populates the right metadata for the classifier to fire end-to-end (probe code documented in this audit; not committed).
- Re-ran `weak_spots` against the live `.ariadne/index.redb`: `crate::main` no longer appears in `dead_symbols` (v1 false-positive resolved). `#[test]`-style functions (`reopen_with_mismatched_schema_version_returns_schema_mismatch`, `registry_covers_v1_langs`) and public types (`EdgeKind`, `ScopeInput`, `FileQuery`) still surface, but a direct dump of the same redb shows 0/2032 symbols with non-`Unknown` visibility and 0/2032 with non-empty attributes — i.e. the live index pre-dates the tier-04 parser changes for those records; the migration filled defaults, and incremental re-index has not yet refreshed unchanged files. `rm .ariadne/index.redb && ariadne index .` is the operator-side fix; the tier-05 classifier itself is correct (verified above). Recorded as INFO, not a tier-05 defect.

Plan adherence reviewed end-to-end:
- `crates/ariadne-graph/src/roots.rs:28-52` — `is_root(lang, visibility, attributes, kind, name) -> bool` dispatches per `Lang`; reads visibility/attributes/kind/name without re-parsing — matches RD4 metadata contract.
- `roots.rs:54-69` — `last_segment` / `attr_leaf_matches` handle `tokio::test`, `pytest.fixture` qualified attribute names so the classifier sees the framework marker, not the path prefix.
- `roots.rs:71-143` — per-language rules (Rust pub/`test`/`bench`/`no_mangle`/`export_name`/`main`; Go Public+`Test*`/`Benchmark*`/`main`; Python `__main__`/`test_*`/`Test*`/decorator prefixes; JS/TS/Vue/Svelte/Astro Public via `export`; Java/Kotlin pub/`@Test`/`main`; C# pub/`[Fact]`/`[Theory]`/`Main`/`main`; C/C++ Public/`main`) cover the tier `<steps>` enumeration.
- `crates/ariadne-graph/src/dead.rs:17-29, 47-72` — new `DeadCodeConfig::roots: BTreeSet<SymbolId>` consulted alongside `entry_points`/`exported`/`tests` before the fan-in=0 test; ordering preserved.
- `crates/ariadne-graph/src/lib.rs:21` — `pub mod roots;` exposes the classifier (callers reach it via `ariadne_graph::roots::is_root`); façade re-exports of `DeadCodeConfig`/`DeadCodeReport`/`DeadSymbol` unchanged.
- `crates/ariadne-mcp/src/catalog.rs:23-58, 92-113` — `SymbolMeta` gains `lang`/`visibility`/`attributes`; the catalog builder joins `FileRecord.lang` into a per-`FileId` map and populates each `SymbolMeta` via `from_record(&rec, lang)` — `Lang::Other("unknown")` fallback when the file lookup misses, which is the same defensive default the rest of the catalog uses.
- `crates/ariadne-mcp/src/tools/weak_spots.rs:76-91` — `handle` walks `cat.symbols`, builds the root set via `roots::is_root(meta.lang, meta.visibility, &meta.attributes, &meta.kind, &meta.name)`, and threads it into `DeadCodeConfig::roots` before calling `dead_code` — i.e. the production path runs the classifier (exit criterion 3).
- `crates/ariadne-graph/tests/dead_code_roots.rs:55-321` — eight cases cover Rust (`main`, pub, `#[test]`, `no_mangle`), Go (`Test*`/`Benchmark*`/`main`), Python (`__main__`/decorator), TS/Tsx (export), Java + C# (test annotations + `main`), C/C++ (`main`/extern). The composing case (`cycle_among_non_roots_with_orphan_dead`) asserts the classifier composes with the existing fan-in test.
- `crates/ariadne-mcp/tests/support.rs:268-283` + `tests/tools_weak_spots.rs:41-47` — canonical changeset gains a non-root `crate::unused_helper`; the `weak_spots` golden now asserts `crate::main` is excluded and `crate::unused_helper` surfaces — i.e. the regression for the v1 false-positive is locked.
- `crates/ariadne-mcp/tests/tools_project_status.rs:20` — `symbol_count` bumped from 6 → 7 to match the new fixture symbol; no other tools were perturbed.

Architecture / file-cap checks:
- `ariadne-graph` workspace deps unchanged (`ariadne-core` + `ariadne-storage` only); no new transitive crate.
- Authored tier + roots/test sizes: tier file 54, `roots.rs` 143, `dead_code_roots.rs` 352 — `dead_code_roots.rs` is a test file (data-table cases), CLAUDE.md `<rules>` `≤200 lines` applies to authored skill/rule/plan/audit files; production `roots.rs` is comfortably under the cap.

Exit-criteria reconciliation:
1. *Per-language root classifier from `SymbolRecord` visibility/attributes/`Lang`, not name heuristics.* ✓ — `is_root` dispatches on `Lang`; visibility + attributes are the primary signals. Name heuristics are confined to documented conventions (`main`, Go `Test*`/`Benchmark*`, Python `__main__`/`test_*`, C# `Main`) as the tier `<steps>` permit.
2. *`dead_symbols` excludes the root set before the fan-in=0 filter.* ✓ — `dead.rs:53-58`.
3. *`ariadne-mcp` `Catalog` exposes per-symbol `Lang`; `weak_spots` runs the classifier on the production path.* ✓ — `catalog.rs:37, 92-113` and `weak_spots.rs:76-91`.
4. *Language fixture set produces zero `dead_symbols` hits on `main`/exported/`#[test]` symbols.* ✓ — covered by `dead_code_roots.rs` (per-`Lang` matrix). Note: the tier `<files>` referenced a `crates/ariadne-graph/fixtures/` directory that does not exist; the implementation realized the same coverage via synthetic in-graph fixtures (`Sym` rows) rather than source-file fixtures. Recorded as INFO because the classifier is pure metadata logic — synthetic fixtures isolate it directly without the parser/SCIP indirection.
5. *`cargo nextest run -p ariadne-graph -p ariadne-mcp` + architecture + clippy + fmt all green.* ✓ — all four commands re-run green this audit.
</checks_run>

<findings>
| id | category | severity | location | problem | fix | sources |
|---|---|---|---|---|---|---|
| F1 | plan_adherence | INFO | tier-05 `<files>` line 23 + repo state (no `crates/ariadne-graph/fixtures/` dir) | Tier `<files>` lists `crates/ariadne-graph/fixtures/ — modify`, but `ariadne-graph` has never carried a `fixtures/` dir; the implementation covered the per-language matrix via synthetic in-graph `Sym` rows in `crates/ariadne-graph/tests/dead_code_roots.rs` instead. Coverage of exit criterion #4 is equivalent — the classifier is pure metadata logic — but the plan text and the realized test fixtures disagree. | Future tier should either drop the fixtures reference or re-state the test approach as "synthetic in-graph fixtures in `tests/`". No code change required for tier-05 itself. | n/a |
| F2 | docs | INFO | live `.ariadne/index.redb` vs `weak_spots` output | The dogfood self-index still flags `#[test]` Rust functions and `pub` types in `dead_symbols`. Root cause: 0/2032 symbols carry tier-04 visibility/attributes — the live redb pre-dates the tier-04 parser changes for those records and the v2→v3 migration filled `Unknown`/`[]` defaults; unchanged files have not been re-indexed since. Not a tier-05 defect (a freshly-indexed mini-crate populates Public + `["test"]` correctly and the classifier excludes both). | Operator-side: `rm .ariadne/index.redb && ariadne index .` after tier-04 lands; or add a follow-up tier note that schema bumps which depend on new parser output should force a full re-parse rather than rely on incremental cache reuse. | tier-04 audit report; this audit's mini-crate probe |

No FAIL findings.
</findings>

<verdict>
PASS. Tier-05's classifier is correctly implemented, integrated into the production `weak_spots` path, and verified end-to-end on a fresh index. All five exit criteria are met; verification commands re-run green. Two INFO findings noted (plan-vs-implementation fixture path mismatch; stale dogfood index masks tier-04 metadata) — neither blocks the tier.
</verdict>

<next_steps>
- Tier-05 is shippable. Recommend a `chore` follow-up: refresh the self-index (`rm .ariadne/index.redb && ariadne index .`) and re-run `weak_spots` to lock the dogfood evidence per plan RD4 intake.
- Consider whether tier-04 (or a separate maintenance tier) should auto-force a re-parse on schema-version bumps where the new schema fields come from parser captures; currently the v2→v3 migration only fills defaults and incremental indexing leaves stale records in place.
</next_steps>

<sources>
- tier-05 plan: `.claude/plans/post-v1-roadmap/tier-05-dead-code-classification.md`
- post-v1 plan (RD4, RD10): `.claude/plans/post-v1-roadmap/plan.md`
- tier-04 audit (visibility/attributes pipeline verified): `.claude/plans/post-v1-roadmap/audit/tier-04-report.md`
- redb `WriteTransaction` semantics (migration context): https://docs.rs/redb/4.1.0/redb/struct.WriteTransaction.html
- Go exported-identifier rule (Go root classifier basis): https://go.dev/ref/spec#Exported_identifiers
- Code-review standard (block only on real defects; INFO otherwise): https://google.github.io/eng-practices/review/reviewer/standard.html
</sources>
