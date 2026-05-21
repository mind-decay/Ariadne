# ADR-0009: Parallel cold-index pipeline + opt-in SCIP

<status>
Accepted
Date: 2026-05-21
Decider: user
</status>

<context>
The tier-10 SLO release gate failed — risk R8 materialised. The cold full-index
took 354s on `dotnet/runtime` alone and 442.8s for a 55,527-file corpus,
against a < 60s SLO [src: ../../.claude/plans/ariadne-core/tier-10-cli-e2e.md
`<blockers>`]. Two root causes:

1. **Single-threaded, hold-everything model.** `run_index` → `walk_repo` →
   `materialise` was a sequential tree-sitter loop: it read every file's bytes
   into a `Vec<WalkedFile>` kept live for the whole run, then parsed each file
   on one thread [src: crates/ariadne-cli/src/domain/mod.rs, tier-10 revision].
   On a many-core host the parse phase used one core and the resident byte
   buffer grew with the file count — both a throughput and a memory ceiling
   problem (plan risk R1, < 4 GB).
2. **SCIP on the measured path.** `run_index` ran external SCIP indexers
   synchronously between the start and end timestamps, while the
   `ScipDoc`→`Changeset` bridge was never built (tier-10 D-A) — so SCIP cost
   minutes for zero graph output, inside the number the SLO judges.

Forces: the cold index must scale to a genuine 100K-file workload within 60s
and stay under the 4 GB ceiling, without changing the on-disk format and
without dragging a non-Rust runtime onto the critical path (plan.md D5).
</context>

<decision>
1. **Parallelise the syntactic pipeline.** The file-system walk uses `ignore`'s
   `build_parallel().run(...)` — each worker pushes recognised paths into a
   shared `Mutex<Vec<PathBuf>>`; the collected paths are sorted before
   `FileId` assignment so ids stay deterministic run-to-run. Reading, parsing,
   and fact extraction fan out across a `rayon` thread pool via
   `par_iter().map_init(...)`: the `init` closure builds a per-worker
   `HashMap<Lang, TreeSitterParser>` from a cloned `ParserRegistry`, so no
   `tree_sitter::Parser` (which is `!Send`) is ever shared. Each file's byte
   buffer is read, hashed, parsed, and **dropped inside the closure** — raw
   bytes never outlive a single parse, so the byte peak is bounded by the
   worker count, not the file count.
2. **Keep edge resolution a sequential post-parse pass.** Symbol-id assignment,
   `name→symbols` indexing, call-edge resolution, and the single redb write
   transaction run once, after the parallel parse collects.
3. **Make SCIP opt-in.** External SCIP indexers run only under
   `ariadne index --scip` (default off). The cold-index wall-clock the SLO
   measures is Ariadne's own walk→parse→commit throughput, never an external
   compiler's build time.
4. **`rayon` becomes an `ariadne-cli` dependency** — it is already a workspace
   dependency (tier-05 `IngestPlan`), pure-Rust, no cgo [src: Cargo.toml].
5. **Verify peak RSS by measurement.** `ariadne-e2e` wraps `ariadne index` in
   `/usr/bin/time` (`-l` on macOS, `-v` for GNU `time` on Linux), parses the
   maxrss figure normalised to bytes, and the `slo` test asserts peak < 4 GiB
   (plan risk R1).
</decision>

<rationale>
- **Efficiency / scalability** — parsing is embarrassingly parallel per file;
  `rayon`'s work-stealing pool saturates every core with no hand-rolled thread
  management. `map_init` gives each worker its own parsers, which is the only
  safe way to share tree-sitter across threads (`Parser` is `!Send`);
  `ParserRegistry`/`tree_sitter::Language` are `Arc`-backed `Send + Sync`, so
  the registry clone is an `Arc` bump [src: registry.rs].
- **Reliability (memory ceiling, R1)** — dropping each file's bytes inside the
  parse closure caps the resident raw-byte total at roughly
  `worker_count × file_size`, decoupling peak RSS from corpus size. The
  retained `SyntacticFacts` are orders of magnitude smaller than source text.
  The `/usr/bin/time` probe turns the 4 GB ceiling into an asserted gate
  rather than an assumption.
- **Correctness preserved** — `FileId` is the only value affected by walk
  order, and sorting the path list makes it deterministic; symbol ids are
  `blake3(path#name@offset)` (path-stable) and edge keys are symbol-based, so
  the graph is byte-identical to the sequential build. The on-disk redb format
  is unchanged — no data migration.
- **Maintainability** — SCIP-opt-in is a single `bool` threaded through `clap`;
  the pipeline keeps its three named phases (walk / parse / assemble) with
  per-phase timings emitted on stderr, so a regression is attributable without
  a profiler.
</rationale>

<alternatives>
- **Chunked / streaming redb commit** — rejected as the default, kept as a
  measurement-gated fallback. If the `/usr/bin/time` probe shows peak > 4 GiB
  even with bytes streamed, the path list is committed in chunks; with bytes
  already dropped per-parse the single commit is expected to hold, so the
  added transaction complexity is not paid upfront [src: tier-12 step 8].
- **Hand-rolled `std::thread` pool** — rejected. Reimplements work-stealing,
  back-pressure, and pool sizing that `rayon` already provides; `rayon` is
  pure-Rust and already vendored.
- **Keep SCIP on the measured path, parallelised** — rejected. SCIP indexers
  run full language builds (compiler invocations); that is structurally not a
  < 60s operation, and with the `ScipDoc`→graph bridge unbuilt it is pure cost
  for zero graph data [src: tier-10 D-A]. Parallelism cannot fix a
  wrong-thing-measured problem.
- **Content-classify before parsing** — out of scope; `lang_for_path` by
  extension is unchanged.
</alternatives>

<consequences>
- The CI image (and any host) running the `ariadne-e2e` `slo` suite on Linux
  must provide GNU `time` at `/usr/bin/time` — the BSD/macOS `time` builtin is
  not it. macOS hosts already ship `/usr/bin/time -l`. The release pipeline
  builds artifacts only; the `#[ignore]`d SLO suite is run explicitly, so this
  is a SLO-runner prerequisite, recorded here and in the tier-12 audit.
- `FileId` integers now follow sorted path order, not filesystem walk order —
  a behavioural change invisible to every consumer (symbol/edge identity is
  path/symbol-based) and a determinism improvement.
- `ariadne index` gains a `--scip` flag; the JSON summary's `scip_successes` /
  `scip_missing` arrays are empty unless it is passed. Per-phase timings
  (`walk parse resolve commit scip`) print on stderr.
- `IndexReport` in `ariadne-e2e` gains `peak_rss_bytes`, populated only by
  `run_index_measured`; `0` after a plain `run_index`.
- Reverting is behavioural-only: the single-threaded path produces an
  identical (slower) index, the on-disk format is untouched.
</consequences>

<sources>
- `[src: https://docs.rs/rayon/latest/rayon/iter/trait.ParallelIterator.html]` — `map_init` per-thread initialiser.
- `[src: https://docs.rs/ignore/latest/ignore/struct.WalkParallel.html]` — `WalkParallel::run` parallel walk.
- `[src: https://www.baeldung.com/linux/process-peak-memory-usage]` — `/usr/bin/time` peak-RSS reporting (`-l` macOS bytes, `-v` GNU kbytes).
- `[src: ../../.claude/plans/ariadne-core/plan.md]` — `<constraints>` SLOs, risk R1, risk R8.
- `[src: ../../.claude/plans/ariadne-core/tier-12-parallel-cold-index.md]` — this tier.
- `[src: ../../.claude/plans/ariadne-core/tier-10-cli-e2e.md]` — `<blockers>`, deviation D-A.
- `[src: crates/ariadne-parser/src/adapters/treesitter/registry.rs]` — `ParserRegistry` clone-cheap, `Language` `Arc`-backed.
</sources>
