---
tier_id: tier-03
title: ariadne outline <path> CLI subcommand — terminal/script parity for the skeleton
deps: [tier-01]
exit_criteria:
  - "`ariadne outline <path> [--include-private] [--json]` builds/queries the catalog through the existing command plumbing, enumerates the file's symbols, reads its bytes, and renders the tier-01 folded skeleton (text by default, JSON with `--json`)."
  - "A CLI integration test runs the subcommand over a fixture and asserts the output folds bodies + keeps signatures and is byte-smaller than the file; output is produced by the same `outline::assemble` the MCP tool uses (shared use case = parity)."
  - "Unknown path → a typed non-zero exit with a clear message, not a panic; zero-symbol file → the line-count note, not a dump."
  - "clippy `-D warnings`, fmt, `cargo test --test architecture` (no driving→driving edge added), and `cargo nextest run -p ariadne-cli` green."
status: completed
completed: 2026-06-07
---

<context>
CLI parity (plan.md D7) lets the skeleton serve terminals and scripts, not only
the agent loop. It composes the same domain use case as the MCP tool, so the two
surfaces cannot drift. The subcommand follows the `digest` command shape — a
`commands/<name>.rs` runner registered in `commands/mod.rs` and dispatched from
`main.rs` — and reuses the catalog/graph access `digest`/`query` already use
[src: crates/ariadne-cli/src/commands/digest.rs; crates/ariadne-cli — query
path]. `ariadne-cli` already depends on `ariadne-graph` (digest composes graph
use cases) and may use `anyhow` + `std::fs` (it is a driving adapter) [src:
CLAUDE.md `thiserror`/`anyhow` rule; plan.md architecture].
</context>

<files>
- `crates/ariadne-cli/src/main.rs` — add an `Outline { path: PathBuf, #[arg(long)]
  include_private: bool, #[arg(long)] json: bool }` variant to the `Commands`
  `#[derive(Subcommand)]` enum + a dispatch arm [src:
  https://docs.rs/clap/latest/clap/_derive/_tutorial/index.html].
- `crates/ariadne-cli/src/commands/outline.rs` — `run(path, include_private,
  json)`: obtain the catalog, enumerate the target file's symbols, read bytes,
  call `ariadne_graph::outline::assemble`, render.
- `crates/ariadne-cli/src/commands/mod.rs` — register the `outline` module [src:
  crates/ariadne-cli/src/commands — digest registration].
- `crates/ariadne-cli/tests/outline_cli.rs` — `assert_cmd` integration test over
  a fixture project (the pattern the other CLI command tests use).
</files>

<steps>
1. **Failing test first.** In `outline_cli.rs`, set up a temp project with a
   multi-symbol source file, run `ariadne outline <file>` via `assert_cmd`, and
   assert stdout contains each signature, a fold marker per body, and is shorter
   than the file. Add `--include-private` (private symbol appears) and a
   missing-path case (non-zero exit, message). Run — fails (no subcommand).
2. **Subcommand.** Add the `Outline` variant + dispatch in `main.rs`, mirroring
   how `Digest` is wired [src: crates/ariadne-cli/src/main.rs Commands enum;
   commands/digest.rs].
3. **Runner.** In `commands/outline.rs`: get the catalog the same way `digest`
   does; resolve the path → file id; collect the file's symbols (`OutlineSymbol`)
   filtered by file and sorted by `byte_start` — the `file_summary` enumeration
   [src: crates/ariadne-mcp/src/tools/file_summary.rs:25-102 (pattern; the CLI
   reads from the daemon catalog mirror at
   crates/ariadne-daemon/src/domain/catalog.rs)]; read bytes via `std::fs`; build
   `OutlineRequest` and call `assemble`. Zero symbols → print the note. Map IO /
   resolution failures to a typed `anyhow` error → non-zero exit.
4. **Render.** Default: print `outline.skeleton` to stdout. `--json`: serialize
   the full `Outline` (skeleton + index + counts) with `serde_json` [src:
   plan.md no-new-deps; serde_json in tree].
5. **Help text.** One-line about/long-about describing the token-cheap skeleton +
   the `read_symbol`/native-`Read` expansion path.
</steps>

<verification>
- `cargo nextest run -p ariadne-cli` — the subcommand test, `--include-private`,
  `--json`, and the missing-path case pass.
- `cargo test --test architecture` — `ariadne-cli` gains no dependency on another
  driving adapter; it reaches the assembler through `ariadne-graph` only [src:
  CLAUDE.md D13; plan.md D3].
- clippy `-D warnings`; `cargo fmt --all --check`.
- Real run: `cargo run -p ariadne-cli -- outline crates/ariadne-graph/src/outline.rs`
  → prints the folded skeleton; confirm it matches the MCP tool's skeleton for the
  same file (shared use case). Report the byte delta vs `wc -c` of the file.
</verification>

<rollback>
Remove the `Outline` variant + dispatch, `commands/outline.rs`, its mod
registration, and the test. Tiers 01–02 are untouched.
</rollback>
