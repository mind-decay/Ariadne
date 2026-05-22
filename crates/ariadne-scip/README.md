# ariadne-scip

Driven adapter for SCIP ingestion. Wraps the per-language SCIP indexers
Sourcegraph publishes, decodes their protobuf output via `prost`, and
exposes a parallel `IngestPlan` orchestrator that surface-degrades to
syntactic-only when an indexer is absent rather than crashing.

## Indexer install matrix

`ariadne-scip` never bundles a language indexer — it shells out to one
already on `PATH`. Missing indexers surface as `IngestReport.warnings`
with the install hint below; they are warnings, never failures
[src: `.claude/plans/ariadne-core/tier-05-scip-ingest.md`].

| Language        | Driver type                                     | Binary           | Min version | Install                                                                                | Detect signal at project root                          |
|-----------------|-------------------------------------------------|------------------|-------------|----------------------------------------------------------------------------------------|--------------------------------------------------------|
| Rust            | `RustAnalyzerIndexer`                           | `rust-analyzer`  | 2024-03-04  | `rustup component add rust-analyzer` (or `brew install rust-analyzer`)                 | `Cargo.toml`                                           |
| TypeScript / JS | `ScipTypescriptIndexer`                         | `scip-typescript`| 0.3.13      | `npm install -g @sourcegraph/scip-typescript`                                          | `package.json` + `tsconfig.json` both present          |
| Python          | `ScipPythonIndexer`                             | `scip-python`    | 0.6.0       | `npm install -g @sourcegraph/scip-python`                                              | `pyproject.toml` or `setup.py`                         |
| Java / Kotlin   | `ScipJavaIndexer`                               | `scip-java`      | 0.10.0      | `coursier install scip-java` (or `brew install sourcegraph/scip/scip-java`)            | `build.gradle*`, `pom.xml`, `BUILD`, `WORKSPACE`, `build.sbt` |
| C / C++         | `ScipClangIndexer`                              | `scip-clang`     | 0.2.0       | Download from <https://github.com/sourcegraph/scip-clang/releases>                     | `compile_commands.json`                                |
| C#              | `ScipDotnetIndexer`                             | `scip-dotnet`    | 0.4.0       | `dotnet tool install -g SourcegraphScipDotnet`                                         | any `*.sln` or `*.csproj` directly under root          |
| Go              | `ScipGoIndexer`                                 | `scip-go`        | 0.2.6       | `go install github.com/scip-code/scip-go/cmd/scip-go@latest`                           | `go.mod`                                               |

Go is indexed by the native `scip-go`, run from the module root. It
supersedes the v1 two-step LSIF fallback (v1 plan risk R3, resolved by
post-v1-roadmap RD1).

## Build inputs

- `proto/scip.proto` is vendored from `sourcegraph/scip` at the SHA
  recorded in `proto/SCIP_COMMIT`. Update by re-pinning the SHA and
  re-running `cargo build` — `build.rs` is `rerun-if-changed` against
  both files.
- `protoc` is supplied by the `protoc-bin-vendored` crate, so the build
  is hermetic on systems without a system `protoc`
  [src: <https://crates.io/crates/protoc-bin-vendored>].
- `Config::disable_comments(["."])` strips proto comments from the
  generated Rust because they include grammar diagrams rustdoc misreads
  as doctests at the current SHA.

## Per-language goldens

`tests/ingest_<lang>.rs` synthesizes a minimal `proto::Index` per
language, encodes via prost, then decodes through the public `parse`
free function and snapshots the resulting `LangSummary`. The
synthesis-then-decode flow exercises the exact contract a
`ScipDocInput` consumer (tier-04+ salsa, tier-07 use cases) sees from
the raw bytes; the driver `run()` subprocess paths are covered
separately by `tests/ingest_plan.rs` (stub drivers across
success / `IndexerMissing` / `SubprocessFailed`) and by
`tests/roundtrip.rs` (a real `rust-analyzer scip` payload).

Why synthesis: at tier-05 build time only `rust-analyzer` was present on
the build host. Pre-generating real SCIP bytes for the other six
languages would require installing the full JVM, .NET, Python, Node, and
Go toolchains. Synthetic fixtures keep the test suite deterministic and
CI-portable; live-binary verification belongs to tier-10 once a CI image
with all seven indexers exists.
