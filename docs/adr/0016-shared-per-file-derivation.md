# ADR-0016: Shared Per-File Derivation In ariadne-salsa

<status>
Accepted
Date: 2026-05-29
Decider: claude (post-v1 tier-07a, RD11)
</status>

<context>
The only real per-file derivation — stable `SymbolId`, the synthesized SFC
`Component` symbol, and global call/render/hook edge resolution — lived in the
`ariadne-cli` driving adapter's streaming committer
[src: crates/ariadne-cli/src/domain/mod.rs:497-768 @ f6b6ae5]. The
`ariadne-daemon` adapter (post-v1 tier-06/07) cannot reach it: adapters never
depend on each other [src: tests/architecture.rs lines 38-49]. Meanwhile the
`ariadne-salsa` queries and `commit_revision` were tier-04 stubs
[src: crates/ariadne-salsa/src/derived.rs:116-182 @ f6b6ae5;
crates/ariadne-salsa/src/db.rs:106-110 @ f6b6ae5]. Shipping a second
derivation in the daemon would mean two paths to keep bit-identical — the
drift risk that gates tier-08. The forces: maintainability (one derivation),
reliability (cold/warm produce the same graph), and the hexagonal invariant
(`ariadne-salsa` is a use-case crate; its in-workspace deps must stay ⊆
{`ariadne-core`, `ariadne-storage`} [src: tests/architecture.rs lines 32,35]).
</context>

<decision>
Move the pure per-file derivation into `ariadne-salsa` behind a driver. Parsed
facts enter salsa through a new `#[salsa::input] SyntacticFactsInput` set by
each composition root (the CLI cold index, the daemon warm derive); per-file
*symbol* derivation is the salsa-memoized tracked query `symbols_for_file`;
global edge resolution is a pure driver pass in `commit_revision`. The CLI
cold index is refactored onto this one path, guarded by a cold byte-parity
gate.
</decision>

<rationale>
- **Maintainability / reliability.** One derivation means the cold index and
  the daemon warm graph cannot diverge by construction; the byte-parity gate
  [src: crates/ariadne-cli/tests/index_parity.rs] freezes the cold output
  against the pre-refactor golden so a future edit to the shared path is
  caught immediately.
- **Hexagonal invariant.** `ariadne-salsa` may not depend on `ariadne-parser`
  [src: tests/architecture.rs lines 30-33], so parsing cannot move into salsa.
  Facts therefore enter as an `Update`-safe input (`SyntacticFactsRaw`,
  mirroring `ariadne_parser::SyntacticFacts`); the only added dependency is
  `blake3` (the `SymbolId` hash, pure-Rust, already a workspace dep — D5
  holds). `decl_kind_tag` and `Visibility::to_byte` stay at the composition
  root because they read parser enums.
- **Efficiency.** Per-file symbol derivation stays salsa-memoized, so an
  incremental edit (tier-07b/08) re-derives only changed files. Global edge
  resolution needs every symbol, so it does not fit per-file memoization; it
  runs as a pure pass over the union, mirroring the CLI's existing two-phase
  structure [src: crates/ariadne-cli/src/domain/mod.rs:624-672 @ f6b6ae5].
  The O(total symbols) per-commit cost is the accepted R-B4 trade
  [src: post-v1-roadmap plan.md `<risks>`].
- **Watcher contract preserved.** `syntactic_facts` still touches
  `FileContentInput::content`, so a content edit invalidates it (the daemon
  re-parses and resets the facts input on that same edit); the tier-06
  re-execution test holds unchanged.
</rationale>

<alternatives>
- **Parallel daemon-only derivation** — rejected: two derivations to keep
  bit-identical, the exact drift that blocks tier-08. `[src: post-v1-roadmap plan.md RD11]`
- **A new `ariadne-derive` crate** — rejected: extra crate surface when salsa
  already scaffolds the fact mirrors and is the natural memoization home.
  `[src: crates/ariadne-salsa/src/derived.rs:25-111]`
- **Carry the `SymbolId` in `SymbolFactsRaw`** — rejected: the driver
  recomputes it via `derive::symbol_id(path, name, offset)` from fields it
  already holds, so the memoized query stays free of the (offset-dependent,
  soon-to-change in tier-07b) id scheme. `[src: post-v1-roadmap plan.md RD12]`
</alternatives>

<consequences>
- `ariadne-salsa` gains `derive.rs` (pure), a `SyntacticFactsInput`, a
  per-file input registry on `AriadneDb`, and a real `seed_file` /
  `seed_from_disk` / `commit_revision`. Its dep set stays ⊆ {core, storage} +
  `blake3`; `tests/architecture.rs` still passes.
- `symbols_for_file` / `syntactic_facts` gain a `SyntacticFactsInput`
  argument; all callers (salsa tests, the edit bench, the watcher pipeline
  test) thread an empty facts input — a mechanical signature change, no
  behaviour change.
- The CLI streaming committer (`run_committer`, batched `Changeset` writes)
  is removed; the cold index now parses, seeds, and commits one changeset.
  The bounded-RAM streaming property is traded for the single shared path
  (acceptable for tier-07a; not a tier-07a SLO). ADR-0010's streaming commit
  is superseded for the cold path.
- tier-07b builds on this: it makes the `SymbolId` edit-stable and
  `commit_revision` diff-aware (ADR-0017), then adds `rederive_file` /
  `forget_file` for the tier-08 watcher.
- Off-limits without superseding: re-introducing a second derivation path in
  any adapter.
</consequences>

<sources>
- `[src: post-v1-roadmap plan.md RD11, RD12, R-B4]`
- `[src: tests/architecture.rs lines 30-49]`
- `[src: crates/ariadne-cli/src/domain/mod.rs:497-768 @ f6b6ae5]`
- `[src: crates/ariadne-salsa/src/derive.rs ; src/db.rs ; src/derived.rs ; src/inputs.rs]`
- `[src: crates/ariadne-cli/tests/index_parity.rs]`
- `[src: docs/adr/0010-streaming-cold-index.md ; docs/adr/0007-cli-composition-root.md]`
</sources>
