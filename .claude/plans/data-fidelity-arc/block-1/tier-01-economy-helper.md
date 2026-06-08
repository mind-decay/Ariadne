---
tier_id: tier-01
title: Shared response-economy helper + find_references pilot end-to-end
deps: []
exit_criteria:
  - "`ariadne_graph::economy` exposes `Verbosity`, `Budget`, an opaque `Cursor` codec, and a generic `paginate`; a unit test proves encode→decode round-trips and a wrong-revision cursor is rejected, not silently mis-paged."
  - "`find_references` caps at the default page (50) across MCP-cold, MCP-warm/daemon, and CLI `query`, returning `next_cursor` + `note` when truncated; a round-trip test shows page-1 ∪ page-2 … equals the un-capped reference set in stable order with no gap/dup."
  - "`verbosity` defaults to concise (omits caller `id` + byte offsets, keeps `caller_name`/`file`); a test asserts concise fields ⊂ detailed fields and concise byte count < detailed; `ariadne digest` (if it consumes references) pins detailed."
  - "Cold `find_references` JSON == warm daemon JSON == CLI `query find_references` JSON (parity)."
  - "ADR-0029 records the mechanism; clippy `-D warnings`, fmt, `cargo test --test architecture`, and the ariadne_v2 self-index dogfood are green."
status: completed
completed: 2026-06-08
---

<context>
Foundation tier: build the one shared helper the whole block reuses, and prove the entire vertical on
the simplest growable tool — `find_references` (a single `Vec<ReferenceSite>`, present on all three
serving paths) [src: crates/ariadne-mcp/src/tools/find_references.rs:20-49; .claude/plans/data-fidelity-arc/block-1/plan.md D1,D2,D3]. Establishing the contract here makes tiers 02–04 mechanical
repetition. Full context + decisions: `plan.md`. ADR-0029 (next free number) records the mechanism
[src: docs/adr/ highest = 0028].
</context>

<files>
- `crates/ariadne-graph/src/economy.rs` — NEW pure module: `Verbosity{Concise,Detailed}`,
  `Budget{limit:usize, cursor:Option<Cursor>, verbosity:Verbosity}`, `Cursor{revision:u32, offsets:Vec<u64>}`
  with an opaque hand-rolled encode/decode (no new dep, D2), and
  `paginate<T>(rows:Vec<T>, sort:impl Fn, budget, revision, sublist_index) -> Page<T>{rows, next_cursor:Option<String>}`.
- `crates/ariadne-graph/src/lib.rs` — re-export the economy surface from the façade.
- `crates/ariadne-core/src/domain/daemon/query.rs` — add `limit`/`cursor`/`verbosity` to the
  `FindReferences` variant.
- `crates/ariadne-core/src/domain/daemon/response.rs` — change `References(Vec<ReferenceSite>)` to a
  wrapper carrying `next_cursor`/`note` (or wrap in a `ReferencesReport`); make `ReferenceSite`
  cryptic fields (`caller`, `byte_start`, `byte_end`) optional for concise (D3).
- `crates/ariadne-mcp/src/types.rs` — mirror `Verbosity`; add `FindReferencesInput{symbol,limit,cursor,verbosity}`
  (a dedicated input so `find_definition`/`doc_for`, which share `SymbolQuery`, are untouched);
  add `next_cursor`/`note` to the references output; make the cryptic `ReferenceSite` fields skip-able.
- `crates/ariadne-mcp/src/tools/find_references.rs` — sort by `(file, byte_start, caller_name)`, then
  `economy::paginate`; apply concise projection.
- `crates/ariadne-mcp/src/server.rs` — `find_references` `#[tool]` takes the new input; `project_daemon`
  handles the wrapped response.
- `crates/ariadne-daemon/src/domain/queries/` (+ `dispatch.rs`) — warm `find_references` calls the same
  `economy::paginate`.
- `crates/ariadne-cli/src/commands/query.rs` — thread `limit`/`cursor`/`verbosity` in `build_query`,
  `dispatch`, and `project` for `find_references`.
- `docs/adr/0029-response-economy-cursor-verbosity.md` — NEW (use the template).
</files>

<steps>
1. **Failing tests first (graph).** In `economy.rs` tests: (a) `Cursor` encode→decode round-trips a
   `{revision, offsets}`; (b) decoding a string from a different revision yields a typed
   invalid-cursor error; (c) `paginate` over a 7-item vec with `limit:3` returns 3 + a `next_cursor`,
   and feeding that cursor returns the next 3 + cursor, then the last 1 + `None`, the union equal to
   the sorted input (no gap/dup). Run — fails (module absent). [src: MCP opaque-cursor + "handle
   invalid cursors gracefully", https://modelcontextprotocol.io/specification/2025-06-18/server/utilities/pagination]
2. **Implement `economy.rs`.** Cursor codec: serialize `revision` + `offsets` to bytes and a
   url-safe/hex string (opaque, hand-rolled — no base64/cbor dep) [src: crates/*/Cargo.toml; plan.md
   D2]. `paginate`: apply the caller's stable `sort`, slice `[offset .. offset+limit]` for this
   sublist, set `next_cursor` only when more remain. Re-export from `lib.rs` (façade convention)
   [src: crates/ariadne-graph/src/lib.rs:1-49]. Graph tests pass.
3. **Thread the protocol (core).** Add `limit`/`cursor`/`verbosity` to `DaemonQuery::FindReferences`;
   wrap the references response with `next_cursor`/`note`; make `ReferenceSite.caller`/`byte_start`/
   `byte_end` `Option` with `#[serde(skip_serializing_if = "Option::is_none")]` [src:
   crates/ariadne-core/src/domain/daemon/{query,response}.rs; serde derives in types.rs]. Build core.
4. **Cold handler.** In `tools/find_references.rs`, after building rows, sort by `(file, byte_start,
   caller_name)`, call `economy::paginate` with the catalog `revision`, and in concise mode leave the
   cryptic fields `None`; set `note` from the page (the steer string, D5) [src: find_references.rs:30-48].
5. **Failing parity + concise tests (mcp).** Assert: default call caps at 50 with a cursor on a hot
   symbol; concise fields ⊂ detailed; cold == warm JSON. Run — fails. Then wire `server.rs` `#[tool]`
   input + `project_daemon`, and the warm daemon handler (same `paginate` call) [src: server.rs:300-315,770-795]. Tests pass.
6. **CLI.** Thread the params through `query.rs` `build_query`/`dispatch`/`project`; if `digest`
   consumes references, pin `verbosity:detailed` there [src: crates/ariadne-cli/src/commands/query.rs:120-122,267-271].
7. **ADR-0029.** Record: helper home (`ariadne-graph`), opaque revision-stamped cursor, concise
   default + lossless detailed, no-new-dep codec, no redb migration. Note BR4 (in-workspace protocol,
   daemon restarts on revision change) [src: plan.md D1,D2,D3, risks].
</steps>

<verification>
- `cargo nextest run -p ariadne-graph -E 'test(economy)'` — codec round-trip, invalid-cursor reject,
  paginate completeness.
- `cargo nextest run -p ariadne-mcp -E 'test(find_references)'` — cap + cursor round-trip + concise⊂detailed + cold/warm parity.
- `cargo test --test architecture`; clippy `-D warnings`; `cargo fmt --all --check`;
  `RUSTDOCFLAGS=-D warnings cargo doc -p ariadne-graph --no-deps`.
- Manual dogfood: `ariadne query find_references '{"symbol":"Lang"}'` returns ≤50 rows + a cursor;
  re-run with the cursor → the next page; union matches the prior un-capped 194-row output. Report the
  before/after token counts. If not runnable in-session, say so — never fabricate [src: CLAUDE.md
  validate-by-execution].
</verification>

<rollback>
Revert the economy module + re-export, the core protocol/DTO changes, the find_references handler +
input + server/daemon/cli wiring, and ADR-0029. The other nine tools are untouched (not yet wired),
so the build returns to the pre-tier state cleanly.
</rollback>
