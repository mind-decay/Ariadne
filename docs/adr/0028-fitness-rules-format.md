# ADR-0028: ariadne-fitness.toml Architecture-Fitness Rules Format

<status>
Accepted
Date: 2026-06-07
Decider: claude (intelligence-platform block-a tier-04)
</status>

<context>
Block A's A3 productises the project's own `tests/architecture.rs` as
config-driven *fitness functions*: declarative layers + forbidden dependency
directions + cycle/coupling thresholds, checked against the live graph
[src: ArchUnit `layeredArchitecture` https://www.baeldung.com/java-archunit-intro;
"Building Evolutionary Architectures" (Ford/Parsons/Kua)]. The rules need a
stable, human-authored on-disk contract decoupled from the tool's own
`config.toml` (which already carries Ca 305 of coupling; loading rules there
would grow it and entangle two concerns) [src:
.claude/plans/intelligence-platform/block-a/plan.md D5]. The pure engine lives
in `ariadne-graph` and reuses the existing coupling/cycle analytics; only the
*parse + glob → layer resolution* is a composition-root concern, so the file
format is the public surface this ADR fixes.
</context>

<decision>
Architecture-fitness rules live in a dedicated `ariadne-fitness.toml` at the
project root with three sections: `[[layer]]` (a `name` and a list of path-glob
`paths`), `[[rule]]` (a `forbid = { from, to }` layer-name pair), and a single
`[thresholds]` table (`max_cycles`, optional `max_instability`). Globs are
resolved against the indexed file paths at the composition root into a per-file
layer assignment; the pure `ariadne_graph::fitness_check` consumes the resolved
rules and emits sorted violations.
</decision>

<rationale>
- **Maintainability** — a separate declarative file mirrors ArchUnit's
  `layeredArchitecture` model (layers + "may not be accessed by" directions),
  so the rules read like the architecture they enforce and evolve without
  touching code [src: https://www.baeldung.com/java-archunit-intro].
- **Reliability** — layers as path globs + forbidden *directions* (not
  per-edge allowlists) make the clean self-index pass deterministically while
  a single seeded cross-layer edge fails the check; resolution iterates files
  in `FileId` order with first-declared-layer-wins, so the verdict is
  byte-identical across runs [src:
  .claude/plans/intelligence-platform/block-a/plan.md `<constraints>`].
- **Efficiency** — the engine reuses `coupling_report` / `cycle_report` and
  adds no new metric code (BR5); parsing is a one-shot read at the query
  process, not a graph mutation [src: crates/ariadne-graph/src/coupling.rs,
  crates/ariadne-graph/src/cycles.rs].

Schema (the public contract):

```toml
[[layer]]
name  = "core"
paths = ["crates/ariadne-core/**"]

[[rule]]
forbid = { from = "core", to = "adapter" }

[thresholds]
max_cycles      = 0
max_instability = 0.9   # optional; omit to disable the coupling check
```

A file matching no layer's globs is unlabeled and excluded from the
dependency-direction check. A `forbid` direction whose endpoints both resolve
to layers fails when any inter-file edge crosses it (deduped per file pair).
</rationale>

<alternatives>
- **A `[fitness]` section inside the tool `config.toml`** — rejected: couples
  architecture rules to tool config and grows `config.rs` (Ca 305), against the
  single-responsibility lens [src:
  .claude/plans/intelligence-platform/block-a/plan.md D5].
- **Per-edge allowlists instead of layer directions** — rejected: brittle and
  unreadable at repo scale; ArchUnit's layer-direction model is the proven
  abstraction [src: https://www.baeldung.com/java-archunit-intro].
- **A persisted layer assignment in storage** — rejected: adds a table +
  migration + staleness for data derivable from globs at query time.
</alternatives>

<consequences>
- `ariadne-fitness.toml` at the repo root is a committed public contract; a
  schema change is a breaking change to this ADR.
- `ariadne-mcp` gains the `toml` dependency (it already had `glob`); the shared
  `tools::fitness_report::handle` parses + resolves once for both the MCP
  `fitness_report` tool and the CLI `fitness check` command, so they stay
  parity by construction (mirrors ADR-0027's shared-handle pattern).
- The warm `DaemonQuery::FitnessReport` leg is intentionally deferred: the cold
  catalog suffices for the CI gate and agent queries; a future tier may add it.
- `ariadne fitness check` exits non-zero on any violation, so CI can gate on
  it; the committed repo config encodes this project's hexagonal layers.
- Reach is edge-based: the forbidden-dependency check fires on resolved
  symbol→symbol graph edges crossing a layer; cross-crate type/import-only uses
  that leave no such edge are not caught, so this gate is a narrower signal than
  `tests/architecture.rs` (declared Cargo deps), which stays the authority on
  static crate boundaries. A future tier may source layer rules from
  import/Cargo-dep edges for parity.
</consequences>

<sources>
- `[src: .claude/plans/intelligence-platform/block-a/tier-04-fitness.md]`
- `[src: .claude/plans/intelligence-platform/block-a/plan.md D5]`
- `[src: https://www.baeldung.com/java-archunit-intro]`
- `[src: docs/adr/0027-mcp-parser-dependency.md]` (shared-handle precedent)
- `[src: tests/architecture.rs]` (the invariants this productises)
</sources>
