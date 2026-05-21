---
tier_id: tier-16
audited: 2026-05-21
verdict: PASS
commit: c79f6ce17a3fa38e67b6b125a27c9b7914a2070f
---

<scope>
tier-16-setup-command: `ariadne setup` — one-shot project onboarding
(config scaffold + `.mcp.json` merge + `CLAUDE.md` discoverability block).
Scoped diff (tier `<files>`):
- crates/ariadne-cli/Cargo.toml — no change needed; `serde_json` already in
  `[dependencies]` (line 42). The tier `<files>` premise that it was absent
  was stale; the correct end-state (dep available to `setup.rs`) holds.
- crates/ariadne-cli/src/main.rs — `Cmd::Setup` variant + dispatch arm + doc
  comment bumped "seven" → "eight".
- crates/ariadne-cli/src/commands/mod.rs — `pub mod setup;`.
- crates/ariadne-cli/src/commands/setup.rs — NEW, 141 lines.
- crates/ariadne-cli/tests/setup.rs — NEW, 139 lines, 4 integration tests.
- README.md — quickstart + Claude Code integration + Commands table.
Out-of-scope files in `git status` (server.rs, handshake.rs, snapshots,
tier-15 report) belong to the tier-15 audit and were not reviewed here.
</scope>

<checks_run>
- `cargo build --workspace` — green.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` — green.
- `cargo fmt --all --check` — green (no output).
- `cargo test --test architecture` — green: `architecture_invariants_hold`
  passes; `setup.rs` sits in the `ariadne-cli` composition root, `serde_json`
  is external — no new in-workspace cross-crate edge.
- `cargo nextest run -p ariadne-cli` — 5/5 pass, incl. all 4 `setup` tests.
- `cargo nextest run --workspace` — 139 pass, 9 skipped; no regression.
- `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
  --document-private-items` — clean.
- Real run: `cargo run -p ariadne-cli -- setup /tmp/ariadne-setup-audit` on a
  scratch dir pre-seeded with a foreign `.mcp.json` server + user CLAUDE.md
  prose. Inspected by eye: `foreign` server carried verbatim; `ariadne` entry
  inserted with `command` = absolute `target/debug/ariadne` path,
  `args=["serve","--watch"]`, `env={}`; CLAUDE.md prose preserved with the
  marker block appended after a blank line; `.ariadne/config.toml` +
  `.gitignore` scaffolded. No index ran. Second run: `shasum` of all four
  artifacts byte-identical to the first run.
- Re-read `init.rs` (reuse target), `main.rs`, `mod.rs`, both new files,
  README diff end-to-end.
</checks_run>

<findings>
| id | category | severity | location | problem | fix |
|---|---|---|---|---|---|
| F1 | correctness | INFO | crates/ariadne-cli/src/commands/setup.rs:87 | `write_claude_block` reads `CLAUDE.md` with `unwrap_or_default()`, swallowing every IO error (not just `NotFound`); a non-`NotFound` read failure on an existing caller-owned file followed by a successful write silently clobbers it — `merge_mcp_json` (lines 47-53) deliberately distinguishes `NotFound` from other errors, so the two functions are asymmetric. | Match `merge_mcp_json`: treat only `ErrorKind::NotFound` as empty, propagate other read errors. Non-blocking — narrow scenario and the existing `init.rs:43` `ensure_gitignored` uses the same `unwrap_or_default()` idiom. |
</findings>

<verdict>
PASS. Zero FAIL findings; one INFO.

All six `exit_criteria` independently verified:
1. `Cmd::Setup` is the 8th variant; one invocation runs `init` scaffolding,
   merges `.mcp.json`, refreshes the `CLAUDE.md` block, runs no index —
   confirmed by the real run (no `index.redb` written).
2. `.mcp.json` merge is non-destructive: the `foreign` server survived
   verbatim; `ariadne` carries `command` = absolute `current_exe()` path,
   `args=["serve","--watch"]`, `env={}`. Covered by
   `setup_preserves_foreign_mcp_entry`.
3. `CLAUDE.md` block is marker-delimited; a re-run replaces it in place
   (`existing[..begin] + block + existing[end..]`) — never duplicates; bytes
   outside the markers are sliced through verbatim. Covered by
   `setup_preserves_user_claude_prose_and_refreshes_block_in_place` (runs
   `setup` twice on pre-populated prose).
4. Idempotent: `setup_is_byte_idempotent` asserts all three artifacts
   byte-identical after a second run; the real-run `shasum` pass confirms it
   on disk.
5. README quickstart leads with `ariadne setup` ahead of `ariadne index`;
   Commands table gains the `ariadne setup [root]` row.
6. build / clippy / fmt / architecture / nextest / doc all re-run green.

`serde_json` (used by `setup.rs` and `tests/setup.rs`) was already a
`[dependencies]` entry, so the tier `<files>` line to add it was a no-op —
the implementer correctly avoided a duplicate. The marker block is 22 lines
(spec cap ≤25), wording tracks the tier-15 server-instructions rewrite, and
every tool name (`list_symbols` … `project_status`) matches a live MCP tool.
Idempotency holds because `serde_json`'s default `Map` is a sorted `BTreeMap`
(observed: `args`/`command`/`env` and `ariadne`/`foreign` emitted sorted).
</verdict>

<next_steps>
None blocking. Optional: address F1 by reading `CLAUDE.md` with the same
`ErrorKind::NotFound`-vs-other discrimination `merge_mcp_json` already uses,
in a follow-up. Tier-16 is shippable as-is.
</next_steps>

<sources>
- tier-16-setup-command.md `<exit_criteria>`, `<spec>`, `<steps>`, `<verification>`.
- plan.md D13 (hexagonal boundary), ADR-0007 (cli composition root).
- crates/ariadne-cli/src/commands/init.rs:14-57 (reuse target, `unwrap_or_default` precedent at :43).
- serde_json `Map` default = `BTreeMap` (sorted keys): https://docs.rs/serde_json/latest/serde_json/struct.Map.html
- Claude Code project `.mcp.json` schema: https://code.claude.com/docs/en/mcp
- Reviewer standard: https://google.github.io/eng-practices/review/reviewer/standard.html
</sources>
