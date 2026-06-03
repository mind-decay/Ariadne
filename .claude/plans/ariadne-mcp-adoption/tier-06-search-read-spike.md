---
tier_id: tier-06
title: Search/read spike — prototype both calls, measure token delta vs grep+Read, go/no-go gate
deps: []
exit_criteria:
  - "A throwaway, `#[ignore]`d harness `crates/ariadne-e2e/tests/search_read_spike.rs` runs against THIS repo's live `.ariadne/index.redb` (real run): substring+regex symbol search and byte-span source-read, replicating the mcp `Catalog` projection — no `ariadne-mcp` dependency, no production code added."
  - "`.claude/plans/ariadne-mcp-adoption/spike-search-read.md` records, per fixed task, the baseline (grep match lines + whole-file Read) vs prototype (search hits + span +3 context lines) cost as both raw bytes and `bytes/4` token proxy, the per-task reduction, the median, and the snapshot `revision`."
  - "The report states an explicit verdict: proceed to tiers 07–09 iff median reduction ≥40%; otherwise mark 07–09 cancelled with the measured number."
  - "Re-running the harness on the same repo state yields a byte-identical report (no model call, no wall-clock, no timestamp embedded)."
  - "`cargo test --test architecture` stays green: only `ariadne-storage` (a driven adapter) + `regex` are added as e2e dev-dependencies; `ariadne-mcp` is never linked."
status: completed
completed: 2026-06-03
---

<context>
"Measure first" (plan.md D11): prove the token payoff before committing to the two
production tools. The metric is a deterministic response-token proxy on a fixed
task set — what a reflexive grep+whole-file-Read path would emit versus what a
search-hit + exact-span path would emit. No `claude -p` run, so the gate never
flakes (anti-flake rule) [src: CLAUDE.md feedback_validation_required; Anthropic
eval-driven tooling https://www.anthropic.com/engineering/writing-tools-for-agents].
The grep-only path is the thing under suspicion of burning tokens [src:
https://milvus.io/blog/why-im-against-claude-codes-grep-only-retrieval-it-just-
burns-too-many-tokens.md].

The prototype must mirror the *production* projection so its numbers transfer to
tiers 07–08: the mcp `Catalog` streams a redb snapshot into a name→(file, span)
map [src: crates/ariadne-mcp/src/catalog.rs:106-131]. It cannot *import* that code —
`tests/architecture.rs:71-83` counts dev-dependencies and rule (4) (lines 126-139)
bans any crate but the composition root from linking the driving adapter
`ariadne-mcp` [src: tests/architecture.rs:56,126-139]. So the spike *replicates*
the projection over the driven `ariadne-storage` adapter (allowed: it is in
`DRIVEN_ADAPTERS`) [src: tests/architecture.rs:40-45]. The index is live (rev 439,
root = this repo) so the run is real, not stubbed [src: mcp__ariadne__project_status].
</context>

<files>
- `crates/ariadne-e2e/tests/search_read_spike.rs` — new, `#[ignore]`. Throwaway:
  opens this repo's redb snapshot, replicates the catalog name→span map, runs the
  task set, computes per-task byte/token deltas, writes the report. Deleted after
  the decision (auto-discovered by cargo — directly under `tests/`).
- `crates/ariadne-e2e/Cargo.toml` — add `ariadne-storage` (workspace) + `regex`
  ("1.12.3") as **dev-dependencies** only. `ariadne-storage` is a driven adapter so
  the dev-edge is architecture-legal; `regex` is already in `Cargo.lock`
  [src: tests/architecture.rs:40-45; Cargo.lock regex 1.12.3].
- `.claude/plans/ariadne-mcp-adoption/spike-search-read.md` — generated data
  artifact: the per-task table, median, and go/no-go verdict.
</files>

<steps>
1. **Task set.** Hard-code ~10 fixed `(intent, query, regex, target_name)` tuples
   over real symbols in this repo spanning the three shapes: find-definition
   (`Catalog`, `summarize`), search-by-pattern (`^handle`, `.*_report$`,
   `^iter_`), read-body (`Catalog::build` → name `build`). Hard-coded so the run is
   reproducible; assert each `target_name` resolves or fail loudly (no fabricated row).
2. **Load symbols.** `RedbStorage::open(repo_root.join(".ariadne/index.redb"))`
   [src: crates/ariadne-storage/src/adapters/redb/mod.rs:58], then replicate the
   catalog build: `storage.snapshot()` → `iter_files(4096)` for path+lang, then
   `iter_symbols(4096)` into a `BTreeMap<name, Vec<(path, byte_start, byte_end,
   kind)>>` — the same projection as the production catalog
   [src: crates/ariadne-mcp/src/catalog.rs:106-131; ports.rs:191-204]. Fail loudly
   if the store is absent or empty.
3. **Baseline cost (grep + whole-file Read).** Per task, estimate the bytes a
   reflexive agent emits with no Ariadne: the grep hit *lines* for the query
   (matching-line text across scanned source files) **plus** the full text of each
   file it must `Read` to inspect the symbol. State the whole-file assumption
   explicitly in the report (it models the reflexive path D11 targets, not a
   line-range-savvy reader).
4. **Prototype cost (search + span read).** Run substring (lowercase `contains`,
   like `list_symbols`) and `regex::RegexBuilder` over the symbol map → hits
   serialized in the `SymbolSummary` shape (name, kind, file, 1-based line range);
   read the body as `fs::read(root/path)[byte_start..byte_end]` plus 3 context
   lines [src: crates/ariadne-mcp/src/tools/list_symbols.rs:12-32; types.rs:36].
5. **Token proxy.** Convert both arms with `tokens = bytes / 4` (OpenAI English
   rule of thumb; documented as an approximation, valid here because the gate is a
   *relative* delta — the proxy's scaling cancels) [src:
   https://help.openai.com/en/articles/4936856-what-are-tokens-and-how-to-count-them].
   Report raw bytes alongside tokens so the verdict does not hinge on one divisor.
6. **Report + verdict.** Compute per-task reduction `(baseline−prototype)/baseline`
   and the median; write `spike-search-read.md` with the table (bytes + tokens per
   arm, per-task %, median), the embedded snapshot `revision`, and an explicit
   "proceed ≥40%" / "cancel <40%" verdict citing the number. No timestamp.
</steps>

<verification>
- `cargo nextest run -p ariadne-e2e --run-ignored all -E 'test(search_read_spike)'`
  runs green; `spike-search-read.md` exists, is non-empty, contains the median +
  verdict line + the revision.
- Determinism: run twice on an unchanged index → byte-identical report (no
  `Instant`/timestamp/random in the harness or the output).
- `cargo clippy -p ariadne-e2e --all-targets -- -D warnings`; `cargo fmt --check`;
  `cargo test --test architecture` (green — `ariadne-storage`+`regex` dev-deps only,
  no `ariadne-mcp` link).
- Fail loudly: if `.ariadne/index.redb` cannot open, the snapshot is empty, or a
  target symbol is missing, abort with the cause — never write a fabricated delta.
  If the spike cannot run in-session, say so and leave the verdict unrecorded.
</verification>

<rollback>
Delete `search_read_spike.rs`, the report, and the two e2e dev-dependency lines. No
production code is touched, so nothing else reverts.
</rollback>
