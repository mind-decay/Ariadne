---
tier_id: tier-06
title: Search/read spike — prototype both calls, measure token delta vs grep+Read, go/no-go gate
deps: []
exit_criteria:
  - "A throwaway, `#[ignore]`d harness runs against THIS repo's live index (real run): substring+regex symbol search and byte-span source-read, no production code added to `ariadne-mcp`."
  - "`.claude/plans/ariadne-mcp-adoption/spike-search-read.md` records, per fixed task, the baseline (grep match + whole-file Read) vs prototype (search hit + span) response-token estimate and the median reduction."
  - "The report states an explicit verdict: proceed to tiers 07–09 iff median reduction ≥40%; otherwise mark 07–09 cancelled with the measured number."
  - "Re-running the harness on the same repo state yields identical numbers (no model call, no wall-clock)."
status: pending
---

<context>
"Measure first" (plan.md D11): prove the token payoff before committing to the
two production tools. The metric is a deterministic response-token proxy on a
fixed task set — what a grep+whole-file-Read agent path would emit versus what a
search-hit + exact-span path would emit. No `claude -p` run, so the gate never
flakes (anti-flake rule) [src: CLAUDE.md feedback_validation_required; Anthropic
eval-driven tooling https://www.anthropic.com/engineering/writing-tools-for-agents].
The grep-only path is the thing under suspicion of burning tokens [src:
https://milvus.io/blog/why-im-against-claude-codes-grep-only-retrieval-it-just-
burns-too-many-tokens.md]. The prototype mirrors `catalog.rs` (snapshot → in-RAM
symbol map) so its numbers transfer to the real tools [src: catalog.rs:81-127].
</context>

<files>
- `crates/ariadne-e2e/tests/search_read_spike.rs` — new, `#[ignore]`. Throwaway:
  builds a symbol map from this repo's redb snapshot, runs the task set, computes
  per-task token deltas, writes the report. Deleted after the decision.
- `crates/ariadne-e2e/Cargo.toml` — add `ariadne-storage` + `ariadne-core` as
  **dev-dependencies** only (test-only; no production/hexagonal dep added).
- `.claude/plans/ariadne-mcp-adoption/spike-search-read.md` — generated data
  artifact: the per-task table, median, and go/no-go verdict.
</files>

<steps>
1. **Task set.** Define ~10 fixed (intent, query, target) tuples over real symbols
   in this repo spanning the three shapes: find-definition (`Catalog`, `summarize`),
   search-by-pattern (`^handle`, `*_report`), read-body (`Catalog::build`). Hard-code
   them so the run is reproducible.
2. **Load symbols.** Open the repo's store via `ariadne_core::Storage::snapshot()` +
   `iter_symbols`/`iter_files` exactly as `catalog.rs` does, into a `BTreeMap` of
   name→(file, span) [src: catalog.rs:88-113]. Fail loudly if the store is absent.
3. **Baseline cost (grep+Read).** Per task, estimate tokens an agent emits with no
   Ariadne: the grep hit lines for the query plus the full text of each file it must
   `Read` to inspect the symbol. Token proxy = `bytes / 4`; state the proxy in the
   report [src: rough GPT-style token heuristic, documented as an approximation].
4. **Prototype cost (search+read).** Run substring+`regex` over the symbol map →
   `SymbolSummary`-shaped hits; read the span as `fs::read(root/path)[start..end]`
   (+3 context lines). Token proxy = serialized hits + the slice only.
5. **Report + verdict.** Compute per-task delta and the median; write
   `spike-search-read.md` with the table, the median reduction, and an explicit
   "proceed ≥40%" / "cancel <40%" verdict citing the number.
</steps>

<verification>
- `cargo nextest run -p ariadne-e2e --run-ignored all` (or
  `-E 'test(search_read_spike)'`) runs the spike green; the report file exists,
  is non-empty, and contains the median + verdict line.
- Determinism: run twice on an unchanged index → byte-identical report numbers.
- `cargo clippy -p ariadne-e2e --all-targets -- -D warnings`; `cargo fmt --check`.
- Fail loudly: if the snapshot cannot be opened or a target symbol is missing,
  abort with the cause — never write a fabricated delta. If the spike cannot run
  in-session, say so and leave the verdict unrecorded.
</verification>

<rollback>
Delete `search_read_spike.rs`, the report, and the e2e dev-dependency lines. No
production code is touched, so nothing else reverts.
</rollback>
