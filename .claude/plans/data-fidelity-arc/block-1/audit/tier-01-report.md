---
tier_id: tier-01
audited: 2026-06-08
verdict: PASS
commit: f96356f51593038b7817c3e92f54f86d9e6dc44c
---

<scope>
Re-audit of tier-01 "shared response-economy helper + find_references pilot" of
`data-fidelity-arc/block-1` (the prior audit FAILed on an unformatted file). Scoped
diff = the tier's `<files>`: the new pure module `crates/ariadne-graph/src/economy.rs`,
its façade re-export (`graph/src/lib.rs`), the core wire surface
(`daemon/{query,response,rows,mod}.rs`, `core/src/lib.rs`), the cold MCP handler +
types + server wiring (`mcp/{tools/find_references.rs,types.rs,server.rs}`), the warm
daemon handler + dispatch (`daemon/{queries/navigate.rs,dispatch.rs}`), the CLI thread
(`cli/commands/query.rs`), `docs/adr/0029-…md`, plus the test/snapshot updates the
shape change forced (`mcp/tests/tools_find_references.rs`,
`mcp/tests/snapshots/handshake__tools_list.snap`, `daemon/tests/{warm_graph,live_update,
scip_pass}.rs`). Diff is uncommitted (gate-blocked pre-PASS); HEAD = f96356f. Ariadne
graph fresh at revision 636.
</scope>

<checks_run>
- Read every file in `<files>` end-to-end, plus the four forced test updates.
- `cargo fmt --all --check` → **exit 0** (the prior FAIL's navigate.rs:89 is now
  rustfmt-clean — the prior F1 blocker is resolved).
- `cargo nextest run -p ariadne-graph -E 'test(economy)'` → 5/5 PASS (codec
  round-trip, wrong-revision reject, garbage reject, paginate completeness,
  single-page-no-cursor).
- `cargo nextest run -p ariadne-mcp -E 'test(find_references)'` → 4/4 PASS
  (concise-by-default, cursor round-trip union, concise⊂detailed + smaller bytes,
  warm==cold parity arm). One "LEAK" mark on the parity arm is a benign tokio
  task-leak warning; the test passed.
- `cargo clippy -p ariadne-graph -p ariadne-daemon -p ariadne-mcp -p ariadne-cli
  -p ariadne-core --all-targets -- -D warnings` → genuine recompile, 0 warnings, exit 0.
- `cargo test --test architecture` → ok (hexagonal invariants hold; `ariadne-graph`
  is a domain crate, so adapter→graph is allowed, not driving→driving).
- `RUSTDOCFLAGS="-D warnings" cargo doc -p ariadne-graph --no-deps` → exit 0.
- Snapshot I1 from the prior audit (`assertion_line` leak) is gone — header is clean.
- `McpError::InvalidInput` → `rmcp::ErrorData::invalid_params` (−32602), matching D2
  (errors.rs:43).
- Dogfood (rebuilt `target/debug/ariadne`, live `.ariadne/index.redb` @ rev 636):
  `find_references {"symbol":"Lang"}` → uncapped 244 rows (~7146 tok); default page =
  50 rows + cursor `7c02…` (decodes to revision 636, offset 50) + note "Showing 50 of
  244 references — …" (~1408 tok, ~5× smaller, ≪25k). Five pages reconstruct the
  uncapped set in identical stable order (`paged == uncapped` true, no gap/dup).
  Concise omits `caller`/`byte_start`/`byte_end`; `verbosity:detailed` restores them.
  Garbage cursor → "malformed pagination cursor"; a revision-1 cursor → "stale
  pagination cursor (minted at revision 1, current is 636); re-run …" (D2 graceful
  reject, never silent).
- `digest` does not consume references (only a help-text mention at digest.rs:203), so
  the "pin detailed if digest consumes references" conditional (step 6) is a correct
  no-op. No consumer of `DaemonResponse::References` / `find_references::handle` exists
  outside the tier's `<files>` and the forced test updates — no dangling refs.
- No new workspace dependency: `ariadne-graph` was already a dep of
  `ariadne-mcp`/`ariadne-daemon`/`ariadne-cli`; the cursor codec is hand-rolled hex.
</checks_run>

<findings>
| id | category | severity | location | problem | fix |
|---|---|---|---|---|---|
| I1 | correctness | INFO | crates/ariadne-graph/src/economy.rs:140-167 | `limit:0` returns an empty page with a `next_cursor` whose offset equals the current offset (0); re-feeding it yields the same empty page + same cursor, so a client paginating with `limit:0` makes no progress (liveness footgun). Not reachable by the schema default or CLI (both omit→50), never crashes or mis-pages, and the note honestly says "Showing 0 of 244". | Treat `limit == 0` as "no more rows" (emit `next_cursor: None`) or clamp `limit` to ≥1 before slicing. |
| I2 | tests | INFO | crates/ariadne-mcp/src/tools/find_references.rs:138-141; crates/ariadne-daemon/src/domain/queries/navigate.rs:152-157 | The truncation `note` wording is duplicated in the cold (`steer`) and warm (inline `format!`) handlers; the `references_arm_matches_cold_find_references` parity test (server.rs:951) asserts against *literal* note strings, not the *computed* ones, so a future divergence in the note text would not be caught by the parity guard. The two strings are byte-identical today, so there is no present parity defect. | Move the steer string into `economy` (single source) or assert the computed cold note == computed warm note. |
</findings>

<verdict>
PASS — zero blocking findings.

The prior audit's single FAIL (F1: `cargo fmt --all --check` exit 1 on navigate.rs:89)
is resolved — fmt is now exit 0 — and the prior INFO (snapshot `assertion_line` leak)
is gone. Every `<verification>` command re-runs green: graph 5/5, mcp 4/4, architecture
ok, clippy 0 warnings, rustdoc 0 warnings.

The implementation satisfies all five exit criteria. The pure `economy` helper (D1)
lives in `ariadne-graph` with an opaque, revision-stamped, hand-rolled hex cursor
(D2, no new dep — confirmed against `crates/*/Cargo.toml` and a clean clippy), a
concise default that is a strict subset of a lossless `detailed` (D3, proven by test +
the dogfood byte delta 7146→1408 tok), a per-list stable `(file, byte_start,
caller_name)` sort capped at `DEFAULT_PAGE = 50` (D4), and `next_cursor`/`note`
truncation reporting (D5). Three-path parity holds: the cold handler, the warm daemon
handler, and the CLI all converge on the same shape via the one shared
`economy::paginate`, with the warm `ReferencesReport` `From`-projected to
`FindReferencesOutput` and unit-asserted byte-identical. The documented step-3
deviation — `skip_serializing_if` on the MCP wire type rather than the daemon-IPC
`ReferenceSite` (postcard is non-self-describing and would underflow the decoder on an
omitted field) — is sound, dogfood-confirmed, and explained in the source. ADR-0029
records the mechanism, the deliberate protocol revision (BR4), and the no-redb-change
boundary. The live cursor round-trip (5 pages == 244 rows, no gap/dup) and the graceful
stale/garbage-cursor rejection were exercised end-to-end against the real index.

The two INFO findings are a degenerate-input liveness footgun (`limit:0`) and a
test-robustness gap on the duplicated steer wording; neither is a present defect nor an
exit-criterion violation, so neither gates.
</verdict>

<next_steps>
None required for PASS. The diff may be committed (the gate now unblocks). Optional,
non-blocking: address I1 (`limit:0` → no cursor) and I2 (single-source the steer string
or assert computed-note parity) in this tier or fold them into the tier-02–04 rollout
where the same `economy::paginate` and per-tool note are reused.
</next_steps>

<sources>
- Tier under audit: .claude/plans/data-fidelity-arc/block-1/tier-01-economy-helper.md
- Sibling plan: .claude/plans/data-fidelity-arc/block-1/plan.md (D1–D5, constraints, risks)
- ADR: docs/adr/0029-response-economy-cursor-verbosity.md
- MCP pagination (opaque cursor, handle-invalid-gracefully, list-ops-only): https://modelcontextprotocol.io/specification/2025-06-18/server/utilities/pagination
- Anthropic writing tools for agents (concise ≈⅓ tokens, 25k cap, steer-on-truncate): https://www.anthropic.com/engineering/writing-tools-for-agents
- Reviewer standard (code-health over perfection): https://google.github.io/eng-practices/review/reviewer/standard.html
- Comment-only-on-real-defects: https://google.github.io/eng-practices/review/reviewer/comments.html
</sources>
</output>
