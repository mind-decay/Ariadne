---
tier_id: tier-01
audited: 2026-06-03
verdict: PASS
commit: 0909c2e532078bbed3cbe06338cb6f36cd260679
---

<scope>
Tier-01 "Force Ariadne tool visibility" — `alwaysLoad` in `.mcp.json` (D1),
per-tool `_meta {"anthropic/alwaysLoad": true}` in the MCP server (D2), and a
tightened ≤2KB `with_instructions`. Diff audited (uncommitted working tree on
`main`, HEAD 0909c2e), scoped to the tier's `<files>`:
- `crates/ariadne-cli/src/commands/setup.rs` — `merge_mcp_json` adds `alwaysLoad`.
- `crates/ariadne-cli/tests/setup.rs` — new `alwaysLoad` + foreign-survives test.
- `crates/ariadne-mcp/src/server.rs` — `meta = always_load_meta()` on all tools;
  `always_load_meta()` helper; rewritten instructions.
- `crates/ariadne-mcp/tests/handshake.rs` — new per-tool `_meta` contract test.
- `crates/ariadne-mcp/tests/snapshots/handshake__server_instructions.snap` —
  updated instructions snapshot.
- `.mcp.json` — dogfood: `alwaysLoad: true` written into the `ariadne` entry.
Out of scope and untouched: `.claude/plans/useful-docgen/` (untracked, foreign
plan). Tier file itself: only `status: pending → completed` + `completed:` flip
(build record, not code). No `Cargo.toml`/`Cargo.lock` change.
</scope>

<checks_run>
- plan_adherence: only the four `<files>` entries touched, as intended; `.mcp.json`
  + tier-file status are expected dogfood/build records. No out-of-scope code.
- Re-ran the tier `<verification>` in full:
  - `cargo nextest run -p ariadne-cli -p ariadne-mcp` → 83 passed, 0 skipped
    (1 leaky, the spawned MCP client subprocess; not a failure).
  - `setup_writes_always_load_into_ariadne_entry` → PASS (isolated re-run).
  - `handshake_tools_carry_always_load_meta` → PASS (over-the-wire `_meta` check).
  - `handshake_snapshots_server_instructions` / `..._tool_descriptions` → PASS.
  - `cargo clippy --workspace --all-targets --all-features -- -D warnings` → clean.
  - `cargo fmt --all --check` → exit 0.
  - `cargo test --test architecture` → 1 passed (hexagonal invariants hold).
- exit_criteria: all four verified independently (see below).
- versions: rmcp = rmcp-macros = `=1.7.0` (Cargo.toml + Cargo.lock); no new dep.
- code intelligence: `project_status` rev 316 (fresh, 359 files / 3336 symbols);
  index current, graph trusted.
</checks_run>

<exit_criteria_verification>
1. `setup` writes `alwaysLoad: true` idempotently, foreign untouched — setup.rs:65-80
   inserts/replaces only the `ariadne` key via `.insert("ariadne", entry)`; test
   asserts foreign `other` survives verbatim with no `alwaysLoad` added. ✅
2. `_meta {"anthropic/alwaysLoad": true}` on every tool (handshake test) — all 17
   `#[tool]`s carry `meta = always_load_meta()`; `handshake_tools_carry_always_load_meta`
   iterates all `EXPECTED_TOOLS=17` and asserts each tool's wire `_meta`. ✅
   (Criterion text says "13"; see INFO-1 — implementation covers all 17, stronger.)
3. `with_instructions` ≤2KB, frames when-to-search, descriptions unchanged — body
   is 931 bytes (≪2048); leads "For any question about symbols… search for and
   call these Ariadne tools instead of grep, Read"; tool-description snapshot
   unchanged (only a trailing `,` added outside each string literal before the
   `meta` arg → description text byte-identical, snapshot test green). ✅
4. This repo's `.mcp.json` carries `alwaysLoad`; fresh session loads tools without
   ToolSearch — `.mcp.json:4` has `"alwaysLoad": true`. Empirically confirmed in
   this audit session: `mcp__ariadne__project_status` was callable directly with
   no preceding `ToolSearch`, and the deferred-tools list excludes every
   `mcp__ariadne__*` tool (lists Context7/Figma/devtools only). ✅
</exit_criteria_verification>

<findings>
| id | category | severity | location | problem | fix |
|----|----------|----------|----------|---------|-----|
| INFO-1 | docs | INFO | tier-01-always-load-visibility.md:7 (+ title) | Exit criterion 2 and the title say "13 tools"; there are now 17 (tiers 13–15 added hotspots/complexity/co_change/diff_blast_radius). Implementation correctly covers all 17 (`EXPECTED_TOOLS=17`), so it exceeds the criterion — only the stale count drifts. | Update the tier's "13" to "every tool" or "17" for accuracy; non-blocking. |
</findings>

<verdict>
PASS. Zero FAIL findings. All four exit criteria independently verified; the full
tier `<verification>` re-runs green; D1 (setup `alwaysLoad`) and D2 (per-tool
`_meta` via the rmcp-macros 1.7 `meta` attribute) are both implemented and proven
at runtime over the MCP wire — the D2 spike resolved without needing a `list_tools`
override. Instructions are 931 B (≪2KB) and lead with the search trigger; tool
descriptions are byte-identical (snapshot locked). No smuggled dependency, no
out-of-scope edit, hexagonal boundary preserved (architecture test green). The
single INFO is documentation drift in the tier's own criterion text, not a code
defect.
</verdict>

<next_steps>
None required to ship tier-01. Optional housekeeping: correct the stale "13 tools"
wording (INFO-1) when next editing the plan. Proceed to tier-02 (digest command)
per the plan's tier order.
</next_steps>

<sources>
- [Connect Claude Code to tools via MCP](https://code.claude.com/docs/en/mcp) — `alwaysLoad`, Tool Search deferral, `anthropic/alwaysLoad`, 2KB instruction truncation.
- [rmcp Tool struct — docs.rs 1.7.0](https://docs.rs/rmcp/1.7.0/rmcp/model/struct.Tool.html) — `meta: Option<Meta>`.
- [Google eng-practices — reviewer standard](https://google.github.io/eng-practices/review/reviewer/standard.html) — code-health-over-perfection gate.
</sources>
