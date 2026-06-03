---
tier_id: tier-08
title: read_symbol MCP tool — return a symbol's exact source span from disk
deps: [tier-06]
exit_criteria:
  - "A new `read_symbol` tool is registered on `AriadneServer` and appears in the rmcp `list_tools` output; the `handshake__tools_list.snap` + `handshake__tools_descriptions.snap` insta snapshots are updated to include it."
  - "`tests/tools_read_symbol.rs` (failing first) passes: `full` mode returns bytes equal to the on-disk file slice `[byte_start, byte_end]`; `signature` mode returns the declaration line(s); `context` mode returns ±N surrounding lines; the response carries file, 1-based line range, and `revision`."
  - "On a fixture whose file was truncated after indexing, the tool returns `stale:true` with a clamped slice and never panics or fabricates bytes (R7)."
  - "Disk IO lives in `crates/ariadne-mcp/src/adapters/source.rs`, not in the tool handler (IO-under-adapters convention)."
  - "`cargo nextest run -p ariadne-mcp`, clippy `-D warnings`, fmt, and `cargo test --test architecture` are green."
status: completed
completed: 2026-06-03
---

<context>
The spike (tier-06) cleared the gate, so build the source-retrieval primitive that
removes whole-file `Read`s. Ariadne stores no source text — only spans into files
on disk (no source-text table) — so `read_symbol` resolves a symbol to its
defining span and reads the live file under `Catalog.root`, returning just the
slice [src: plan.md D9; crates/ariadne-mcp/src/catalog.rs:96 root; storage
tables.rs:15-36]. It is the
first MCP tool that touches the filesystem, so the IO is isolated in a new
`adapters/source.rs` per the convention "IO lives under src/adapters/" [src:
CLAUDE.md conventions]. Spans can be stale after an edit: the tool reads the
current file, clamps an out-of-range span, and flags `stale:true` rather than
failing or serving wrong bytes [src: plan.md D9, R7].
</context>

<files>
- `crates/ariadne-mcp/src/types.rs` — `ReadSymbolInput` + `SourceSlice` output.
- `crates/ariadne-mcp/src/adapters/source.rs` — new; `read_span(root, rel_path,
  start, end, mode, ctx) -> Result<SourceSlice, McpError>` (the only `std::fs` use).
- `crates/ariadne-mcp/src/adapters/mod.rs` — `pub mod source;`.
- `crates/ariadne-mcp/src/tools/read_symbol.rs` — resolve symbol → span, delegate
  to `adapters::source::read_span`.
- `crates/ariadne-mcp/src/tools/mod.rs` — `pub mod read_symbol;`.
- `crates/ariadne-mcp/src/server.rs` — `#[tool]` method delegating one-line
  [src: server.rs:186-209].
- `crates/ariadne-mcp/tests/tools_read_symbol.rs` — temp-dir fixtures.
- `crates/ariadne-mcp/tests/snapshots/` — accept updated `handshake__tools_list.snap`
  + `handshake__tools_descriptions.snap`.
</files>

<steps>
1. **Failing test.** In `tools_read_symbol.rs`, write a temp file with known
   content, build a fixture `Catalog` whose symbol span points into it, call the
   handler, assert: `full` slice == `file_bytes[start..end]`; `signature` ==
   first declaration line; `context` includes ±N lines; line range is 1-based and
   correct; mutate the file shorter and assert `stale:true` + clamped slice. Run —
   fails (tool absent).
2. **Types.** `ReadSymbolInput { symbol: String, file: Option<String> (disambiguate
   overloads), #[serde(default)] mode: Option<String> /* signature|full|context */,
   context_lines: Option<u32> }`; `SourceSlice { name, file, line_start, line_end,
   byte_start, byte_end, revision, stale, source }`, deriving the standard
   `Deserialize, Serialize, JsonSchema` set [src: types.rs:36,70].
3. **Source adapter.** `read_span` does `std::fs::read(root.join(rel_path))`, clamps
   `end` to file length (`stale = end > len`), slices, derives 1-based line numbers
   by counting `\n` to `start`/`end`; for `signature` stop at the first line break
   or `{`/`:`; for `context` widen by `context_lines` (default 3). Lossy-UTF8 the
   slice. Errors → `McpError::*` (missing file, bad path) [src: errors.rs].
4. **Handler.** Resolve `symbol` via `cat.by_name`/`find_symbol` (use `file` to
   disambiguate when several match; else first + note), fetch `SymbolMeta` span +
   path via `meta_of`/`path_of`, call `read_span`, attach `cat.revision`
   [src: crates/ariadne-mcp/src/catalog.rs:78-81,164-184].
5. **Register.** Add the `#[tool]` method on `AriadneServer` with a trigger-phrase
   description ("read a symbol's source without reading the whole file"), delegating
   one-line [src: server.rs:186-209].
6. **Snapshots.** Re-run `tests/handshake.rs` and accept the regenerated
   `handshake__tools_list.snap` + `handshake__tools_descriptions.snap`
   [src: crates/ariadne-mcp/tests/snapshots/handshake__tools_list.snap].
</steps>

<verification>
- `cargo nextest run -p ariadne-mcp -E 'test(read_symbol)'` — slice-equality,
  signature, context, and stale cases green.
- `cargo nextest run -p ariadne-mcp -E 'test(handshake)'` — passes with the accepted
  tool-list/descriptions snapshots now including `read_symbol`.
- `cargo clippy -p ariadne-mcp --all-targets -- -D warnings`; `cargo fmt --check`;
  `cargo test --test architecture` — `read_symbol.rs` does no `std::fs`; only
  `adapters/source.rs` does (IO-under-adapters invariant holds).
- Real run: launch the server, `read_symbol` for an existing function in `full` and
  `signature` modes; confirm the returned bytes match the file. Report it; if not
  runnable in-session, say so.
</verification>

<rollback>
Remove `read_symbol.rs`, `adapters/source.rs`, the `mod.rs` lines, the types, the
server method, and the test. Additive tier — nothing else reverts.
</rollback>
