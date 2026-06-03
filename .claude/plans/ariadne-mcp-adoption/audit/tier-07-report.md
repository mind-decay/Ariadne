---
tier_id: tier-07
audited: 2026-06-03
verdict: PASS
commit: b2159990b225b08af440caaf9521a4aa1cdb96ea
---

<scope>
Audited tier-07 (`search_code` MCP tool) against its tier file and sibling
`plan.md`. Diff scoped to the tier `<files>`: `Cargo.toml` (+regex/glob, +bench
entry), `src/types.rs` (`SearchCodeInput`), `src/tools/search_code.rs` (new
handler), `src/tools/mod.rs` (`pub mod`), `src/server.rs` (`#[tool]` method),
`tests/tools_search_code.rs` (new), the two `handshake__tools_*.snap`
snapshots, and `benches/search_latency.rs` (new). `tests/handshake.rs`
(`EXPECTED_TOOLS` 17→18) was also touched — outside the listed `<files>` but a
necessary, trivially-correct consequence of registering an 18th tool; recorded,
not a finding. Other working-tree changes (tier-06/08/09 plan files, e2e
spike artifacts) belong to other audits and were excluded.
</scope>

<checks_run>
- plan_adherence: every `<files>` entry touched as intended; change set is
  strictly additive — no existing tool signature or behavior altered. Cargo.lock
  delta is 4 lines (regex/glob promoted from transitive `ignore` deps to direct
  `ariadne-mcp` deps), versions exactly `regex 1.12.3` / `glob 0.3.3` per D10.
- correctness: read `search_code.rs` end-to-end. Matcher compiled once
  (substring lowercase-`contains` default / case-insensitive `Regex`); path glob
  compiled once; per-symbol filters (kind exact, lang `tag()` ci, visibility ci,
  path `matches_path`); global rank (exact>prefix>other, then name, then id) via
  collect→sort→truncate(limit) — top-K is global, not first-K. Deterministic
  (BTreeMap iteration + total-order sort).
- security: regex bounded by `size_limit(1<<20)` + `nest_limit(64)` → linear
  time, no ReDoS (EC3). Invalid regex/glob map to typed `McpError::Other`, never
  `unwrap`/panic. Pure in-RAM projection — no disk IO, no injection surface, no
  path traversal (glob matches stored relative paths only).
- performance: regex/glob compiled once; substring fast-path; early scan is a
  single linear pass; `limit` truncation. Bench (100k symbols / 500 files, worst-
  case broad substring + broad anchored regex) ran: p50 4.295ms, **p95 4.630ms**,
  p99 4.791ms vs 100ms budget — well under SLO (EC4 / R6).
- architecture: handler returns `Vec<SymbolSummary>` only; `regex`/`glob` types
  never leak into the public API; `Matcher` is private. No new domain port (D8,
  pure `Catalog` projection). `cargo test --test architecture` green.
- tests: 9 unit cases (substring/regex/path/kind/lang/visibility/limit + invalid
  regex + invalid glob) plus 1 stdio e2e over the real rmcp transport. Assert
  behavior (expected name sets, typed `Err`), fail loudly.
- docs/verification: re-ran the tier `<verification>` in full (below).
- exit_criteria: all 5 independently verified — see findings/verdict.

Commands re-run (full output captured):
- `cargo fmt --all --check` → exit 0.
- `cargo nextest run -p ariadne-mcp -E 'test(search_code)'` → 10/10 pass.
- `cargo nextest run -p ariadne-mcp -E 'test(handshake)'` → 5/5 pass (tool list
  + descriptions snapshots include `search_code`, count 18).
- `cargo nextest run -p ariadne-mcp` → 67/67 pass (full package, EC5).
- `cargo bench -p ariadne-mcp --no-run` → builds (5 bench executables).
- `cargo bench -p ariadne-mcp --bench search_latency` → p95 4.630ms < 100ms.
- `cargo clippy -p ariadne-mcp --all-targets --all-features -- -D warnings` → 0.
- `cargo test --test architecture` → 1 passed.
</checks_run>

<findings>
| id | category | severity | location | problem | fix |
|----|----------|----------|----------|---------|-----|
| I1 | exit_criteria | INFO | benches/search_latency.rs:11-14 | EC4 wording says "criterion bench" but the bench is a custom `harness=false` runner, not the `criterion` crate. | None required — `criterion` is not in `<tech_inventory>`, sibling `cold_start`/`concurrent` benches are custom too, and only a custom harness can exit non-zero to gate R6; substance (p95 asserted < SLO on 100k catalog) is met. Surfaced so the user confirms the reading. |
| I2 | performance | INFO | src/tools/search_code.rs:94 | `name_lc = meta.name.to_lowercase()` allocates per symbol unconditionally, but in regex mode it is unused (the `Regex` arm matches `meta.name` and `rank` returns 2 without reading it). | Optional: compute `name_lc` lazily / only on the substring path. Non-gating — bench shows 4.63ms p95 vs 100ms budget, ample headroom. |
| I3 | correctness | INFO | src/errors.rs:30-32 | Invalid-regex/glob (caller input) surface as JSON-RPC `internal_error` via the shared `into_rmcp`, not `invalid_params`, so a client cannot distinguish bad input from a server fault. | Optional: map caller-input variants to `invalid_params`. Pre-existing crate-wide convention (also used by `NotFound`), not introduced by this tier; EC3 only requires a typed error and no panic, which holds. |
</findings>

<verdict>
PASS. Zero FAIL findings. All five exit criteria verified by execution: the
18th tool `search_code` is registered and present in `list_tools`
(snapshots + `EXPECTED_TOOLS` updated); the TDD behavior suite passes for every
filter and both invalid-input cases; invalid regex/glob return a typed
`McpError` (never a panic) with the regex `size_limit`+`nest_limit` bounded; the
latency bench asserts p95 4.630ms ≪ 100ms on a 100k-symbol catalog; and
`nextest`, `clippy -D warnings`, `fmt`, and the architecture invariant test are
all green. The change is strictly additive, hexagonal boundaries hold (pure
`Catalog` projection, no new port, no leaked `regex`/`glob` types), and the
promoted deps match D10 exactly. The three INFO items are non-blocking.
</verdict>

<next_steps>
None required for PASS. If addressed opportunistically in a later tier:
consider mapping caller-input errors to `invalid_params` (I3) and trimming the
unconditional `to_lowercase` allocation (I2) — both crate-wide, out of this
tier's additive scope. Tier-08 (`read_symbol`) may proceed.
</next_steps>

<sources>
- [regex RegexBuilder 1.12.3](https://docs.rs/regex/1.12.3/regex/struct.RegexBuilder.html) — `size_limit`/`nest_limit` bound program size + nesting (ReDoS mitigation).
- [glob Pattern 0.3.3](https://docs.rs/glob/0.3.3/glob/struct.Pattern.html) — `matches_path`, `**` component matching.
- [OWASP Top 10](https://owasp.org/www-project-top-ten/) — input-validation lens for the regex/glob compile paths.
- [Google eng-practices — reviewer standard](https://google.github.io/eng-practices/review/reviewer/standard.html) — code-health-over-perfection; INFO vs FAIL gating.
</sources>
