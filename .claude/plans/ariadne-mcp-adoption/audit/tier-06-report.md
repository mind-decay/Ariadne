---
tier_id: tier-06
audited: 2026-06-03
verdict: PASS
commit: b2159990b225b08af440caaf9521a4aa1cdb96ea
---

<scope>
Tier-06 — search/read spike. A throwaway, `#[ignore]`d harness measures the
deterministic response-token delta between the reflexive `grep` + whole-file-`Read`
path and a `search-hit` + exact-span path, gating tiers 07–09 (plan.md D11). No
product code. Reviewed the working-tree diff on top of HEAD `b215999` scoped to the
tier's `<files>` (deliverable uncommitted):

- `crates/ariadne-e2e/tests/search_read_spike.rs` — new, `#[ignore]`d. Copies this
  repo's live `.ariadne/index.redb` to a tempdir, opens the copy via the driven
  `ariadne-storage` adapter, replicates the mcp `Catalog` name→span projection
  (`iter_files`→paths, `iter_symbols`→`by_name` first-wins), runs a fixed 10-task
  set across the three shapes (find-definition / search-by-pattern / read-body),
  computes per-task byte + `bytes/4` deltas, and writes the report. Imports only
  `ariadne_core`, `ariadne_storage`, `regex`, `serde_json` — `ariadne-mcp` not linked.
- `crates/ariadne-e2e/Cargo.toml` — adds `ariadne-storage` (workspace) + `regex`
  ("1.12.3") as **dev-dependencies** only.
- `.claude/plans/ariadne-mcp-adoption/spike-search-read.md` — generated data
  artifact: per-task table (bytes + tokens, both arms), median, go/no-go verdict.
- `Cargo.lock` — adds `ariadne-storage` + `regex` to the `ariadne-e2e` dep list;
  `regex 1.12.3` was already resolved transitively (via `ignore`), so no new crate
  enters the graph.

`git status` confirms nothing outside the e2e crate + the plan/tier markdown was
touched; no product code introduced, as the tier asserts.
</scope>

<checks_run>
Every command in the tier's `<verification>` re-run on `b215999`:

- `cargo nextest run -p ariadne-e2e --run-ignored all -E 'test(search_read_spike)'`
  → **1 passed, 18 skipped**. The report exists, is non-empty, and contains the
  `Median reduction` line, the `Verdict` line, and the snapshot `revision`.
- *Determinism* — ran the harness twice back-to-back; the two emitted reports are
  **byte-identical** (`diff` empty). The harness contains no `Instant`/`SystemTime`/
  timestamp/random; its only inputs are the index copy + live source bytes. A fresh
  run differs from the committed artifact by exactly one line — `Index revision:
  447` → `448` — because the live daemon advanced the index since generation; all
  costs, the median (87.3%), and the verdict are unchanged. This is a *different
  repo state*, not a determinism violation; the criterion ("byte-identical on the
  same repo state") holds. Committed artifact restored to as-found afterward.
- `cargo clippy -p ariadne-e2e --all-targets -- -D warnings` → clean (compiles the
  `#[ignore]` target; the four local `cast_*`/`naive_bytecount` allows are scoped to
  the throwaway arithmetic and documented in the module header).
- `cargo fmt --all --check` → clean.
- `cargo test --test architecture` → `architecture_invariants_hold` pass. Confirmed
  by reading `tests/architecture.rs`: `ariadne-storage` ∈ `DRIVEN_ADAPTERS` (l.41),
  `ariadne-mcp` ∈ `DRIVING_ADAPTERS` (l.56); rule (4) (l.126-140) bans any non-root
  crate from linking a driving adapter — a dev-edge to a *driven* adapter is legal,
  and `regex` is filtered out as a non-workspace member. Exit criterion 5 verified.
- *Fail-loudly* re-read in source: missing index → `assert!(index.is_file())` (l.244);
  empty snapshot → `assert!(!paths.is_empty())` / `!symbols.is_empty()` (l.279-280);
  missing target → `panic!("target … not in catalog")` (l.296); zero baseline →
  `assert!(baseline > 0)` (l.313). No fabricated-row path.
- *Replication fidelity* cross-checked against `crates/ariadne-mcp/src/catalog.rs:106-165`:
  the spike's `by_name` first-wins in `iter_symbols` order matches `Catalog::find_symbol`
  (`by_name.get(name).first()`); the search arm mirrors `list_symbols` lowercase
  `contains` + a `RegexBuilder` name match, capped at the production default 64.
- *Deviation validated* (test header l.11-16): step 2 prescribes opening
  `.ariadne/index.redb` directly, but `RedbStorage::open` → `Database::create` takes
  an exclusive redb lock and `bootstrap` **writes** the META table
  (`crates/ariadne-storage/src/adapters/redb/mod.rs:58,68-69`). Against the
  daemon-held live file a direct open would both fail and mutate the live index, so
  the copy-to-tempdir open is the correct call — byte-identical data, same revision,
  a real run, no live-index mutation. Justified and documented; not a defect.
</checks_run>

<findings>
| id | category | severity | location | problem | fix |
|----|----------|----------|----------|---------|-----|
| 1 | docs | INFO | `.claude/plans/ariadne-mcp-adoption/tier-06-search-read-spike.md:103-104` | The diff appends orphan `</content>` + `</invoke>` closing tags after `</rollback>` — unmatched, no opening tag; malformed against the "XML tags wrap semantic sections" convention. | Delete both stray lines from the tier file. Non-blocking — affects only the spec doc, not the harness, build, tests, or any exit criterion. |
</findings>

<verdict>
PASS. Zero FAIL findings; every gating `<verification>` command re-runs green and
all five `exit_criteria` are independently verified:

1. *Throwaway `#[ignore]` harness, real run, no `ariadne-mcp`* — confirmed: opens a
   byte-copy of the live index (rev 447/448), substring + regex symbol search and
   byte-span source-read, replicates the `Catalog` projection, links only
   core/storage/regex/serde_json.
2. *Report records both arms (bytes + `bytes/4`), per-task reduction, median,
   revision* — all present in the table + `## Result`.
3. *Explicit ≥40% verdict citing the number* — `render_report` branches on
   `median >= 400`; output reads "PROCEED … Median reduction 87.3% ≥ 40% (D11)".
   Median re-derived by hand from the 10 rows: sorted middle pair (83.8, 90.9) →
   87.3% (even-n mean of indices 4,5). Internally consistent.
4. *Byte-identical re-run* — demonstrated (run1 ≡ run2); harness is timestamp/random-free.
5. *Architecture green, only storage + regex dev-deps, `ariadne-mcp` never linked* —
   `cargo test --test architecture` passes; `Cargo.toml`/`Cargo.lock` diff adds
   exactly those two dev-deps.

Architecture holds (dev-edge to a driven adapter only; no driving→driving dep, no
smuggled tech — `regex 1.12.3` was already in `Cargo.lock`). The whole-file-`Read`
baseline assumption and the `bytes/4` proxy are stated explicitly in the report,
and the gate is a *relative* delta so the divisor cancels. The copy-to-tempdir
deviation is necessary, documented, and preserves the "real run" intent. The 87.3%
median clears the 40% threshold by a wide margin, so the spike's go decision for
tiers 07–09 is well-supported.
</verdict>

<next_steps>
None required for tier-06. The spike verdict is PROCEED, so tiers 07–09 are live per
D11. Optional cleanup: drop the two orphan XML tags (finding 1) when next editing the
tier file. The harness + artifact are deleted "after the decision" per the tier's
`<rollback>` — a downstream step, not a tier-06 deliverable.
</next_steps>

<sources>
- Tier + plan under review: `.claude/plans/ariadne-mcp-adoption/tier-06-search-read-spike.md`,
  `plan.md` (D11 spike gate, D8/D9 tool shapes).
- Replication cross-check: `crates/ariadne-mcp/src/catalog.rs:106-165` (Catalog build /
  `find_symbol`); `crates/ariadne-storage/src/adapters/redb/mod.rs:58,68-69` (open writes);
  `tests/architecture.rs:40-45,56,126-140` (driven/driving sets, rule 4).
- [OpenAI — what are tokens / `bytes÷4` heuristic](https://help.openai.com/en/articles/4936856-what-are-tokens-and-how-to-count-them) (token proxy, cited in the harness).
- [Code review standard — Google eng-practices](https://google.github.io/eng-practices/review/reviewer/standard.html) (ship-if-satisfies; the documented deviation is accepted, not nitpicked).
</sources>
