# ADR-0026: Default-On Out-Of-Band SCIP

<status>
Accepted
Date: 2026-06-05
Decider: user / claude
</status>

<context>
Tiers 01–03 made SCIP drive precise graph edges (cross-crate references,
reads/writes, implements/type-of), but only behind the opt-in `--scip` flag,
which runs `IngestPlan` INLINE on the cold-index path
[src: crates/ariadne-cli/src/domain/mod.rs run_index]. So the committed,
dogfood, and MCP graph the LLM consumes stays tree-sitter-only unless a human
passes the flag, and the daemon — which has zero SCIP wiring — never produces a
SCIP edge at all. The forces: reliability (the precise edges must reach the
default graph), efficiency (13 external indexers perform full language builds —
seconds to minutes — and must not regress the cold<60s / incremental-p95<500ms
SLOs, R9), and maintainability (the daemon is a driving adapter kept isolated
from heavyweight driven adapters — RD7/ADR-0023 keep it free of `ariadne-git`).
Sourcegraph's auto-index endpoint is the anchor: "automatically uses Precise
whenever available, search-based as fallback"
[src: https://sourcegraph.com/docs/code-search/code-navigation/precise_code_navigation].
</context>

<decision>
SCIP is DEFAULT-ON with a `--no-scip` opt-out, and it runs OUT-OF-BAND on every
path: the fast tree-sitter index commits first, then a separate SCIP pass
re-commits the covered files' precise edges. In the CLI cold index the pass is a
post-commit follow-up (Phase 4, already off the measured walk/parse/resolve/
commit phases); in the daemon it is a background pass driven by the CLI
composition root — the CLI runs `IngestPlan`, extracts pure-core `ScipFacts`,
and ships them over a channel `serve_live` hands back, so `ariadne-daemon` stays
free of `ariadne-scip` exactly as it stays free of `ariadne-git`. A missing
indexer binary, or a file whose content hash has drifted off the indexed hash,
degrades that file to the precise tree-sitter resolver (ADR-0024/0025), never a
failure.
</decision>

<rationale>
- **Efficiency / R9.** SCIP never sits on a synchronous index, query, or
  incremental-commit path. The cold index's measured phases end at the
  tree-sitter `commit`; the SCIP pass is timed separately (`scip_ms`) and
  re-commits afterward, so cold<60s and incremental-p95<500ms are unchanged by
  default-on [src: crates/ariadne-cli/src/domain/mod.rs run_index Phase 4]. The
  daemon pass runs on its own background thread and only takes the warm-catalog
  write lock for the brief final commit + rebuild — a query in flight while the
  indexers build never blocks (it reads the current resolver/last-covered
  edges).
- **Reliability.** The precise resolver is the live fallback (plan D4): a file
  is "covered" — edges from SCIP — only while its content hash still matches the
  hash its facts were indexed at, so a live edit immediately drops back to the
  shape-gated resolver and never shows a stale SCIP edge
  [src: crates/ariadne-salsa/src/db.rs build_changeset coverage gate].
- **Maintainability / hexagonal isolation.** Driving the daemon's pass from the
  CLI composition root keeps `ariadne-daemon` an isolated driving adapter: only
  pre-computed pure-core `ScipFacts` cross the boundary, mirroring how
  pre-computed Git hunks cross it for `diff_blast_radius` (RD7/ADR-0023). The
  architecture test already permits this — the daemon depends only on
  `ariadne-core` for the wire type [src: tests/architecture.rs].
</rationale>

<alternatives>
- **Keep `--scip` opt-in** — rejected: the precise edges stay stranded behind a
  flag the daemon never sets, so the default graph the LLM reads stays
  tree-sitter-only — the exact gap this plan closes [src: plan.md D6].
- **Run SCIP inline, default-on** — rejected: 13 subprocess language builds on
  the cold-index path blow the cold<60s SLO (R9) [src: plan.md D6].
- **Daemon links `ariadne-scip` and runs `IngestPlan` itself** — rejected
  (permitted by the arch test, but) it makes a driving adapter own a heavyweight
  driven adapter, diverging from the RD7/ADR-0023 daemon-isolation precedent;
  the CLI-driven channel reuses the existing `serve_live`/`on_ready` background
  wiring and the cold path's fact conversion [src: ADR-0023; plan.md D6 "CLI
  follow-up"].
- **Union SCIP + resolver edges per file** — rejected: double-counts and
  produces conflicting `dst` for one `src`; coverage is per-file exclusive (D4).
</alternatives>

<consequences>
- `ariadne index` runs SCIP by default; deterministic syntactic-only callers
  (the cold byte-parity goldens) pass `--no-scip` to stay independent of which
  indexers are installed [src: crates/ariadne-cli/tests/index_parity.rs].
- `serve_live` gains a second hand-back to `on_ready`: a `ScipFactsBatch` sender
  feeding the live engine's pump, alongside the existing `IndexLock`. The pump
  applies a batch by setting the salsa SCIP inputs, re-committing, and rebuilding
  the warm catalog from the committed redb (no new `ariadne-salsa` API).
- Eventual consistency bound: the daemon's `LiveEngine` seeds from redb without
  SCIP facts, so until the first background pass completes the live db derives
  resolver edges; an edit landing in that window re-derives unedited covered
  files to resolver edges until the pass re-establishes coverage. The window is
  bounded by one ingest and self-heals; edited files correctly use the resolver
  regardless (D4).
- The daemon's SCIP pass thread is detached, not joined on shutdown: its only
  interruption point is the pre-build settle, so a `daemon stop` issued after the
  indexers have started returns immediately rather than blocking for the
  seconds-to-minutes a 13-indexer build takes; the in-flight facts are discarded
  (`tx.send` is best-effort once `serve_live` drops the receiver).
- Off-limits without superseding: putting SCIP on any synchronous/incremental
  path, or letting `ariadne-daemon` link `ariadne-scip`.
</consequences>

<sources>
- `[src: https://sourcegraph.com/docs/code-search/code-navigation/precise_code_navigation]`
- `[src: .claude/plans/scip-driven-edges/plan.md D4, D6]`
- `[src: docs/adr/0023-mcp-git-diff-dependency.md]`
- `[src: docs/adr/0024-scoped-call-resolution.md; docs/adr/0025-shape-scoped-same-crate-resolution.md]`
- `[src: crates/ariadne-cli/src/domain/mod.rs run_index; crates/ariadne-salsa/src/db.rs build_changeset]`
</sources>
