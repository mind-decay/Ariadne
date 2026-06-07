---
tier_id: tier-01
title: Pure code-skeleton assembler use case in ariadne-graph (bytes + spans → folded source)
deps: []
exit_criteria:
  - "`ariadne-graph` exposes a pure `outline::assemble(req) -> Outline` (no IO, no model) returning folded-source text + a symbol index; re-exported from the crate façade."
  - "Golden-snapshot unit tests over byte fixtures for rust, typescript, and javascript assert: bodies folded to a marker carrying the exact elided-line count, signatures + leading doc comments kept byte-faithful, nesting by span containment, and `include_private=false` drops non-public symbols."
  - "Skeleton byte length is strictly less than the input byte length for every multi-symbol fixture; `elided_lines + kept_lines` accounts for every source line."
  - "clippy `-D warnings`, fmt, `cargo test --test architecture` (no new cross-crate edge), and `cargo nextest run -p ariadne-graph` are green."
status: completed
completed: 2026-06-07
---

<context>
The capability's core is a pure function — no filesystem, no graph query, no model
— so it lives in the domain (`ariadne-graph`) beside `docgen`/`api_surface` and is
reused by both driving adapters (tiers 02, 03). Placing it here is what lets the
CLI subcommand exist without a banned `ariadne-cli`→`ariadne-mcp` edge [src:
plan.md D3; CLAUDE.md D13]. Input is byte-faithful: callers hand in the file
bytes and the file's ordered symbol spans; the assembler slices signatures + doc
comments from those bytes and folds bodies [src: plan.md D4]. Doc comments and
nesting are derived deterministically, no symbol-record/parser change [src:
plan.md D5].
</context>

<files>
- `crates/ariadne-graph/src/outline.rs` — new use case: `OutlineRequest`,
  `OutlineSymbol`, `OutlineOptions`, `Outline`, `OutlineEntry`, `assemble`, and
  pure lexical helpers (`signature_end`, line math, doc-comment scan, comment
  syntax per `Lang`).
- `crates/ariadne-graph/src/lib.rs` — re-export the new public types + `assemble`
  from the façade (re-exports only, no logic) [src: CLAUDE.md façade rule].
- `crates/ariadne-graph/tests/outline.rs` — golden-snapshot tests over byte
  fixtures (per language) asserting the rendered skeleton + index.
- `crates/ariadne-graph/tests/fixtures/outline/{sample.rs,sample.ts,sample.js}` —
  small multi-symbol, nested, doc-commented fixtures.
</files>

<steps>
1. **Types.** Define `OutlineSymbol { name: String, kind: String, byte_start: u32,
   byte_end: u32, visibility: Visibility }`; `OutlineRequest { source: Vec<u8>,
   symbols: Vec<OutlineSymbol>, lang: Lang, options: OutlineOptions }`;
   `OutlineOptions { include_private: bool, max_symbols: usize }`; `Outline {
   skeleton: String, symbols: Vec<OutlineEntry>, kept_lines: u32, elided_lines:
   u32 }`; `OutlineEntry { name, kind, line_start, line_end, body_lines, has_body
   }`. Reuse the core `Visibility`/`Lang` types the graph already consumes [src:
   crates/ariadne-core/src/domain/types/lang.rs; api_surface.rs visibility use].
2. **Failing test first.** Write `tests/outline.rs` with a rust fixture (module
   doc + `use` block + a `///`-documented `pub fn` with a multi-line body + an
   `impl` containing two methods + a `priv fn`). Assert the golden skeleton:
   `use` block kept verbatim; doc comments + signatures kept; each body replaced
   by a fold marker `{ … N lines }` with the exact body-line count; methods
   rendered nested under the impl; `include_private=false` omits the `priv fn`.
   Run — fails (no `assemble`).
3. **Lexical helpers.** Port the pure byte logic from the MCP source adapter into
   `outline.rs`: `signature_end` (first `{` on the decl, else trailing `:`/`;`),
   `line_start`/`line_end`/`line_at` [src:
   crates/ariadne-mcp/src/adapters/source.rs:108-170]. Extend `signature_end`
   with a multi-line probe: if no `{`/`:`/`;` on the first line, scan forward to
   the first such terminator (R3). Duplication with `read_symbol`'s copy is
   accepted (different layer); consolidation is out of scope.
4. **Doc-comment scan.** `lang_comment(lang) -> CommentSyntax` maps each `Lang`
   variant to its prefixes: C-family (`//`, `///`, `//!`, `/* */`) for
   rust/typescript/javascript and dialects, `#` for hash-comment languages,
   `None` for unknown → no capture (R1) [src:
   crates/ariadne-core/src/domain/types/lang.rs]. `doc_above(bytes, byte_start,
   syntax)` returns the byte range of contiguous comment lines directly above the
   declaration (stop at a blank or non-comment line).
5. **Nesting.** `parents(symbols)`: a symbol's parent is the nearest other symbol
   whose span strictly contains it (`p.byte_start <= s.byte_start && s.byte_end <=
   p.byte_end`, smallest such span); top-level = no parent. Deterministic;
   ambiguous/overlapping spans fall back to top-level (R2).
6. **Assemble.** Sort symbols by `byte_start` (assert/normalize). Walk top-level
   symbols in source order. For each inter-symbol gap, keep it verbatim when ≤
   `GAP_KEEP_LINES` (e.g. 8) non-blank lines (imports/attrs survive), else emit
   `// … N lines elided`. Per symbol: emit `doc_above` + the signature slice;
   then — if it has children, open the body, recurse children at their own source
   indentation, close; else if the body spans more than `INLINE_LINES` (e.g. 2),
   emit the fold marker with `body_lines = line_at(byte_end-1) -
   line_at(signature_end)`; else emit the full span verbatim (short consts/types).
   Preserve original leading whitespace (slice whole lines). Accumulate
   `kept_lines`/`elided_lines`; honour `include_private` and `max_symbols` (cap +
   note the cap in the skeleton tail, never silently truncate, R4) [src:
   https://www.anthropic.com/engineering/writing-tools-for-agents no-silent-caps].
7. **Façade.** Re-export the public types + `assemble` from `lib.rs`.
8. **Re-run** the language fixtures (ts uses `/** */` + `export`; js plain). Make
   them green. Assert `skeleton.len() < source.len()` and full line accounting.
</steps>

<verification>
- `cargo nextest run -p ariadne-graph` — golden snapshots match for rust/ts/js;
  private filter, nesting, fold counts, and line accounting asserted.
- `cargo test --test architecture` — `ariadne-graph` gains no in-workspace dep
  (pure domain; depends only on `ariadne-core`) [src: CLAUDE.md invariants].
- clippy `-D warnings`; `cargo fmt --all --check`; `RUSTDOCFLAGS=-D warnings cargo
  doc -p ariadne-graph --no-deps` (public items documented).
- Determinism: run the assembler twice on a fixture → byte-identical output.
</verification>

<rollback>
Delete `outline.rs`, its façade re-exports, the test, and the fixtures. Pure
addition in one domain crate; no adapter, config, or schema touched, so nothing
downstream depends on it yet.
</rollback>
