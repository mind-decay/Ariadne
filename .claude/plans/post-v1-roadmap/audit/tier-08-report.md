---
tier_id: tier-08
audited: 2026-05-30
verdict: PASS
commit: f6b6ae56e514104d6eead95176cc1a9fdf14d565
---

<scope>
Audited tier-08 (daemon live updates: watcher loop → incremental warm-graph
invalidation) against `tier-08-daemon-watcher-live.md` + sibling `plan.md`.
Scoped diff = the daemon crate (`Cargo.toml`, `src/`, `tests/`) plus the two
files the build notes pull into tier-08: the salsa `Changeset`-surfacing change
in `ariadne-salsa/src/db.rs` (resolved decision 1) and the CLI watcher wiring in
`ariadne-cli/src/commands/daemon.rs`. The remaining uncommitted diff
(`ariadne-salsa/src/derive.rs`, `derived.rs`, `inputs.rs`, the CLI cold-index
refactor, ADR-0016/0017, fixtures/goldens, `index_parity.rs`) belongs to the
tier-07a/07b audits and was excluded.

Files read end-to-end: `live.rs`, `facts.rs`, `dump.rs`, `catalog.rs`,
`snapshot.rs`, `adapters/ipc.rs`, `domain/mod.rs`, `lib.rs`, `Cargo.toml`,
`tests/live_update.rs`, `tests/incremental_warm.rs`, `commands/daemon.rs`, the
tier-08 surface of `db.rs`, plus `tests/architecture.rs`, `redb/mod.rs`, and
`salsa/src/memory.rs` for invariant verification.
</scope>

<checks_run>
- plan_adherence: `<files>` honoured. `ariadne-watcher` deliberately NOT a
  daemon dep (build-notes decision 3); the CLI composition root wires the
  watcher to `serve_live` over the `ariadne_core::Invalidation` channel. The
  `ariadne-scip` dep is deferred (decision 2). Both deviations are documented in
  the tier file's own build notes and are the sanctioned path, not smuggled.
- exit_criterion 1 ("daemon owns the watcher event loop"): reconciled. The
  literal wording is overridden by build-notes decision 3 (strict hexagonal
  invariant — no driving→driving dep). Functionally met: the daemon hosts the
  update pump thread (`LiveEngine::spawn_pump`, live.rs:199) draining
  Invalidations; the CLI owns the watcher. Aligns with ADR-0007 + the
  architecture invariant the literal reading would violate.
- exit_criterion 2 (edit re-derives subset + applies delta): met —
  `upsert` → `AriadneDb::rederive_file` (salsa) → `WarmCatalog::apply_changeset`
  → `GraphIndex::apply_delta` (live.rs:124-151, catalog.rs:151-215).
- exit_criterion 3 (redb deltas persist, revision advances): met —
  `commit_changeset` runs `WriteTxn::apply` and returns the new `RevisionId`;
  `apply_changeset` bumps `cat.revision` (db.rs:225-233, catalog.rs:214).
- exit_criterion 4 (divergence-0 proptest): met — `warm_apply_equals_fresh_
  rebuild`, 100 cases, create/edit/delete/recreate + cross-file edges, PASS.
- architecture: re-ran `tests/architecture.rs` — green. Daemon deps =
  core/graph/storage/salsa/parser/blake3/thiserror/interprocess/postcard; no
  sibling driving adapter. `RwLock`-serialized read/write on the warm catalog.
- correctness: traced symbol-id stability (path-based RD12), `by_name`
  ascending-id ordering (binary-search insert), file-id reassignment after
  delete, edge body-only churn, exhaustive stale-delete diff. All consistent
  with a fresh rebuild; the proptest is the empirical guard.
- memory probe (R1): `memory_report()` is the tier-04 zero-baseline stub
  (memory.rs:48-56) — calling it asserts nothing. Incremental accumulation is
  instead bounded by the divergence-0 proptest (warm graph == fresh rebuild ⇒
  no stale-record growth); absolute daemon RSS is scheduled at tier-10 (plan
  Block B). Reconciled — not a finding.
- verification commands (all re-run green):
  - `cargo fmt --all --check` → exit 0.
  - `cargo nextest run -p ariadne-daemon` → 20/20 pass incl.
    `live_update::live_edit_is_reflected_over_ipc` and the 55s 100-case
    `incremental_warm::warm_apply_equals_fresh_rebuild`.
  - `cargo test --test architecture` → 1 passed.
  - `cargo clippy --workspace --all-targets -- -D warnings` → exit 0.
</checks_run>

<findings>
| id | category | severity | file:line | problem | fix |
|----|----------|----------|-----------|---------|-----|
| I1 | correctness | INFO | crates/ariadne-daemon/src/domain/live.rs:10-13, crates/ariadne-daemon/src/adapters/ipc.rs:124 | The doc comment claims the catalog write lock makes a concurrent staleness rebuild "never open redb at the same time", but `serve_connection` calls `load_catalog` (which opens redb) *outside* the catalog lock, while the pump opens redb *under* the write lock. With a concurrent external writer advancing redb (the only case where `is_stale` is true in single-writer live mode), the two opens can race → transient `DatabaseAlreadyOpen`. Effect is recoverable (typed error response or dropped invalidation re-emitted by reconcile), so non-blocking. | Open redb for the staleness rebuild while holding the catalog write lock (or soften the comment to state the narrow race). |
| I2 | performance | INFO | crates/ariadne-daemon/src/domain/live.rs:135 | `upsert` allocates a fresh `extractors` map per edit, so the per-engine `FactExtractor` cache `parse_facts` is designed for (and `start` uses) is discarded between edits — every edit recompiles the tree-sitter fact queries. Well within the <500ms incremental SLO. | Hold the extractor cache on `LiveEngine` and reuse it across `upsert` calls. |
</findings>

<verdict>
PASS. Zero FAIL findings. All four `<verification>` commands re-run green; the
live-update integration test exercises the create→query→edit→query path over the
real IPC socket (symbol rename + caller-edge re-resolution), and the 100-case
incrementality proptest proves divergence 0 between the live-updated warm graph
and a fresh rebuild — the load-bearing tier-08 invariant. The watcher-placement
deviation from exit-criterion 1 is explicitly sanctioned by the tier's build
notes and preserves the hexagonal invariant. The two INFO items are minor and do
not gate. The manual self-index fs→`NotifyWatcher`→daemon run remains the only
unautomated `<verification>` item (translation/debounce are `ariadne-watcher`'s
own tested concern); it was not executed in this audit session.
</verdict>

<next_steps>
None required for PASS. Optional follow-ups: address I1 (lock-scope the
staleness redb open) and I2 (engine-level extractor cache) when tier-10 lands the
daemon SLO/RSS work. Run the manual self-index live edit once before relying on
the real watcher path in production.
</next_steps>

<sources>
- redb 4.1.0 `Database::create` exclusive lock (single-open-per-process):
  https://docs.rs/redb/4.1.0/redb/struct.Database.html
- Hexagonal adapter-isolation invariant: crates/.../tests/architecture.rs:34-45,
  113-138; docs/adr/0007-cli-composition-root.md
- Reviewer standard (code health over perfection; nits are non-blocking):
  https://google.github.io/eng-practices/review/reviewer/standard.html
- tier file + plan: .claude/plans/post-v1-roadmap/tier-08-daemon-watcher-live.md;
  .claude/plans/post-v1-roadmap/plan.md (RD6, RD11, RD12, R-B2, R-B4)
</sources>
</content>
</invoke>
