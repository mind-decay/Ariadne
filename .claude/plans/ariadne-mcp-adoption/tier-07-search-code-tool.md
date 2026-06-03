---
tier_id: tier-07
title: search_code MCP tool — regex/glob/kind/lang/visibility search over the catalog
deps: [tier-06]
exit_criteria:
  - "A new `search_code` tool is registered on `AriadneServer` and appears in the rmcp `list_tools` output alongside the existing 17 tools; the `handshake__tools_list.snap` + `handshake__tools_descriptions.snap` insta snapshots are updated to include it."
  - "`tests/tools_search_code.rs` (written failing first) passes: substring (default) and `regex:true` name match, `path` glob filter, `kind`/`lang`/`visibility` filters, and `limit` all return the expected symbol sets on a fixture catalog."
  - "Invalid regex or invalid glob returns a typed `McpError`/`ErrorData`, never a panic; the regex is compiled with `size_limit` + `nest_limit` set."
  - "A criterion bench asserts search latency stays well under the 100ms query p95 SLO on a large synthetic catalog (R6)."
  - "`cargo nextest run -p ariadne-mcp`, clippy `-D warnings`, fmt, and `cargo test --test architecture` are green."
status: completed
completed: 2026-06-03
---

<context>
The spike (tier-06) cleared the ≥40% gate, so build the production search
primitive. `search_code` generalises `list_symbols` (name-substring only) to
regex name match plus `path` glob, `kind`, `lang`, `visibility`, and `limit`,
returning a ranked `SymbolSummary` list. It is a pure read projection over the
in-RAM `Catalog`, the same shape as the existing tool handlers — no new domain
port [src: plan.md D8; crates/ariadne-mcp/src/catalog.rs:71-185; tools/list_symbols.rs:12-32]. `regex`
1.12.3 is linear-time with `RegexBuilder.size_limit`/`nest_limit` bounding
resources (no ReDoS); `glob` 0.3.3 `Pattern::matches_path` handles `**` [src:
https://docs.rs/regex/1.12.3/regex/struct.RegexBuilder.html;
https://docs.rs/glob/0.3.3/glob/struct.Pattern.html].
</context>

<files>
- `crates/ariadne-mcp/Cargo.toml` — add `regex = "1.12.3"`, `glob = "0.3.3"` (D10).
- `crates/ariadne-mcp/src/types.rs` — `SearchCodeInput` (mirrors `ListSymbolsInput`
  derive set + `#[serde(default)]` optionals) [src: types.rs:70].
- `crates/ariadne-mcp/src/tools/search_code.rs` — new `handle(&Catalog, &input)`.
- `crates/ariadne-mcp/src/tools/mod.rs` — `pub mod search_code;`.
- `crates/ariadne-mcp/src/server.rs` — `#[tool]` method delegating one-line, like
  `list_symbols` [src: server.rs:186-209].
- `crates/ariadne-mcp/tests/tools_search_code.rs` — behaviour tests.
- `crates/ariadne-mcp/tests/snapshots/` — accept updated `handshake__tools_list.snap`
  + `handshake__tools_descriptions.snap` (the new tool changes them).
- `crates/ariadne-mcp/benches/` — search latency bench (extend or new).
</files>

<steps>
1. **Failing test.** In `tools_search_code.rs`, build a fixture `Catalog` (reuse the
   test support helper) with symbols of known names/kinds/langs/paths. Assert:
   substring default matches; `regex:"^handle"` matches only those; `path:"src/**/
   *.rs"` filters by file; `kind`/`visibility` narrow; `limit` caps the count;
   invalid regex → `Err`. Run — fails (tool absent) [src: tests/support.rs].
2. **Deps.** Add `regex`/`glob` to `ariadne-mcp/Cargo.toml` (already transitive via
   `ignore`, pure-Rust) [src: Cargo.lock; plan.md D10].
3. **Input type.** `SearchCodeInput { query: String, #[serde(default)] regex: bool,
   path: Option<String>, kind: Option<String>, lang: Option<String>, visibility:
   Option<String>, limit: Option<u32> }`, deriving `Deserialize, Serialize,
   JsonSchema` [src: types.rs:70].
4. **Handler.** Compile the name matcher once: substring (lowercase `contains`,
   like `list_symbols`) by default, else `RegexBuilder::new(&q).case_insensitive
   (true).size_limit(1<<20).nest_limit(64).build()?`; compile `glob::Pattern::new
   (path)?` when `path` is set. Iterate `cat.symbols`, apply name + path + kind +
   lang + visibility filters, rank exact > prefix > substring/regex then by name,
   early-exit at `limit`, map via `summarize` [src: crates/ariadne-mcp/src/tools/mod.rs:34].
5. **Errors.** Map regex/glob compile failures to `McpError::*` → `ErrorData`; never
   `unwrap` on caller input [src: errors.rs; server.rs error mapping].
6. **Register.** Add the `#[tool(...)]` method on `AriadneServer` with a
   trigger-phrase description ("search the codebase by symbol name pattern / path /
   kind"), delegating to `tools::search_code::handle` [src: server.rs:186-209].
7. **Snapshots.** Re-run `tests/handshake.rs` and accept the regenerated
   `handshake__tools_list.snap` + `handshake__tools_descriptions.snap` (tool count
   17 → 18) [src: crates/ariadne-mcp/tests/snapshots/handshake__tools_list.snap].
8. **Bench.** Criterion case over a synthetic ~100k-symbol catalog asserting median
   search time leaves ample headroom under the 100ms query p95 SLO (R6).
</steps>

<verification>
- `cargo nextest run -p ariadne-mcp -E 'test(search_code)'` — all cases green.
- `cargo nextest run -p ariadne-mcp -E 'test(handshake)'` — passes with the accepted
  snapshots (tool list + descriptions now include `search_code`).
- `cargo bench -p ariadne-mcp --no-run` builds; the search bench reports < SLO.
- `cargo clippy -p ariadne-mcp --all-targets -- -D warnings`; `cargo fmt --check`;
  `cargo test --test architecture` (no leaked `regex`/`glob` types in the public
  API — handler returns `SymbolSummary` only).
- Real run: launch the stdio server, call `search_code` with `regex:"^handle"` and
  a `path` glob; confirm structured hits. Report the call + output; if not runnable
  in-session, say so.
</verification>

<rollback>
Remove `search_code.rs`, the `mod.rs` line, `SearchCodeInput`, the server method,
the test, the bench, and the two `Cargo.toml` deps. Additive tier — no existing
tool changes, so nothing else reverts.
</rollback>
