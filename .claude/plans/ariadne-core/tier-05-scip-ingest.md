---
tier_id: tier-05
title: SCIP ingestion (protobuf decode, external indexer drivers, symbol normalization)
deps: [tier-01, tier-02, tier-04]
exit_criteria:
  - scip.proto compiled via prost-build (build.rs); round-trip a real SCIP file from `rust-analyzer --scip` byte-for-byte.
  - Indexer trait + concrete drivers for: rust-analyzer, scip-typescript, scip-python, scip-java, scip-clang, scip-dotnet, lsif-go (Go via lsif-go + scip lsif-to-scip).
  - ingest_repo(root) -> Vec<ScipDoc> orchestrates drivers in parallel; missing indexers degrade to syntactic-only with a structured warning, never crash.
  - normalize_scip_symbol(raw) produces deterministic IDs across re-runs (proptest 1K cases).
  - Insta golden fixtures: 1 small per-lang repo indexed, snapshot of symbol + relationship counts.
status: pending
---

<context>
Semantic depth comes from SCIP, the language-agnostic protobuf format Sourcegraph adopted to replace LSIF [src: https://sourcegraph.com/blog/announcing-scip, https://github.com/sourcegraph/scip]. v1 reuses existing indexers — we do not write per-lang resolvers. Go is the open gap (R3 in plan.md): no first-party scip-go; fallback path: lsif-go then `scip lsif-to-scip`.
</context>

<files>
- crates/ariadne-scip/Cargo.toml — prost, prost-types, workspace deps; build-dependencies = prost-build.
- crates/ariadne-scip/build.rs — compiles vendored scip.proto (copy from https://github.com/sourcegraph/scip/blob/main/scip.proto into crates/ariadne-scip/proto/scip.proto; pin commit SHA in README).
- crates/ariadne-scip/src/lib.rs — re-exports ScipDoc, Indexer, IngestPlan, CanonicalSymbol, ScipError.
- crates/ariadne-scip/src/proto.rs — `include!(concat!(env!("OUT_DIR"), "/scip.rs"));` re-exports protobuf types.
- crates/ariadne-scip/src/indexer/mod.rs — Indexer trait + registry.
- crates/ariadne-scip/src/indexer/{rust_analyzer,scip_ts,scip_py,scip_java,scip_clang,scip_dotnet,lsif_go}.rs — one driver per lang.
- crates/ariadne-scip/src/normalize.rs — symbol/relationship normalization.
- crates/ariadne-scip/tests/golden/<lang>/ — small fixture repos (vendored as git submodule or copied minimal sample).
- crates/ariadne-scip/tests/roundtrip.rs — load a real SCIP file, decode, re-encode, byte-compare.
- crates/ariadne-scip/tests/normalize.rs — proptest determinism.
- crates/ariadne-scip/tests/ingest_<lang>.rs — golden insta snapshot of `ingest_repo` output for one repo per lang.
</files>

<steps>
1. Vendor scip.proto from sourcegraph/scip at a pinned commit (record SHA in crates/ariadne-scip/proto/SCIP_COMMIT) [src: https://github.com/sourcegraph/scip/blob/main/scip.proto].
2. build.rs invokes `prost_build::compile_protos(&["proto/scip.proto"], &["proto/"])`. Emit into OUT_DIR; include in proto.rs.
3. **Failing test first** (tests/roundtrip.rs): use `rust-analyzer --scip` (pre-generated, stored in tests/fixtures/sample.scip from a tiny Rust crate fixture) → decode via prost → re-encode → byte-compare. Fails until step 2 succeeds and types match.
4. Define Indexer trait:
   ```rust
   pub trait Indexer: Send + Sync {
       fn lang(&self) -> Lang;
       fn detect(&self, root: &Path) -> bool;          // is this lang present?
       fn run(&self, root: &Path, out: &Path) -> Result<()>;
       fn parse(&self, scip_bytes: &[u8]) -> Result<ScipDoc>;
   }
   ```
   Returning `Result` so a single broken indexer cannot poison the run.
5. RustAnalyzerIndexer: invokes `rust-analyzer scip <root> --output <out>` via `std::process::Command`; detect = exists `Cargo.toml`; parse = `scip::Index::decode(bytes)` [src: https://github.com/rust-lang/rust-analyzer (scip subcommand)].
6. ScipTypescriptIndexer: invoke `scip-typescript index --output <out>`; detect = `package.json` + `tsconfig.json`; [src: https://github.com/sourcegraph/scip-typescript].
7. ScipPythonIndexer: invoke `scip-python index --project-name <name> --output <out> --cwd <root>`; detect = `pyproject.toml` or `setup.py` [src: https://github.com/sourcegraph/scip-python].
8. ScipJavaIndexer: invoke `scip-java index --output <out> --build-tool <gradle|maven|bazel|sbt>`; detect = `build.gradle*` / `pom.xml` / `BUILD` / `build.sbt` [src: https://github.com/sourcegraph/scip-java].
9. ScipClangIndexer: requires compile_commands.json; invoke `scip-clang --compdb <path> --out <out>`; detect = `compile_commands.json` exists [src: https://github.com/sourcegraph/scip-clang].
10. ScipDotnetIndexer: invoke `scip-dotnet index --output <out>`; detect = `*.sln` or `*.csproj` [src: https://github.com/sourcegraph/scip-dotnet].
11. LsifGoIndexer (Go fallback, R3): invoke `lsif-go --no-animation --output dump.lsif` then `scip convert --from=lsif --in=dump.lsif --out=out.scip` [src: https://github.com/sourcegraph/lsif-go, https://github.com/sourcegraph/scip CLI docs]. Detect = `go.mod` exists. Document the two-step in driver doc-comment.
12. IngestPlan orchestrator: walks `root`, asks each `Indexer::detect`, runs surviving ones in parallel with `rayon::scope` (cap parallelism to `num_cpus / 2`). Each driver writes to a per-lang temp dir; failures logged via `tracing::warn!` and surfaced in returned `IngestReport { successes: Vec<Lang>, failures: Vec<(Lang, ScipError)> }`.
13. Normalization (src/normalize.rs): SCIP symbol grammar is documented in scip.proto comments [src: https://github.com/sourcegraph/scip/blob/main/scip.proto]. Implement `CanonicalSymbol = { scheme, manager, package_name, version_or_none, descriptors: Vec<Descriptor> }`. Stable hash: `blake3(serialized canonical form)` truncated to 64 bits → SymbolId.
14. **Failing test first** (tests/normalize.rs): proptest 1K random SCIP symbol strings (legal grammar); assert `normalize(s)` is deterministic across 10 invocations and that two equivalent forms (whitespace, ordering of well-known suffixes) hash equal.
15. Per-lang golden tests (tests/ingest_<lang>.rs): each fixture repo (≤100 LOC) is indexed; insta snapshots `IngestReport` + symbol-count + relationship-count + top-5 symbols by occurrence count.
16. Wire `ScipDocInput` (tier-04 Salsa) consumer to call `ariadne-scip::parse` on the raw bytes; integration covered by tier-07 end-to-end.
17. Document the indexer install matrix in crates/ariadne-scip/README.md: name, install command, min version, detected-by signal.
</steps>

<verification>
- `cargo nextest run -p ariadne-scip` green; round-trip + normalize + per-lang golden all pass.
- Manual: on a real small repo per lang (e.g., ariadne_v2 itself for Rust, a 100-line TS demo, etc.), run `ariadne-cli ingest-scip` (stub command) and inspect `IngestReport`. Expected: each lang reports `successes` if its indexer is on PATH; failures surface with actionable install hints, not silent skips.
- Go path verified: indexing `golang/example` (small public repo) via lsif-go → scip convert yields non-empty `ScipDoc` with `Definition` + `Reference` occurrences.
- If any indexer is missing on PATH, `IngestPlan` returns a `Warning::IndexerMissing { lang, install_hint }` rather than crashing.
</verification>

<rollback>
`git rm -r crates/ariadne-scip` + workspace member removal. The vendored proto file goes with it. No external state to clean up beyond temp dirs (tier creates them under `std::env::temp_dir()` and unlinks on drop).
</rollback>
