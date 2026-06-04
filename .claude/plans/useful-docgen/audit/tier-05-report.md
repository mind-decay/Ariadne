---
tier_id: tier-05
audited: 2026-06-04
verdict: PASS
commit: 710f3c80cd069940babab7c42eeb3ff894ff4b76
---

<scope>
Re-audit of tier-05 "Symbol doc enrichment" after the post-audit amendments
that resolved the three INFO findings of the prior pass (F2 depth-1→3, F3
direct `file_risk`, F1 `<files>` rewrite). The working tree at HEAD 710f3c8
holds the amended, uncommitted code; the scoped diff (12 files) matches the
tier `<files>` touch set exactly — nothing outside it:

- `crates/ariadne-core/src/domain/daemon/response.rs` — `DocForReport` +4 fields, `Eq` derive dropped.
- `crates/ariadne-graph/src/{hotspot.rs,doc_model.rs,lib.rs}` — pure `file_risk` / `symbol_role` use cases + façade re-export.
- `crates/ariadne-daemon/src/domain/queries/docs.rs` — `doc_for` enrichment, `file_complexity`, depth-3 blast, scope-filtered `public_refs`.
- `crates/ariadne-mcp/src/tools/doc_for.rs` — cold handler, same enrichment + depth + scope.
- `crates/ariadne-mcp/src/types.rs` — `DocForOutput` DTO mirror (+4 fields).
- `crates/ariadne-mcp/src/server.rs` — parity unit-test fields.
- `crates/ariadne-{mcp/tests/{tools_doc_for.rs,support.rs},daemon/tests/warm_analytics.rs,graph/tests/hotspot.rs}` — tests.

Correctly untouched (verified, not trusted): `codec.rs` encodes the whole
`DaemonResponse` via `postcard::to_stdvec` over the serde derive
[codec.rs:57-63], so additive fields flow with no field-by-field mirror; and
`cli/commands/query.rs` renders every arm via field-agnostic `json(&report)`
[query.rs:200,279], so the new fields serialize automatically.
</scope>

<checks_run>
- `cargo fmt --all --check` → clean (exit 0).
- `cargo deny check` → advisories/bans/licenses/sources ok; only pre-existing unmatched-license-allowance warnings. Proves no new dependency (plan no-new-dep constraint).
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` → clean, no warnings.
- `cargo nextest run -p ariadne-mcp -p ariadne-daemon -p ariadne-cli -p ariadne-graph` → 208/208 passed (1 leaky, pre-existing resource note, not a failure).
- Targeted re-run: `file_risk_matches_ranked_score` (graph), `doc_for_matches_cold` (warm/cold parity, daemon), `server::tests::doc_for_arm_matches_cold_output` (JSON parity), `doc_for_returns_signature_and_refs` + `doc_for_scope_filters_test_path_neighbours` (mcp) — all PASS.
- `cargo test --test architecture` → `architecture_invariants_hold` ok (core owns the type; graph owns the pure helpers; adapters only consume + render).
- Read every changed file end-to-end. Cross-checked `file_risk` against `rank`/`file_hotspots` (hotspot.rs:74-120): identical norm over the churn set, same f64→f32 cast → byte-identical score (locked by `file_risk_matches_ranked_score`). Cross-checked the `blast_must`/`blast_may` field docs against `blast.rs:64-96`: must = immediate-dominator predecessors within depth, may = the rest — field docs accurate (and more precise than blast.rs's own struct doc).
- Confirmed `Eq` drop is safe: `DaemonResponse` derives `PartialEq` only (response.rs:178), `DocForReport` is used in no `Eq`-requiring context, and the whole workspace compiles under `-D warnings`.
- Parity mechanism: `DocForReport` and `DocForOutput` field order is identical; cold/warm both share `DOC_BLAST_DEPTH=3`, `DocScope::default()`, and the same `file_complexity` build → byte-equal JSON.
</checks_run>

<findings>
| id | category | severity | location | problem | fix |
| --- | --- | --- | --- | --- | --- |
| F1 | docs | INFO | tier-05-symbol-enrich.md step 2 (lines 50-51) | The unamended `<steps>` step 2 still instructs extending "the `codec.rs` DTO mirror" with a `[src: …codec.rs]`, contradicting the amended `<files>`/prior-F1 (and the code) which correctly leave `codec.rs` untouched. | Reword step 2 to drop the codec mirror clause; no code change — the implementation is correct. |
| F2 | performance | INFO | daemon docs.rs:81-90; mcp doc_for.rs:84-95 | `file_complexity` still rebuilds the full per-file complexity map (O(symbols) scan + alloc) on every single-symbol `doc_for` call on both paths; the F3 amendment dropped the file-ranking sort but retained this build. | Plan-accepted (the amendment keeps it deliberately; `max_complexity` needs it); deterministic, no SLO gate at tier-05 — flag only for the ≥10 perf gate (cache or scope the build if `doc_for` p95 matters at 100K files). |
</findings>

<verdict>
PASS. Zero FAIL findings. All five exit criteria independently verified by
execution:
- EC1 — `role`/`file_risk`/`blast_must`/`blast_may` appended after the stable `signature`/`kind`/`file`/`brief`/`public_refs` prefix (response.rs:86-100); pre-existing fields unchanged in name and order. The only non-additive delta is dropping the `Eq` derive (forced by `f32`), which is safe and compiles.
- EC2 — daemon (docs.rs:68-76) and mcp (doc_for.rs:73-81) populate all four; cli displays via field-agnostic `json(&report)` on both warm (query.rs:200) and cold (query.rs:279) routes.
- EC3 — `public_refs` scope-filtered through `DocScope.include`, proven by `doc_for_scope_filters_test_path_neighbours` (a `tests/`-path caller is dropped while the source caller is kept, and the blast counts still see both — D3: scope is a doc-layer filter, never a graph mutation).
- EC4 — cold/warm parity green: `doc_for_matches_cold` (full report) + `doc_for_arm_matches_cold_output` (JSON-serialization equality).
- EC5 — structured + parity tests green; clippy / fmt / deny / architecture all green.

The amendments are sound: depth-3 blast gives `blast_may` real signal
(demonstrated non-zero by `doc_for_returns_signature_and_refs`, `blast_may=1`),
`public_refs` (now must-touch only, scope-filtered) and `blast_must` keep their
intended meaning, and `file_risk` scores the queried file directly with a score
byte-identical to the full ranking. No smuggled dependency or pattern; hexagon
intact (graph stays pure, adapters only render). The two INFO findings do not
gate.
</verdict>

<next_steps>
None blocking — tier may proceed to commit (audit-gate state updated to PASS).
Optional, not tier-05 rework:
- F1: reword tier-05 `<steps>` step 2 to match the amended `<files>` (drop the codec-mirror clause).
- F2: at the ≥10 perf gate, measure `doc_for` p95 on a 100K-file workload; cache/scope `file_complexity` if it breaches the <100ms query budget.
</next_steps>

<sources>
- repo: crates/ariadne-graph/src/blast.rs:64-96 (must=immediate-dominator / may semantics); hotspot.rs:61-156 (`norm`/`rank`/`file_hotspots`/`file_risk` — direct score equals ranked score); doc_model.rs:60-133 (`DocScope.include`, `crate_of`, `symbol_role`); crates/ariadne-daemon/src/adapters/codec.rs:57-63 (whole-`DaemonResponse` postcard); crates/ariadne-core/src/domain/daemon/response.rs:178 (`DaemonResponse: PartialEq` only); crates/ariadne-cli/src/commands/query.rs:200,279 (field-agnostic `json`).
- plan.md tier-05 D6 (graph-pure helper, no daemon-handler dependency); D3 (scope is doc-layer only); CLAUDE.md D13 (core owns the type, adapters render).
- [OWASP Top 10](https://owasp.org/www-project-top-ten/) — reviewed; read-only additive change, no input-validation / injection / deserialization surface added (postcard frame cap unchanged at 64 MiB).
</sources>
</output>
