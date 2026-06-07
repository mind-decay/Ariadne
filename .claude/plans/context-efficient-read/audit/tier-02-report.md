---
tier_id: tier-02
audited: 2026-06-07
verdict: PASS
commit: 0af641eb20fe515e34782d60fa539ff1169b7c58
---

<scope>
Reviewed `tier-02-outline-mcp.md` (slug `context-efficient-read`) against its
sibling `plan.md` and the working-tree diff scoped to the tier's `<files>`:
- `crates/ariadne-mcp/src/types.rs` — `ReadOutlineInput`, `OutlineEntry`, `SourceOutline`.
- `crates/ariadne-mcp/src/tools/read_outline.rs` — `handle` (new).
- `crates/ariadne-mcp/src/tools/mod.rs` — `pub mod read_outline`.
- `crates/ariadne-mcp/src/adapters/source.rs` — `read_file` helper.
- `crates/ariadne-mcp/src/server.rs` — `read_outline` `#[tool]` arm.
- `crates/ariadne-mcp/tests/tools_read_outline.rs` — `handle` + over-stdio tests.
- `crates/ariadne-mcp/tests/handshake.rs` + `snapshots/handshake__*tools*.snap` — count 21→23, snapshot.

Out of scope and not audited here: tier-01 internals (`ariadne-graph/src/outline.rs`,
separately PASSed at this same commit) and the **intelligence-platform block-a
tier-04 `fitness_report`** changes that share `types.rs` / `server.rs` /
`tools/mod.rs` / the handshake snapshot in the same uncommitted working tree
(separate plan, separate audit). The live Ariadne MCP (installed binary, rev 67)
predates this uncommitted tool, so the server.rs live real-run is not
reproducible via the shipped binary; the over-stdio test is the real-server
substitute (see INFO-2).
</scope>

<checks_run>
- `cargo fmt --all --check` — clean (exit 0).
- `cargo nextest run -p ariadne-mcp` — 88 passed / 0 failed; all 6 `tools_read_outline`
  cases + over-stdio pass.
- `cargo test -p ariadne-mcp --test handshake` — 5 passed; tools-list + descriptions
  snapshots accepted, `EXPECTED_TOOLS == 23` matches the registered set.
- `cargo test --test architecture` — 1 passed; no new cross-crate edge (mcp→graph
  pre-existed).
- `cargo clippy -p ariadne-mcp -p ariadne-graph --all-targets --all-features -- -D warnings`
  — clean, no warnings.
- Read end-to-end: `read_outline.rs`, `types.rs` additions, `source::read_file`,
  the server arm, the test file, and `ariadne-graph` re-exports
  (`OutlineEntry/OutlineOptions/OutlineRequest/OutlineSymbol/assemble` at
  `lib.rs:51`) + `Catalog`/`SymbolMeta` field shapes (`catalog.rs:26-96`) — all
  names and types used by the handler exist and match.

<exit_criteria_verification>
1. Registered `#[tool]` taking `{path, include_private?}` → `SourceOutline` built
   via the tier-01 use case — MET (`server.rs:252-272`; `SourceOutline` carries
   path, revision, stale, skeleton, symbol index, kept/elided counts, optional note).
2. handle + over-stdio assert fold / signatures / docs, stale+clamp on truncation,
   zero-symbol note — MET (`tools_read_outline.rs:166-291,316-370`).
3. skeleton bytes < whole-file; p95 <100ms — MET for the byte delta
   (`tools_read_outline.rs:208-215,343-347`); p95 asserted but on a 3-symbol
   fixture, not a repo-scale file (INFO-2).
4. Snapshot accepted; clippy/fmt/architecture/nextest green — MET (all re-run green).
</exit_criteria_verification>
</checks_run>

<findings>
| id | category | severity | location | problem | fix |
|----|----------|----------|----------|---------|-----|
| INFO-1 | plan_adherence | INFO | types.rs:762-809; server.rs:715-735; tools/mod.rs:20; handshake.rs:21-25; handshake__tools_*.snap | Working tree intermingles intelligence-platform block-a tier-04 `fitness_report` into tier-02's listed files and regenerated snapshot; a commit on this PASS would carry that out-of-tier, separately-audited code. read_outline itself is unaffected. | Commit tier-02 (read_outline) separately from the fitness_report work; ensure fitness_report is covered by its own plan's audit before it ships. |
| INFO-2 | performance | INFO | tools_read_outline.rs:294-308 | The p95<100ms SLO test loops over the 3-symbol tempdir fixture, not a repo-scale file, so it does not meaningfully exercise the "on this repo" SLO; correctness is covered by the over-stdio + handle tests. | Sample timing against a real multi-symbol repo file (e.g. server.rs) or the standard workload to make the SLO assertion load-bearing. |
</findings>

<verdict>
PASS. Zero FAIL findings. The `read_outline` tool is correctly wired: the handler
resolves the file via `cat.path_to_id`, enumerates symbols by the `file_summary`
pattern (filter by `file`, sort by `(byte_start, byte_end)`), reads live bytes
through the `src/adapters/` IO helper, delegates the projection to the pure
tier-01 `ariadne_graph::assemble`, computes `stale` against EOF (clamp-not-fail,
R5), and returns a line-count note instead of dumping a zero-symbol file (D2).
Visibility, default `include_private=true`, stale/clamp, and the byte-delta
property are each asserted by a passing test, including a real-server over-stdio
run. Architecture invariants hold (no driving→driving edge, IO isolated under
`src/adapters/`). Both INFO items are process/coverage notes that do not block the
change.
</verdict>

<next_steps>
- None required to land tier-02 on its own merits.
- Before committing: separate the read_outline diff from the intermingled
  `fitness_report` work (INFO-1) so the audit-gate covers each tier with its own
  verdict.
- Optional: strengthen the p95 assertion against a repo-scale file (INFO-2).
</next_steps>

<sources>
- Re-run command output (this session): fmt, `nextest -p ariadne-mcp`, handshake,
  `--test architecture`, clippy `-D warnings`.
- `crates/ariadne-graph/src/lib.rs:51` (outline re-exports);
  `crates/ariadne-mcp/src/catalog.rs:26-96` (Catalog/SymbolMeta fields).
- [rmcp 1.7.0 `#[tool]`/`#[tool_router]`](https://docs.rs/rmcp/1.7.0/rmcp/index.html) — tool-arm shape matches `read_symbol`.
- [Google eng-practices — reviewer standard](https://google.github.io/eng-practices/review/reviewer/standard.html) (code-health-over-perfection gate for INFO severity).
</sources>
