---
tier_id: tier-06
audited: 2026-06-04
verdict: PASS
commit: 710f3c80cd069940babab7c42eeb3ff894ff4b76
---

<scope>
Tier-06 — CLI `doc` command: write `docs/codebase-overview.md` + sidecar
`.svg`. Scoped to the tier `<files>`:
- `crates/ariadne-cli/src/commands/doc.rs` (NEW)
- `crates/ariadne-cli/src/commands/mod.rs` (`pub mod doc;`)
- `crates/ariadne-cli/src/main.rs` (`Cmd::Doc` variant + arm)
- `crates/ariadne-cli/tests/doc_command.rs` (NEW)
- `docs/codebase-overview.md` + `docs/codebase-overview.svg` (regenerated)
- `crates/ariadne-cli/src/config.rs` — OPTIONAL, not touched (defaults inlined
  as clap `default_value` in `main.rs`; acceptable per the OPTIONAL marker).

The working tree also carries the uncommitted tier-05 diff (graph/mcp/daemon/
core renderers). Those files are out of scope here and belong to the tier-05
audit (`audit/tier-05-report.md`); they were excluded from this review.
Index fresh at revision 1561 (`project_status`); graph trusted.
</scope>

<checks_run>
- Read every in-scope file end-to-end: `doc.rs`, `doc_command.rs`, the
  `mod.rs`/`main.rs` diff, `docs/codebase-overview.md`, `docs/codebase-overview.svg`.
- Signature parity (Ariadne `read_symbol`): `for_project(graph, snap, modules,
  churn, co_change, scope)`, `architecture_svg(graph, modules, scope)`,
  `build_modules(&Catalog, Option<&str>)`, `DocScope { extra_excludes }`,
  `Catalog { churn, co_change, graph, .. }` — all match the call sites in
  `doc.rs:49-63`. Cold-path build (`index_path` → `RedbStorage::open` →
  `Catalog::build` → `storage.snapshot()`) mirrors `query.rs:66-74` exactly (D4).
- `cargo nextest run -p ariadne-cli` → 44/44 pass, incl.
  `doc_command::doc_writes_pair_is_deterministic_and_honours_exclude`.
- Determinism (real run, twice to same paths): `.md` and `.svg` byte-identical.
- Freshness (real run vs committed): regenerated `.svg` byte-identical to the
  committed `docs/codebase-overview.svg`; regenerated `.md` identical modulo the
  image-link basename (expected — the `--svg` rewrite). Committed artefact is in
  sync with current code.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` → clean.
- `cargo fmt --all --check` → clean.
- `cargo deny check` → advisories/bans/licenses/sources ok (proves no new dep, D5).
- `cargo test --test architecture` → `architecture_invariants_hold` ok (no
  hexagon inversion introduced; cli-as-composition-root pattern unchanged).
- SVG validity: well-formed XML (`xml.dom.minidom`), `viewBox` present, 13
  `<rect>`/13 `<text>` (= 13 crate nodes from the Architecture table), one
  `arrow` marker def. `jquery` absent (0 hits) in both `.md` and `.svg`.
- Largest SCC named: Cycle clusters lists the 22-member cluster
  (`forget_file`, `symbols_for_file`, …) first, descending.
- No collateral: `git status` for `docs/`+`ariadne-cli/` shows exactly the
  tier-06 set; my real runs wrote to `/tmp`, leaving the repo untouched.
</checks_run>

<findings>
| id | category | severity | location | problem | fix |
| --- | --- | --- | --- | --- | --- |
| F1 | docs | INFO | tier-06 exit_criteria L7 / step-1 vs `crates/ariadne-graph/src/doc_model.rs:64-70` | Exit criterion §2 labels the flag `--exclude <glob>` and step-1 suggests `--exclude '**/fixtures/**'`, but `DocScope::include` matches by `path.contains` (substring), so a literal glob would silently match nothing. | Non-blocking: the CLI help (`main.rs`) and `doc.rs` correctly call them "substring excludes" and the shipped test uses a substring (`alpha`); only the plan wording is loose. Reconcile the plan text to "substring" in a future plan edit. |
</findings>

<verdict>
PASS — zero FAIL findings; one INFO (non-gating).

All four exit criteria independently verified:
1. `ariadne doc` writes both files at configurable paths; re-run byte-identical
   (test + real twin-run). ✓
2. `--exclude` populates `DocScope.extra_excludes` and is honoured — the test
   proves `--exclude alpha` drops the alpha crate row while keeping beta. The
   substring semantics are owned by tier-01's committed `DocScope`; tier-06's
   job (wire the flag → populate → honour) is met. The step-1 fixture-glob
   example was incoherent with the Source-only default (fixtures already
   excluded by `classify`); the implementer's substring test is the correct
   adaptation, not a regression. ✓
3. Regenerated `.svg` is well-formed standard SVG (viewBox + rects + text +
   arrow marker, 13 crate nodes); `jquery.js` absent; largest SCC (22 members)
   named. Structural validity stands in for the in-IDE visual render the
   implementer attests to in step 5. ✓
4. `doc_command` green; clippy/fmt/deny/architecture all green. ✓
</verdict>

<next_steps>
None required for tier-06. The INFO is a plan-wording reconciliation, not a code
change. Note for the slug owner: the working tree still mixes the uncommitted
tier-05 diff with tier-06; stage/commit per tier so the audit-gate maps each
commit to its verdict.
</next_steps>

<sources>
- repo: crates/ariadne-cli/src/commands/doc.rs; tests/doc_command.rs; main.rs;
  crates/ariadne-graph/src/doc_model.rs:64-70 (`include`); docgen.rs:277-345
  (`for_project`/`architecture_svg`); crates/ariadne-mcp/src/catalog.rs:71-97;
  crates/ariadne-cli/src/commands/query.rs:66-74 (cold path).
- [MDN — SVG Element reference](https://developer.mozilla.org/en-US/docs/Web/SVG/Element) — confirms `svg`/`viewBox`/`rect`/`text`/`marker` are standard renderable elements.
- [Google eng-practices — reviewer standard](https://google.github.io/eng-practices/review/reviewer/standard.html) — ship-if-it-improves-health bar applied to F1.
</sources>
