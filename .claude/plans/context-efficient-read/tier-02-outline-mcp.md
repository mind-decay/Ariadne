---
tier_id: tier-02
title: read_outline MCP tool — live-file bytes + Catalog spans → folded skeleton
deps: [tier-01]
exit_criteria:
  - "`read_outline` is a registered `#[tool]` on `AriadneServer` taking `{path, include_private?}` and returning a `SourceOutline` (path, revision, stale, folded skeleton, symbol index, kept/elided line counts) built by the tier-01 graph use case."
  - "Over-stdio and `handle` tests assert the skeleton folds bodies, keeps signatures/doc comments, flags `stale` with clamping on a truncated fixture, and that an unindexed (zero-symbol) file returns a line-count note instead of dumping the file."
  - "The skeleton's byte length is strictly less than a whole-file read for a multi-symbol fixture; outline p95 <100ms on this repo."
  - "Regenerated tools-list handshake snapshot accepted; clippy `-D warnings`, fmt, `cargo test --test architecture` (no new cross-crate edge), and `cargo nextest run -p ariadne-mcp` green."
status: completed
completed: 2026-06-07
---

<context>
This driving adapter exposes the tier-01 use case over MCP. It mirrors
`read_symbol`: resolve the target, read the live file under `Catalog.root`, never
fail on stale spans — clamp and flag [src:
crates/ariadne-mcp/src/adapters/source.rs:74-99; tier-08 D9]. The symbol set for a
file is the `file_summary` enumeration pattern — iterate `cat.symbols`, keep
`meta.file == file_id`, sort by `byte_start` [src:
crates/ariadne-mcp/src/tools/file_summary.rs:25-102]. `ariadne-mcp` already
depends on `ariadne-graph` (the coupling/docgen tools route there), so calling
`outline::assemble` adds no new cross-crate edge [src: plan.md architecture;
CLAUDE.md invariants].
</context>

<files>
- `crates/ariadne-mcp/src/types.rs` — `ReadOutlineInput { path, include_private:
  Option<bool> }` and `SourceOutline` output (path, revision, stale, skeleton,
  `Vec<OutlineEntry>`-shaped index, kept_lines, elided_lines, optional note).
- `crates/ariadne-mcp/src/tools/read_outline.rs` — `handle(cat, input)`: resolve
  file, map symbols → `OutlineRequest`, read bytes, call `assemble`, wrap.
- `crates/ariadne-mcp/src/tools/mod.rs` — register the `read_outline` module +
  any shared `summarize`-style mapping [src:
  crates/ariadne-mcp/src/tools/mod.rs].
- `crates/ariadne-mcp/src/adapters/source.rs` — add `read_file(root, rel_path) ->
  (Vec<u8>, stale)` returning whole bytes (stale = any span would exceed len;
  computed by the handler against EOF). Reuse line/clamp helpers.
- `crates/ariadne-mcp/src/server.rs` — `read_outline` async `#[tool]` arm using
  the cold catalog, like `read_symbol` [src: crates/ariadne-mcp/src/server.rs:239-249].
- `crates/ariadne-mcp/tests/tools_read_outline.rs` — `handle` + over-stdio tests.
- `crates/ariadne-mcp/tests/snapshots/handshake__*tools*.snap` — accept the
  regenerated tools-list (one new tool).
</files>

<steps>
1. **Failing test first.** In `tools_read_outline.rs`, build a catalog over a
   temp fixture file with several symbols; call `handle` and assert: the
   `skeleton` contains every signature, folds each body to a marker, omits private
   symbols when `include_private=false`, and `skeleton.len() <
   fs::read(file).len()`. Add a truncated-file case → `stale == true`, clamped.
   Add a zero-symbol file → a `note` with the line count, no file dump. Run —
   fails (no tool).
2. **Map + assemble.** Implement `handle`: resolve `file_id` via
   `cat.path_to_id`; collect `OutlineSymbol`s from `cat.symbols` filtered by file,
   sorted by `byte_start`; take `lang` from the symbols' `meta.lang`; read bytes
   via `source::read_file`; build `OutlineRequest { include_private:
   input.include_private.unwrap_or(true), max_symbols: <cap> }`; call
   `ariadne_graph::outline::assemble`; attach `revision`, `stale`, `path` [src:
   tier-01; catalog.rs:71-97]. Zero symbols → return the note branch (advise
   native `Read`), never dump.
3. **Tool arm.** Add the `#[tool]` method on `AriadneServer` delegating to
   `tools::read_outline::handle(&self.catalog().await?, &input)`, `wire(&out)`,
   matching the `read_symbol` arm shape and rmcp `Parameters`/`CallToolResult`
   conventions [src: crates/ariadne-mcp/src/server.rs:239-249;
   https://docs.rs/rmcp/1.7.0/rmcp/index.html `#[tool]`/`#[tool_router]`].
4. **Tool description.** Write a trigger-phrase doc string: "Read a whole file as
   a token-cheap code skeleton — signatures + doc comments kept, bodies folded;
   expand a body with `read_symbol`. Use instead of reading a whole source file."
   (Discoverability wiring into `with_instructions`/CLAUDE.md is tier-04.)
5. **Snapshots.** Run the handshake test; accept the regenerated tools-list snap
   (verify only the new tool was added).
6. **Bench p95.** Add/extend a timing assertion or reuse the existing tool bench
   harness to confirm outline p95 <100ms on this repo [src: plan.md SLO].
</steps>

<verification>
- `cargo nextest run -p ariadne-mcp` — `handle`, over-stdio, stale/clamp,
  zero-symbol note, and private-filter cases pass; handshake snap accepted.
- `cargo test --test architecture` — no new cross-crate edge (mcp→graph already
  exists); façade unchanged.
- clippy `-D warnings`; `cargo fmt --all --check`.
- Real run: `read_outline` on `crates/ariadne-mcp/src/server.rs` in a live
  session → skeleton far smaller than a whole-file `Read`; report the byte delta.
  If not runnable in-session, say so; never fabricate the ratio [src: CLAUDE.md
  validate-by-execution].
</verification>

<rollback>
Remove the tool module, the `server.rs` arm, the `types.rs` additions, the
`source::read_file` helper, and the test; revert the handshake snap. Tier-01 is
untouched; no config or data path changes.
</rollback>
