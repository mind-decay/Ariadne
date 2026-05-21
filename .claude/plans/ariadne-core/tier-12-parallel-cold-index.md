---
tier_id: tier-12
title: Parallel cold-index pipeline + opt-in SCIP + >=100K SLO release gate
deps: [tier-11]
exit_criteria:
  - "`run_index` walk + parse parallelised (`ignore` parallel walk + `rayon`); per-file bytes freed before edge resolution; edge resolution stays a post-parse pass."
  - "SCIP ingest gated behind `ariadne index --scip` (default off); the measured cold-index wall-clock excludes external indexer time."
  - "`ariadne-e2e` measures cold-index peak RSS via `/usr/bin/time`; the `slo` test asserts peak < 4 GiB (R1)."
  - "The SLO corpus (`repos.toml` + `slo.rs CORPUS`) assembles >=100,000 genuinely-indexed source files across >=3 languages."
  - "`cargo nextest run -p ariadne-e2e --run-ignored all` green: `slo` (cold < 60s, incremental p95 < 500ms, query p95 < 100ms, peak < 4 GiB) + every `tests/repos/*` cold < 60s."
  - "`docs/adr/0009-parallel-cold-index.md` written, status Accepted."
  - "tier-10 SLO blocker cleared; tier-10 `status` set to `completed`."
status: completed
completed: 2026-05-21
---

<context>
tier-10's SLO release gate failed — risk R8 [src: tier-10-cli-e2e.md
`<blockers>`]: cold-index 354s on `dotnet/runtime`, 442.8s for 55,527 files
combined, against a < 60s SLO. Two root causes:
(a) `run_index` → `walk_repo` → `materialise` is a single-threaded tree-sitter
loop that reads every file's bytes into a `Vec<WalkedFile>` and accumulates the
whole `Changeset` in RAM [src: crates/ariadne-cli/src/domain/mod.rs:140-304].
(b) `run_index` runs external SCIP indexers synchronously inside the measured
wall-clock (`IngestPlan...ingest(root)` sits between `started` and
`elapsed_ms`), while the `ScipDoc`→graph bridge was never built (tier-10 D-A) —
so SCIP costs minutes for zero graph output.
This tier parallelises the syntactic pipeline, moves SCIP off the measured
path, bounds peak memory, grows the corpus past 100K (C/C++ from tier-11), and
re-runs the gate green. Full context: plan.md + tier-10-cli-e2e.md.
</context>

<files>
- docs/adr/0009-parallel-cold-index.md — NEW. Parallel pipeline + opt-in SCIP + memory probe.
- crates/ariadne-cli/Cargo.toml — add `rayon = { workspace = true }`.
- crates/ariadne-cli/src/domain/mod.rs — rewrite walk→parse→materialise as parallel phases + per-phase timing.
- crates/ariadne-cli/src/main.rs — `Index` variant gains `#[arg(long)] scip: bool`.
- crates/ariadne-cli/src/commands/index.rs — thread `--scip` through; print the phase breakdown.
- crates/ariadne-e2e/src/domain/mod.rs — `run_index_measured` (`/usr/bin/time` wrap, peak-RSS parse); `IndexReport` gains `peak_rss_bytes`.
- crates/ariadne-e2e/fixtures/repos.toml — add/repin C/C++-heavy fixture(s) to clear >=100K indexed files.
- crates/ariadne-e2e/tests/slo.rs — update `CORPUS`; assert peak RSS < 4 GiB.
- crates/ariadne-e2e/tests/repos/{c,cpp}.rs — NEW per-language syntactic suites.
</files>

<steps>
1. **Failing measurement first**: instrument `run_index` to record per-phase elapsed (walk, read+parse+extract, edge-resolve, redb commit, SCIP) and emit it on stderr. Run on one fixture; record the breakdown in the tier-12 audit. This attributes the 442.8s before any fix [src: plan.md scope item 2].
2. **SCIP opt-in**: add `--scip` to the `Index` clap variant [src: https://docs.rs/clap]; `run_index` runs `IngestPlan` only when set. The `scip_successes`/`scip_missing` summary fields stay, empty when off. Rationale (ADR-0009): SCIP indexers run full language builds — structurally not a < 60s operation — and the `ScipDoc`→graph bridge is unbuilt (tier-10 D-A), so SCIP is currently pure cost for zero graph data; the cold-index SLO must measure Ariadne's own throughput, not an external compiler.
3. **Parallel walk**: replace the sequential `WalkBuilder::build().flatten()` loop with `WalkBuilder::build_parallel().run(...)`, each visitor pushing recognised paths into a shared sink (`Mutex<Vec<PathBuf>>`) [src: https://docs.rs/ignore/latest/ignore/struct.WalkBuilder.html]. Sort the collected paths, then assign `FileId`s sequentially — ids stay deterministic run-to-run.
4. **Parallel parse**: `paths.into_par_iter().map_init(init, op)` where `init` builds a per-thread `HashMap<Lang, TreeSitterParser>` from a cloned `ParserRegistry`, and `op` reads + parses + extracts facts into `PerFileFacts { FileRecord, decls, calls }`, then `.collect()`. `map_init`'s `init` runs on the worker thread, so each thread owns its parsers — no `Parser` is shared; `ParserRegistry`/`Language` are `Send + Sync` (Arc-backed) [src: https://docs.rs/rayon/latest/rayon/iter/trait.ParallelIterator.html, registry.rs:27-32]. Drop each file's bytes inside the closure once facts are extracted — raw bytes never outlive a single parse, so byte peak is bounded by thread count, not file count.
5. **Sequential assembly**: from the collected `PerFileFacts`, assign symbol ids (`symbol_id` is already path-stable), build `name→symbols`, run the existing `resolve_edges` pass, assemble one `Changeset`, single redb commit. Edge resolution stays a post-parse pass [src: plan.md scope item 1; domain/mod.rs:309-356].
6. **Memory probe**: `ariadne-e2e` `run_index_measured` spawns `/usr/bin/time -l` (macOS — maxrss in bytes) or `/usr/bin/time -v` (Linux GNU — "Maximum resident set size (kbytes)") wrapping `ariadne index`, cfg-branched parse normalised to bytes [src: https://www.baeldung.com/linux/process-peak-memory-usage]. `IndexReport.peak_rss_bytes`; the `slo` test asserts < 4 GiB (R1). The CI image must provide GNU `time` — note it in the release pipeline.
7. **Corpus**: with C/C++ live (tier-11), measure per-repo indexed-file yield; recompose `repos.toml` + `slo.rs CORPUS` so the corpus genuinely clears >=100,000 indexed source files with headroom — C-heavy repos (e.g. a `torvalds/linux` or `redis` subtree) now count. Pin every commit SHA. Add `tests/repos/c.rs` + `cpp.rs`.
8. **Memory fallback**: if step-1 phase data or the R1 probe shows peak > 4 GiB even with bytes streamed, chunk the path list and commit per chunk; otherwise keep the single commit. Decide from measurement, never upfront.
9. **Re-run the gate**: `cargo nextest run -p ariadne-e2e --run-ignored all`. Every `tests/repos/*` cold < 60s; `slo` green on cold + incremental p95 + query p95 + peak RSS. The incremental and query stages run at 100K scale for the first time (`slo.rs` previously panicked at cold before reaching them) — this tier owns whatever they surface; a failure orthogonal to indexing throughput escalates to a further follow-up tier per tier-10's `<verification>` rule.
10. Write ADR-0009 (parallel cold-index pipeline + opt-in SCIP placement + `rayon` into `ariadne-cli` + `/usr/bin/time` memory probe). Set tier-10 `status` to `completed` and note the SLO blocker cleared.
</steps>

<verification>
- `cargo build --workspace`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo fmt --all --check`, `RUSTDOCFLAGS=-D warnings cargo doc --workspace --no-deps --document-private-items` — clean.
- `cargo test --test architecture` green: `rayon` is a workspace dependency [src: Cargo.toml:59] — no new cross-crate edge.
- `cargo nextest run --workspace` green.
- `cargo nextest run -p ariadne-e2e --run-ignored all` — `slo` + every `repos/*` green: cold-index < 60s, incremental p95 < 500ms, query p95 < 100ms, peak RSS < 4 GiB. Any breach fails loud — the bench is never silenced [src: tier-10-cli-e2e.md `<verification>`].
- Per-phase breakdown + measured peak RSS recorded in the tier-12 audit report.
</verification>

<rollback>
`git revert` the `domain/mod.rs` rewrite and the `Cargo.toml` / `main.rs` /
`index.rs` / e2e-harness / `slo.rs` / `repos.toml` edits; delete ADR-0009 and
`tests/repos/{c,cpp}.rs`; restore tier-10 `status: blocked`. The on-disk index
format is unchanged — no data migration. Parallel parse and SCIP-opt-in are
behavioural only; reverting leaves a correct (slow) single-threaded index.
</rollback>

<blockers>
RESOLVED 2026-05-21 by tier-13 — the streaming cold-index pipeline + edge-batch
tuning closed the SLO release gate (cold 40.8s, peak 3434 MiB, incremental p95
408µs, query p95 168µs at 121,100 files). tier-12 is `completed`; the failure
analysis below is retained as the historical record [src: tier-13-cold-index-slo.md].

Build session 2026-05-21. All tier-12 code landed (steps 1-7, 10); non-SLO
verification is green; the SLO release gate still FAILS.

Verification — GREEN:
- `cargo build --workspace`, `clippy -D warnings`, `fmt --check`,
  `RUSTDOCFLAGS=-D warnings cargo doc` (3 pre-existing tier-10 broken
  doc-link citations repaired: `[src:]`->`(src:)`), `cargo test --test
  architecture`, `cargo nextest run --workspace` (129 passed) — all clean.
- Functional smoke (release binary, mixed-lang tree): parallel pipeline
  works, `parse_failures: 0`, phase breakdown on stderr, `--scip` gating
  confirmed, byte-identical output across two `--fresh` runs (deterministic).
- Every per-language `repos/*` suite PASSES the cold-index SLO (the index
  itself < 60s; the larger nextest wall-clock is clone time): rust 2.1s,
  python 7.8s, java 8.7s, cpp 4.1s, typescript 33.8s, go 44.9s,
  csharp 106.1s wall (index < 60s — down from tier-10's 354s), c/linux
  241.6s wall (index < 60s).

SLO release gate — FAILED (`cargo nextest run -p ariadne-e2e --run-ignored
all --release -j 1`). The recomposed corpus genuinely assembles
**121,100 indexed files / 9 langs** (kubernetes + vscode + dotnet/runtime +
linux) — exit_criteria #4 (>=100K) is met. But two SLOs breach:
- cold index **84.343s** vs the < 60s SLO (1.9M symbols, 3.5M edges).
- peak RSS **4833 MiB** vs the < 4096 MiB ceiling (R1). The test panics on
  the cold-index assertion first, so incremental-p95 / query-p95 stay
  unverified at 100K scale (plan R9).

This is a ~9x throughput gain over tier-10 (442.8s/55,527 files ->
84.3s/121,100 files) but short of the line. The parallelisation prescribed
by steps 3-5 is fully implemented; closing the remaining cold-time gap and
the memory overshoot needs a decision beyond this tier's `<steps>`: step 8's
chunked-commit fallback (triggered — peak > 4 GiB) addresses memory but not
the 84.3s, and no `<steps>` item closes the throughput gap.

Phase attribution (step 1) — isolated index of `torvalds/linux` v6.12
(60,096 files, 813,668 symbols, 1,744,450 edges, 24.6s total, peak 1437 MiB):
`walk=98ms parse=20429ms resolve=932ms commit=3167ms scip=0`. Parse is ~83%
of cold time and is already parallel (281s user / 24.6s real ~= 11x core
utilisation); the single-threaded redb commit and the peak RSS both scale
with symbol/edge volume — the corpus's 1.9M symbols / 3.5M edges are why its
84.3s and 4833 MiB exceed a linear scale-up from linux alone. Per-file parse
throughput and a streamed/chunked commit are the levers for the follow-up.

Resolution: per the user, the remaining throughput + memory work escalates
to a new spec tier (tier-13) authored via `/spec-plan`. tier-12 keeps
`status: blocked`; tier-10 stays `blocked` — its SLO blocker is not cleared.
</blockers>
