---
tier_id: tier-04
audited: 2026-06-09
verdict: PASS
commit: 904a9962e9cbcc60ba35be4f301018bc29f812bc
---

<scope>
Audited tier-04 "Roll the economy helper out to the diff-aware tools" against
`block-1/plan.md`, ADR-0031, and the tier `<exit_criteria>`. Scoped diff =
`git diff HEAD` over the tier `<files>` plus the two justified consequents
(`core/.../rows.rs` — the real home of `DiffSeed`, which `<files>` mislocated to
`response.rs`; `daemon/.../dispatch.rs` — routes the new query-variant fields to
the warm handlers). New files: `crates/ariadne-cli/tests/affected_tests.rs`,
`crates/ariadne-mcp/tests/tools_affected_tests.rs`,
`docs/adr/0031-diff-aware-pagination.md`. Ariadne graph fresh at revision 1416
(`project_status`); read-confirmed every changed file end-to-end.
</scope>

<checks_run>
- plan_adherence: every `<files>` entry touched as intended; the two out-of-list
  files are necessary consequents (see scope), nothing smuggled. No new dep —
  fingerprint is hand-rolled FNV-1a, codec reuses ADR-0029 hex helpers
  (constraints honored).
- correctness: cold `tools::{diff_blast,affected_tests}::page` and warm
  `impact::{diff_blast_page,affected_tests}` build identical shapes — sort by
  `(file, byte_start, name)` BEFORE concise projection nulls `byte_start`;
  per-seed inner lists capped at `limit` with full pre-cap count
  (`must_touch_total`/`may_touch_total`) read before truncation; seed symbol kept
  detailed until after the seeds-page sort. Aggregate `must_touch`/`may_touch`
  remain the full deduped union and ARE paged, so no dependent is unreachable via
  the bounded per-seed preview (ADR-0031 decision 2).
- security: cursor is opaque hex, length-checked before fixed-window reads
  (`le_u32`/`le_u64` callers guard); malformed/stale input → typed error, never a
  panic or wrong rows. No injection / secret / deserialization surface.
- performance: capping only shrinks already-computed results; sorts are
  O(n log n) over bounded lists; saturating `u32`/`usize` conversions. No hot-path
  regression.
- architecture: economy stays a pure `ariadne-graph` use case; adapters
  (`mcp`/`daemon`/`cli`) call it, never each other; `From`-projection lives at
  the wire boundary so the postcard daemon-IPC type carries `None` and the
  JSON-level `skip_serializing_if` omission applies at serialize. `cargo test
  --test architecture` green.
- tests: behavioral, loud — top-level cap + multi-list cursor round-trip
  (per-sublist union == un-capped), per-seed inner cap + reported count,
  concise⊂detailed, stale-fingerprint reject; cold (spawn_client), warm
  (warm_graph socket + server `assert_parity` byte-oracle), CLI (real indexed
  repo, both `affected-tests` and `query affected_tests`).
- docs: ADR-0031 records both decisions (fingerprint stamp; fixed inner cap)
  with rationale/alternatives/consequences; snapshots re-accepted to the concise
  default (BR2). `<verification>` reproduced in full below.
- exit_criteria: all four independently verified (see findings preamble).
</checks_run>

<findings>
| id | category | severity | location | problem | fix |
|---|---|---|---|---|---|
| F1 | architecture | INFO | crates/ariadne-daemon/src/domain/queries/impact.rs:324,511 | A stale/malformed cursor on the WARM path returns `DaemonResponse::Error` → `project_daemon` maps it to JSON-RPC `internal_error` (−32603), while the COLD path returns `McpError::InvalidInput` → `invalid_params` (−32602); the error *message* is identical, only the envelope code diverges. Pre-existing arc-wide pattern (same shape in tier-01/03 `impact.rs:95` for `blast_radius`, audited PASS); the exit criterion ("graceful invalid-cursor error, not wrong rows") and ADR-0031's cold-path −32602 claim both hold. | If strict cold==warm error-code parity is wanted arc-wide, give `DaemonResponse` a typed `InvalidInput` arm mapped to `invalid_params`; out of this tier's scope. |
</findings>

<verdict>
PASS. Zero FAIL findings; one non-blocking INFO.

Exit criteria:
1. **Top-level cap + multi-list cursor + note** — VERIFIED. `affected_tests`
   (`tests`,`seeds`) and `diff_blast_radius` (`seeds`,`must_touch`,`may_touch`)
   each `paginate_sublist` per offset, share one `diff_multi_cursor`, and emit
   `note` via `multi_truncation_note`. Tests `affected_tests_caps_and_round_trips`
   and `diff_blast_caps_and_round_trips_top_level` page to exhaustion and assert
   per-sublist union == un-capped set (completeness, no gap/dup).
2. **Per-seed inner fixed cap + count, never nested cursor** — VERIFIED.
   `inner_page` sorts, truncates to `budget.limit`, reports pre-cap count on
   `DiffSeed{must_touch_total,may_touch_total}`. Only the three top-level lists
   are cursored. Test `diff_blast_per_seed_inner_cap_reports_count` (cap 1,
   `must_touch_total: 2`).
3. **Cursor stamped revision + changed-paths fingerprint; changed diff rejects**
   — VERIFIED. `DiffCursor{revision,fingerprint,offsets}`, order-free FNV-1a
   `diff_fingerprint`, `decode` → `CursorError::StaleDiff` on mismatch. Tests
   `*_stale_fingerprint_rejects_cursor` edit a 2nd file between pages → graceful
   error naming the cursor, not wrong rows. Economy unit tests cover round-trip +
   revision/fingerprint rejection.
4. **Parity (cold==warm==CLI), ADR, green gates** — VERIFIED. Cold spawn_client
   tests, warm `diff_blast_resolves_changed_seed_over_warm_socket`, server
   `{diff_blast,affected_tests}_arm_matches_cold_output` byte-oracle, CLI
   `affected_tests.rs` (both routes). ADR-0031 present (Accepted). fmt clean,
   clippy `-D warnings` exit 0 (0 warnings), architecture green, snapshots
   re-accepted to concise default.
</verdict>

<checks_run_commands>
- `cargo fmt --all --check` → clean (exit 0).
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` → exit 0, 0 warnings.
- `cargo nextest run -p ariadne-mcp -E 'test(diff_blast) | test(affected_tests)'` → 11 passed.
- `cargo nextest run -p ariadne-cli -E 'test(affected_tests)'` → 2 passed.
- `cargo nextest run -p ariadne-daemon` → 30 passed.
- `cargo test --test architecture` → 1 passed.
- `cargo nextest run -p ariadne-mcp -E 'test(handshake) | test(diff_blast)'` → 11 passed (snapshots current).
Dogfood: the cold/warm/CLI integration tests exercise capping + paging +
stale-fingerprint on real indexed git repos end-to-end; the re-accepted
`diff_blast_working_tree` snapshot shows concise dropping `id`/`byte_start`/
`byte_end` from every symbol row (the ≈⅓ per-row reduction D3 targets). The
≤25k-token harness assertion is tier-05's scope, not tier-04's.
</checks_run_commands>

<next_steps>
None required for PASS. Optional: if the arc later wants byte-identical
cold/warm error codes, address F1 across all cursor tools in a dedicated change
(not a tier-04 redo).
</next_steps>

<sources>
- Tier + plan: .claude/plans/data-fidelity-arc/block-1/tier-04-diff-aware-rollout.md ; block-1/plan.md
- ADR: docs/adr/0031-diff-aware-pagination.md ; 0029 ; 0030
- MCP pagination (opaque cursor, handle-invalid-gracefully): https://modelcontextprotocol.io/specification/2025-06-18/server/utilities/pagination
- Anthropic writing tools for agents (concise ≈⅓, steer-on-truncate, 25k cap): https://www.anthropic.com/engineering/writing-tools-for-agents
- rmcp invalid_params mapping: crates/ariadne-mcp/src/errors.rs:40-43
- Reviewer standard (code health over perfection): https://google.github.io/eng-practices/review/reviewer/standard.html
</sources>
</content>
</invoke>
