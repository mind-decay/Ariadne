---
tier_id: tier-16
title: ariadne setup — one-shot project onboarding: config + .mcp.json merge + CLAUDE.md discoverability block
deps: [tier-15]
exit_criteria:
  - "`ariadne setup [root]` exists as an 8th `Cmd` variant; one invocation runs the `init` config scaffolding, writes/merges `<root>/.mcp.json`, and writes/refreshes a marker-delimited Ariadne block in `<root>/CLAUDE.md`. It does NOT run an index."
  - "`.mcp.json` merge is non-destructive: any pre-existing foreign `mcpServers` entry survives; the `ariadne` entry is inserted/replaced with `command` = the absolute current-exe path and `args = [\"serve\", \"--watch\"]`, `env = {}`. A failing-first CLI test covers the foreign-entry-preserved case."
  - "The CLAUDE.md block is delimited by `<!-- BEGIN ARIADNE -->` / `<!-- END ARIADNE -->`; a re-run replaces the block in place — never duplicates it — and leaves every byte outside the markers untouched. A failing-first CLI test covers re-run idempotency on a pre-populated CLAUDE.md."
  - "`setup` is fully idempotent: a second consecutive run leaves `.ariadne/config.toml`, `.mcp.json`, and `CLAUDE.md` byte-identical. The CLI test asserts this."
  - "README quickstart documents `ariadne setup` as the one-command onboarding path, ahead of `ariadne index`."
  - "`cargo build --workspace`, `clippy -D warnings`, `fmt --check`, `cargo test --test architecture`, `cargo nextest run --workspace`, `RUSTDOCFLAGS=-D warnings cargo doc` all green."
status: pending
---

<context>
Post-v1 onboarding tier. Today a consumer wires Ariadne into a project by
hand: run `ariadne init` (scaffolds `.ariadne/config.toml` + gitignores
`.ariadne/` [src: crates/ariadne-cli/src/commands/init.rs:14-38]), then
hand-author a `.mcp.json`, then hand-edit `CLAUDE.md` so the agent knows
the tools exist. tier-15 sharpens the in-binary discoverability surfaces;
this tier delivers the out-of-binary surface — the strongest lever, since
a CLAUDE.md block lands in agent context every session — and removes the
manual steps.

`ariadne setup` is a new subcommand. The fixed surface was 7 subcommands
[src: tier-10-cli-e2e.md D-E], so this is a deliberate surface expansion:
`init` stays the low-level primitive (scaffold `.ariadne/`); `setup` is the
one-shot onboarding UX (config + MCP wiring + discoverability) and reuses
`init`'s logic rather than duplicating it. Rejected: a `init --mcp` flag —
`setup` does strictly more than init and reads as a distinct verb;
overloading `init` with flags hides that.

Scope: `ariadne-cli` only (the composition root
[src: docs/adr/0007-cli-composition-root.md]) — a new subcommand + one
command module + tests + a README edit. `serde_json` is added to
`crates/ariadne-cli/Cargo.toml`: it is already a workspace dependency
(`ariadne-mcp` uses it for tool JSON [src: crates/ariadne-mcp/src/server.rs:237])
— a manifest addition, not a new technology, so no ADR is owed. No new
crate, no port, no cross-crate edge, no on-disk schema. `setup` runs no
index — indexing stays an explicit `ariadne index` step. Full context:
plan.md.
</context>

<files>
- crates/ariadne-cli/Cargo.toml — add the `serde_json` workspace dependency.
- crates/ariadne-cli/src/main.rs — add `Setup { root }` to the `Cmd` enum
  [src: main.rs:30-89] and a dispatch arm in `run` [src: main.rs:105-119].
- crates/ariadne-cli/src/commands/mod.rs — add `pub mod setup;`.
- crates/ariadne-cli/src/commands/setup.rs — NEW: orchestrates `init` reuse,
  `.mcp.json` merge, and the CLAUDE.md marker block.
- crates/ariadne-cli/tests/setup.rs — NEW: integration test driving the
  `ariadne` binary via `CARGO_BIN_EXE_ariadne`.
- README.md — quickstart updated to lead with `ariadne setup`.
</files>

<spec>
**`.mcp.json` entry** — Claude Code project-scoped MCP config: project-root
file, `mcpServers` object, each server `{command, args, env}`
[src: https://code.claude.com/docs/en/mcp; concrete example: the repo's own
`.mcp.json`]. `setup` owns the `ariadne` key only:
```json
{ "mcpServers": { "ariadne": {
  "command": "<absolute path to the running ariadne binary>",
  "args": ["serve", "--watch"], "env": {} } } }
```
`command` is the absolute `std::env::current_exe()` path, not the bare
string `"ariadne"` — robust when `ariadne` is not on `PATH`. `--watch`
keeps the in-process watcher live so the index stays fresh during a
session [src: crates/ariadne-cli/src/commands/serve.rs:21-40].

**CLAUDE.md block** — between `<!-- BEGIN ARIADNE -->` and
`<!-- END ARIADNE -->` (HTML comments: invisible when rendered, trivially
located). ≤25 lines. Content: states the Ariadne MCP server is configured;
instructs the agent to prefer the Ariadne MCP tools over grep/Read for
symbol / reference / impact / architecture questions; lists the tool
families with one trigger phrase each. Wording is kept consistent with the
tier-15 server-instructions rewrite (the `deps` reason).
</spec>

<steps>
1. **Failing test first.** In `crates/ariadne-cli/tests/setup.rs` drive
   `Command::new(env!("CARGO_BIN_EXE_ariadne")).arg("setup").arg(tmp)`
   (confirm the bin name is `ariadne` from the cli `Cargo.toml` `[[bin]]`).
   Assert: `.ariadne/config.toml`, `.mcp.json`, `CLAUDE.md` all exist;
   `.mcp.json` parses and `mcpServers.ariadne.args == ["serve","--watch"]`;
   `CLAUDE.md` contains both markers exactly once. Pre-seed cases: a
   `.mcp.json` holding a foreign server + a `CLAUDE.md` with user prose —
   assert both survive. Run `setup` twice — assert all three files
   byte-identical after the second run. MUST FAIL first: no `setup`
   subcommand exists.

2. **Subcommand wiring.** Add `Setup { root: PathBuf }` to `Cmd`
   [src: main.rs:30-89] with a doc comment, and the arm
   `Cmd::Setup { root } => commands::setup::run(&root).map(|()| true)` in
   `run` [src: main.rs:105-119]. Add `pub mod setup;` to
   `commands/mod.rs`.

3. **`setup.rs` — step A (config).** `run(root)` first calls
   `crate::commands::init::run(root)` — reuse, do not duplicate; `init` is
   already idempotent [src: init.rs:14-57].

4. **step B (`.mcp.json` merge).** Read `<root>/.mcp.json` if present and
   parse to `serde_json::Value`, else start from `json!({})`. Ensure a
   `mcpServers` object, then insert/replace only the `ariadne` key with the
   `<spec>` entry (`command` from `std::env::current_exe()?` made
   absolute). Serialize pretty + trailing newline; write. Foreign server
   keys are never read-modified beyond being carried through verbatim.

5. **step C (CLAUDE.md block).** Read `<root>/CLAUDE.md` if present, else
   empty. If both markers are found, replace the span between them
   (inclusive) with the freshly rendered `<spec>` block; otherwise append
   the block, prefixed by a blank line if the file is non-empty. Write.
   Re-running thus refreshes the block in place — never appends a second.

6. **step D (report).** Print what was written (`.ariadne/`, `.mcp.json`,
   the CLAUDE.md block) and the next step: `run \`ariadne index\` to build
   the index`.

7. **README.** Update the quickstart so onboarding is `ariadne setup`
   (writes config + `.mcp.json` + CLAUDE.md) → `ariadne index`, replacing
   the manual `init` + hand-authored `mcp.json` instructions
   [src: tier-10-cli-e2e.md step 13].

8. **Verify.** Run the full gate (`<verification>`).
</steps>

<verification>
- `cargo nextest run -p ariadne-cli` — green: the step-1 test passes;
  foreign `.mcp.json` entry preserved, user CLAUDE.md prose preserved,
  second run byte-identical (idempotency).
- `cargo nextest run --workspace` — green: no other crate is touched.
- `cargo test --test architecture` — green: `setup.rs` lives in
  `ariadne-cli` (composition root); `serde_json` is external, so no new
  in-workspace cross-crate edge.
- `cargo build --workspace`, `cargo clippy --workspace --all-targets
  --all-features -- -D warnings`, `cargo fmt --all --check`,
  `RUSTDOCFLAGS=-D warnings cargo doc --workspace --no-deps
  --document-private-items` — clean.
- Real run, recorded in the tier-16 audit: `cargo run -p ariadne-cli --
  setup <tempdir>` on a scratch repo; inspect the three artifacts by eye;
  run it a second time and confirm no diff. A non-idempotent result is
  root-caused, not silenced.
</verification>

<rollback>
`git revert` the `setup.rs` module, the `commands/mod.rs` line, the
`Cmd::Setup` variant + dispatch arm, the `Cargo.toml` `serde_json` line,
`tests/setup.rs`, and the README quickstart edit. `setup` writes only its
own `.mcp.json` `ariadne` key and its own marker-delimited CLAUDE.md block
in consumer repos — both are caller-owned files; reverting the tier leaves
any already-written `.mcp.json` / CLAUDE.md valid and editable by hand. No
on-disk format, no `SCHEMA_VERSION`, no migration.
</rollback>
