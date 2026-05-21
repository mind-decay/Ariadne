---
tier_id: tier-15
audited: 2026-05-21
verdict: PASS
commit: c79f6ce17a3fa38e67b6b125a27c9b7914a2070f
---

<scope>
Tier-15 — MCP tool discoverability. Rewrites all 13 `#[tool(description=...)]`
strings and the `with_instructions(...)` argument in `crates/ariadne-mcp/src/server.rs`,
and adds discoverability coverage to `crates/ariadne-mcp/tests/handshake.rs` plus
two new `.snap` files.

Diff audited (scoped to the tier's `<files>`):
- `crates/ariadne-mcp/src/server.rs` — 13 description literals (L85-257) + the
  `with_instructions` argument (L276-287). String literals only; no signature,
  no `Parameters<T>`, no logic touched. Confirmed by reading the full diff.
- `crates/ariadne-mcp/tests/handshake.rs` — 3 new tests + `EXPECTED_TOOLS` const.
- `crates/ariadne-mcp/tests/snapshots/handshake__tools_descriptions.snap` — NEW.
- `crates/ariadne-mcp/tests/snapshots/handshake__server_instructions.snap` — NEW.
Nothing outside `<files>` changed (`git status`: only the four files above plus
the tier file's own `status` flip).
</scope>

<checks_run>
All `<verification>` commands re-run at HEAD c79f6ce with the working-tree diff applied:

| command | result |
|---|---|
| `cargo build --workspace` | green |
| `cargo nextest run -p ariadne-mcp` | green — 22 passed, 0 skipped (incl. all 4 handshake tests) |
| `cargo nextest run --workspace` | green — 135 passed, 9 skipped |
| `cargo test --test architecture` | green — `architecture_invariants_hold` ok |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | green — no warnings |
| `cargo fmt --all --check` | green — no diff |
| `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --document-private-items` | green |

Snapshot checks:
- `handshake__tools_list.snap` — byte-unchanged. `git diff --stat` empty; last
  commit touching it is tier-09 (2de7c0b). `handshake.rs:27` snapshots
  `tool.input_schema` only — no description string can reach it. Exit criterion 4 met.
- `handshake__tools_descriptions.snap` / `handshake__server_instructions.snap` —
  NEW, accepted, and content-reviewed against `<spec>`.

String-budget measurement (script over the accepted snapshots):
- 13 tool descriptions present; longest = 203 chars (`blast_radius`); all ≤320.
- All 13 descriptions contain the literal `Use when ` and ≥2 double-quoted
  trigger phrases drawn from `<trigger_map>`; leading "what" clause matches the
  pre-rewrite wording verbatim.
- Server instructions = 779 chars (≤900); contains `grep` and `Read`; satisfies
  all 4 `<spec>` instructions-contract clauses (identity, grep/Read nudge,
  workflow map naming every tool, Context7-style "even when the answer seems
  known" framing).

Failing-first (exit criterion 3): the pre-rewrite strings in the diff confirm
the property — old descriptions carry no `Use when ` clause and the old
instructions never mention `grep`, so `handshake_descriptions_carry_when_and_triggers`
would fail against tier-08. Verified by inspecting the diff's removed lines.

Manual MCP session (exit criterion 5): a fresh general-purpose agent was given a
neutral, un-primed code-structure task — "What would break if I change the
`apply_writes` function? ... which call sites and files are affected" — with no
mention of Ariadne, grep, or any tool. Index confirmed fresh first
(`project_status`: revision 2, 202 files, 2032 symbols). The agent's reported
tool order was:

  `Bash, Bash, ToolSearch, Read, Read, mcp__ariadne__project_status, Bash,`
  `mcp__ariadne__blast_radius, mcp__ariadne__find_references, Bash, Read, Read, Bash`

The agent reached for the Ariadne graph — `mcp__ariadne__blast_radius` and
`mcp__ariadne__find_references` — as the substantive impact tools, and its
analysis cited those results ("`apply_writes` ... `pub(super)` ... sole call
site `RedbWriteTxn::apply` at mod.rs:141 ... one production consumer
`ariadne-cli/src/domain/mod.rs:614`"). The lazy grep/Read default did not win
the impact analysis. Exit criterion 5 satisfied; transcript excerpt recorded here.
</checks_run>

<findings>
| id | category | severity | location | problem | fix |
|---|---|---|---|---|---|
| INFO-1 | plan_adherence | INFO | `server.rs:276-287`; tier file exit-criterion 1 | Exit-criterion 1 prose lists "explicit trigger phrases" as a component the instructions string must carry; the string carries none (zero quoted phrases — measured). | None required: the plan's own `<spec>` "Server instructions contract" enumerates 4 mandatory clauses (none being trigger phrases) and the implementation satisfies all 4; the criterion prose drifted from the binding `<spec>`. Plan-internal inconsistency, not an implementation defect. |
| INFO-2 | docs | INFO | `server.rs:1` | Module doc says "wiring the 10 Ariadne analytics into MCP"; there are 13. | Update "10" → "13". Pre-existing from tier-08/09, outside tier-15's `<files>` diff scope — noted for cleanup, not introduced by this tier. |
</findings>

<verdict>
PASS. Zero FAIL findings.

Every exit criterion is independently verified:
1. Instructions string carries workflow guidance and an explicit
   prefer-over-`grep`/`Read` nudge naming both tools; snapshotted as
   `server_instructions`. (The criterion's "explicit trigger phrases" sub-clause
   is unmet — see INFO-1 — but is contradicted by the plan's own `<spec>`
   instructions contract, which the implementation follows faithfully; non-gating.)
2. All 13 descriptions follow the `<spec>` template — what + `Use when ` clause +
   ≥2 quoted trigger phrases, longest 203 chars (≤320); snapshotted as
   `tools_descriptions`.
3. `handshake_descriptions_carry_when_and_triggers` asserts `Use when ` on every
   description and `grep` in the instructions; failing-first property confirmed
   against the pre-rewrite strings.
4. `handshake__tools_list.snap` byte-unchanged; the input-schema golden is
   untouched by string edits.
5. Real un-primed agent session chose `blast_radius` + `find_references` over the
   grep/Read default; transcript excerpt recorded above.
6. Full gate (build, clippy, fmt, both nextest runs, architecture, doc) green.

No new dependency, port, signature, ADR, or cross-crate edge — architecture test
green confirms. Tests assert behavior (description/instruction content) with loud
panic messages, not implementation detail.
</verdict>

<next_steps>
None blocking. Tier-15 may be committed; `audit-state.json` records the PASS.
Optional cleanup, at the author's discretion, not gating:
- INFO-2: correct the stale "10" in `server.rs:1` next time that file is touched.
- INFO-1: if the plan is revised, reconcile exit-criterion 1's "explicit trigger
  phrases" wording with the `<spec>` instructions contract so the two agree.
</next_steps>

<sources>
- MCP `instructions` field semantics — https://modelcontextprotocol.io/specification/2025-06-18/basic/lifecycle
- MCP model-controlled tool discovery — https://modelcontextprotocol.io/specification/2025-06-18/server/tools
- rmcp 1.7.0 `#[tool]` attribute (description is metadata only) — https://docs.rs/rmcp/1.7.0/rmcp/attr.tool.html
- Reviewer standard (ship code that satisfies the plan) — https://google.github.io/eng-practices/review/reviewer/standard.html
- Tier file under audit — .claude/plans/ariadne-core/tier-15-mcp-discoverability.md
- Sibling plan — .claude/plans/ariadne-core/plan.md
</sources>
