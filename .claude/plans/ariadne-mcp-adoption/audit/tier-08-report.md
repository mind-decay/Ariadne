---
tier_id: tier-08
audited: 2026-06-03
verdict: PASS
commit: b2159990b225b08af440caaf9521a4aa1cdb96ea
---

<scope>
Tier-08 `read_symbol` MCP tool — resolve a symbol to its defining span and
return the live on-disk slice in `signature | full | context` mode, with
`stale` clamping. Scoped diff (tier-08 `<files>`):
- `crates/ariadne-mcp/src/types.rs` — `ReadSymbolInput` + `SourceSlice`.
- `crates/ariadne-mcp/src/adapters/source.rs` — new; `read_span` + `SourceMode` (only `std::fs`).
- `crates/ariadne-mcp/src/adapters/mod.rs` — `pub mod source;`.
- `crates/ariadne-mcp/src/tools/read_symbol.rs` — new; resolve + delegate.
- `crates/ariadne-mcp/src/tools/mod.rs` — `pub mod read_symbol;`.
- `crates/ariadne-mcp/src/server.rs` — `#[tool] read_symbol` (server.rs:230-249).
- `crates/ariadne-mcp/src/errors.rs` — `InvalidInput` variant used by `SourceMode::parse`.
- `crates/ariadne-mcp/tests/tools_read_symbol.rs` — new; 7 tests incl. stdio real-run.
- `crates/ariadne-mcp/tests/snapshots/handshake__tools_{list,descriptions}.snap` — accepted.
Working tree also carries uncommitted tier-07 (`search_code`) and plan-doc
edits; those belong to the tier-07 audit (already PASS) and were not assessed here.
</scope>

<checks_run>
- `cargo nextest run -p ariadne-mcp -E 'test(read_symbol)'` → 7 passed (incl. `read_symbol_over_stdio_full_and_signature` real-run).
- `cargo nextest run -p ariadne-mcp -E 'test(handshake)'` → 5 passed.
- `cargo nextest run -p ariadne-mcp` (whole crate, exit-criterion #5) → 75 passed, 0 skipped.
- `cargo clippy -p ariadne-mcp --all-targets -- -D warnings` → clean.
- `cargo fmt --all --check` → exit 0.
- `cargo test --test architecture` → `architecture_invariants_hold` ok.
- IO-under-adapters (exit #4): `grep std::fs crates/ariadne-mcp/src` — `read_symbol.rs` has zero (comment only); `adapters/source.rs:68` holds the only `std::fs::read`. Handler stays pure.
- Logic trace of `read_span` against the DEMO/OTHER fixtures: full/signature/context byte+line bounds, stale clamp, and underflow/out-of-range slice paths — no panic path found (`line_end > start` guards the `:` drop; clamped bounds keep `out_start <= out_end <= len`).
- Snapshots `handshake__tools_list.snap:510` + `handshake__tools_descriptions.snap:21` include `read_symbol` with the trigger-phrase description and `alwaysLoad` meta.
</checks_run>

<findings>
| id | category | severity | location | problem | fix |
|----|----------|----------|----------|---------|-----|
| INFO-1 | correctness | INFO | `tools/read_symbol.rs:37-44` | Plan step 4 says "else first + note"; when `file` is omitted and several symbols share the name the handler returns `ids[0]` silently — the caller cannot tell other overloads existed (the `name`/`file` fields only reveal *which* was picked, not that a choice was made). | When `file.is_none() && ids.len() > 1`, surface the alternative count/paths (e.g. in a field or log) so the caller knows to disambiguate. Non-blocking. |
</findings>

<verdict>
PASS. Zero FAIL findings. All five exit criteria independently verified:
1. `read_symbol` registered (server.rs:239) and present in `list_tools`; both
   handshake snapshots updated and their tests green.
2. `tests/tools_read_symbol.rs` passes — `full` slice == on-disk `[start,end]`;
   `signature` == declaration line (no body brace); `context` widens ±N; response
   carries `file`, 1-based `line_start`/`line_end`, and `revision`.
3. Truncated-file case (`read_symbol_truncated_file_is_flagged_stale_and_clamped`)
   returns `stale:true` with a clamped slice and no panic (R7).
4. Disk IO isolated in `adapters/source.rs`; the handler does no `std::fs`.
5. nextest (75/75), clippy `-D warnings`, fmt, and architecture test all green.
Security: `input.file` is only equality-matched against catalog paths for
disambiguation; the actual read joins `root` with a catalog-derived path, so no
user-controlled path-traversal reaches `std::fs::read`. Lossy-UTF8 decode means a
truncated multi-byte char yields a replacement char, never a panic.
</verdict>

<next_steps>
- Optional (INFO-1): emit an ambiguity signal when resolving an overloaded name
  without `file`. Not required to ship tier-08.
- Out of scope, flagged for a future audit: `tools/diff_blast.rs:146` (tier-15c,
  already committed) performs `std::fs::read` directly in a tool handler rather
  than via an adapter — the same IO-under-adapters convention tier-08 honors. Not
  a tier-08 defect; noted so it is not lost.
</next_steps>

<sources>
- Tier file: `.claude/plans/ariadne-mcp-adoption/tier-08-read-symbol-tool.md` (exit_criteria, steps, verification).
- Plan: `.claude/plans/ariadne-mcp-adoption/plan.md` D9, R7.
- Code: `crates/ariadne-mcp/src/adapters/source.rs`, `src/tools/read_symbol.rs`, `src/catalog.rs:78-184`, `src/server.rs:230-249`, `src/errors.rs:21-46`.
- Convention: CLAUDE.md "IO lives under src/adapters/"; OWASP path-traversal (A01) — confirmed not reachable.
</sources>
