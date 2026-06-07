---
tier_id: tier-01
audited: 2026-06-07
verdict: PASS
commit: 0af641eb20fe515e34782d60fa539ff1169b7c58
---

<scope>
Tier-01 — pure code-skeleton (outline) assembler in `ariadne-graph`. Scoped diff
(tier `<files>`):
- `crates/ariadne-graph/src/outline.rs` — new use case (types + `assemble` + pure
  lexical helpers). 473 lines, all new.
- `crates/ariadne-graph/src/lib.rs` — façade re-export of the outline public API.
- `crates/ariadne-graph/tests/outline.rs` — golden + structured tests.
- `crates/ariadne-graph/tests/fixtures/outline/{sample.rs,sample.ts,sample.js}`.
- `crates/ariadne-graph/tests/snapshots/outline__{rust,typescript,javascript}_skeleton.snap`.

Reviewed against `plan.md` D1–D5 and the four tier `exit_criteria`. The working
tree also carries unrelated changes from the `intelligence-platform` fitness plan
(`src/fitness.rs`, `commands/fitness.rs`, mcp/cli files); those are out of scope
and excluded from this verdict.
</scope>

<checks_run>
- `cargo nextest run -p ariadne-graph` → 97/97 PASS, incl. 8 outline tests
  (rust/ts/js goldens, private-include, determinism, gap-collapse, max_symbols
  cap, multiline-signature probe).
- `cargo test --test architecture` → PASS (1 test). `ariadne-graph` gains no
  in-workspace edge; `outline.rs` imports only `ariadne_core::{Lang,Visibility}`;
  `crates/ariadne-graph/Cargo.toml` unmodified → no new dependency (EC4, D-hexagonal).
- `cargo clippy -p ariadne-graph --all-targets --all-features -- -D warnings` →
  clean.
- `cargo fmt --all --check` → no diff in `outline.rs` / `lib.rs`.
- `RUSTDOCFLAGS=-D warnings cargo doc -p ariadne-graph --no-deps` → RED, but only
  on 3 pre-existing broken intra-doc links in unmodified files (see INFO-1);
  `outline.rs` adds zero doc errors.
- Read every changed file end-to-end. Hand-traced line-accounting and the
  multi-line `signature_end` probe against the rust + wide-signature fixtures.
- Purity verified: no `fs`/`io`/`process`/`Command` in `outline.rs`; `assemble`
  is a total function over `&OutlineRequest`.

Exit criteria:
- EC1 (pure `outline::assemble -> Outline`, no IO/model, re-exported from façade)
  — PASS. `lib.rs` adds `pub use outline::{Outline, OutlineEntry, OutlineOptions,
  OutlineRequest, OutlineSymbol, assemble};` (re-exports only).
- EC2 (golden snapshots rust/ts/js: fold marker w/ exact elided count;
  signatures + leading doc comments byte-faithful; nesting by span containment;
  `include_private=false` drops non-public) — PASS. Snapshots show
  `… N lines` markers, captured `///` / `/** */` / `//` docs, `impl Counter {`
  with methods nested + indented; `helper`/`secret`/`Hidden` dropped.
- EC3 (skeleton bytes < source bytes for multi-symbol files; `elided+kept`
  accounts for every line) — PASS. `assert_invariants` enforces both across all
  fixtures + the focused cases; hand-trace of the rust fixture: kept 25 + elided
  11 = 36 = total.
- EC4 (clippy `-D warnings`, fmt, architecture, nextest green) — PASS (all four).
</checks_run>

<findings>
| id | category | severity | location | problem | fix |
|----|----------|----------|----------|---------|-----|
| INFO-1 | docs | INFO | crates/ariadne-graph/src/{doc_model.rs:166, docgen.rs:220, hotspot.rs:130} | `cargo doc -p ariadne-graph` (a tier `<verification>` command) is red from 3 broken intra-doc links in files **unmodified by this tier** (present at HEAD; e.g. `[src: plan.md tier-05 …]` parsed as a link, `[`rank`]`/`[`SymbolTable`]` linking private items). `outline.rs` itself is doc-clean. | Out of tier-01 scope; raise a separate cleanup tier — escape the `[src: …]` brackets and add `#[allow(rustdoc::private_intra_doc_links)]` or `--document-private-items`. Does not gate tier-01. |
| INFO-2 | plan_adherence | INFO | crates/ariadne-graph/src/lib.rs:24,47 | The lib.rs diff also adds `mod fitness;` + `pub use fitness::{…}` — a different plan's (`intelligence-platform` fitness) work interleaved in a tier-01 file. | Not a defect; flagged so the commit for tier-01 separates the outline re-exports from the fitness re-exports. |
| INFO-3 | correctness | INFO | crates/ariadne-graph/src/outline.rs:224-238 vs 256-260 | A retained child of a `max_symbols`-capped top-level container is elided from the skeleton (parent returns early in `render_node`) yet still listed in `out.symbols` (index filter is `retained && !capped`, and children are never marked `capped`). Untested edge case. | Propagate the cap to descendants when building the index, or exclude symbols whose nearest rendered ancestor was capped. Low impact; non-blocking. |
</findings>

<verdict>
PASS. Zero FAIL findings. All four tier `exit_criteria` independently verified
green: the assembler is pure and façade-re-exported (EC1), the three language
golden snapshots assert fold-marker counts, byte-faithful signatures + doc
comments, span-containment nesting, and the private filter (EC2), the skeleton is
strictly smaller than source with full line accounting (EC3), and clippy / fmt /
architecture / nextest are green (EC4). The code is deterministic (HashMap-free,
stable sort, explicit determinism test), depends only on `ariadne-core` (no new
cross-crate edge, no new dependency — D-hexagonal honoured), and the multi-line
`signature_end` probe (R3) works as specified.

The single red `<verification>` command — `RUSTDOCFLAGS=-D warnings cargo doc`
— fails exclusively on pre-existing broken intra-doc links in three sibling
modules that tier-01 does not touch (confirmed unmodified in the working tree;
the hotspot error text exists verbatim at HEAD). `outline.rs` and the façade add
no doc errors. Per the reviewer standard (judge the diff; bias against
false-positive findings), this pre-existing crate-wide debt is reported as INFO-1
and does not constitute a tier-01 defect.
</verdict>

<next_steps>
- None required for tier-01; the diff is acceptable to commit.
- Recommended (separate tier, not blocking): fix the 3 pre-existing rustdoc
  errors (INFO-1) so the crate's `cargo doc` gate is green for downstream tiers.
- Optional hardening: cover the capped-container/index edge case (INFO-3) when a
  later tier exercises `max_symbols` over nested symbols.
- When committing tier-01, isolate the outline re-exports from the interleaved
  fitness re-exports in `lib.rs` (INFO-2).
</next_steps>

<sources>
- Tier file: `.claude/plans/context-efficient-read/tier-01-outline-projection.md`
- Plan: `.claude/plans/context-efficient-read/plan.md` (D1–D5, R1–R6).
- [Google eng-practices — the standard of code review](https://google.github.io/eng-practices/review/reviewer/standard.html)
- [Google eng-practices — writing review comments](https://google.github.io/eng-practices/review/reviewer/comments.html)
- In-tree evidence: `crates/ariadne-graph/src/outline.rs`, `tests/outline.rs`,
  snapshots; `crates/ariadne-graph/Cargo.toml` (unmodified); architecture test
  pass; `git show HEAD:crates/ariadne-graph/src/hotspot.rs` (pre-existing doc err).
</sources>
