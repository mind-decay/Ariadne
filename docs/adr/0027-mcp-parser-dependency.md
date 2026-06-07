# ADR-0027: MCP Links the Parser Adapter; A2 Has No Daemon Leg

<status>
Accepted
Date: 2026-06-07
Decider: claude
</status>

<context>
Block A's A2 (`api_surface_diff`) classifies the public-surface delta between two
refs as a `SemverBump` (none / patch / minor / major) per the Cargo SemVer
taxonomy [src: <https://doc.rust-lang.org/cargo/reference/semver.html>]. The
classifier is pure over two `PublicSymbol` lists [src:
crates/ariadne-graph/src/api_surface.rs], but assembling those lists requires
git + parser work: resolve the diff between the refs, read each changed file's
blob at each ref, and re-extract its public surface with tree-sitter [src:
.claude/plans/intelligence-platform/block-a/plan.md D3/D4].

That assembly must run where both `ariadne-git` and `ariadne-parser` are linked.
Unlike `diff_blast_radius`, the answer needs no warm graph — it is a pure
function of the two surfaces — so there is no reason to involve the daemon, and
involving it would force `ariadne-git` into the always-warm process, breaking the
tested `ariadne-daemon ↛ ariadne-git` invariant (RD7 / ADR-0023) [src:
tests/architecture.rs:108-154; docs/adr/0023-mcp-git-diff-dependency.md]. The MCP
server is already a cold composition root linking the driven `ariadne-storage`
and `ariadne-git` adapters [src: crates/ariadne-mcp/Cargo.toml].
</context>

<decision>
`ariadne-mcp` depends on `ariadne-parser`, and `api_surface_diff` runs entirely
in the querying process (MCP server / CLI) with no `DaemonQuery` variant.

The `api_surface_diff` `#[tool]` (and the `ariadne api-diff` CLI command) run the
whole composition in-process: `ariadne_git::diff` → `ariadne_git::read_blobs_at`
at each ref → `ariadne_parser::public_surface` per changed source blob →
`ariadne_graph::api_surface_diff`. A single `tools::api_surface_diff::handle`
holds this composition; both the MCP server and the CLI call it, so their output
is parity by construction. The daemon is never consulted and stays git-free.
</decision>

<rationale>
- **Maintainability (hexagonal):** `ariadne-mcp → ariadne-parser` is a driving →
  driven edge, which the architecture invariant already permits — the same
  composition-root pattern by which the MCP server links `ariadne-storage` and
  `ariadne-git` (ADR-0007, ADR-0023). `ariadne-parser` itself stays
  `deps ⊆ {core}`, and nothing depends on a driving adapter, so the invariant is
  intact [src: tests/architecture.rs:40-45,124-147].
- **Maintainability (RD7 preserved):** with no daemon leg, the daemon's
  dependency set is unchanged and the `ariadne-daemon ↛ ariadne-git` invariant
  holds untouched [src: tests/architecture.rs:148-154].
- **Reliability:** one shared `handle` means the MCP and CLI surfaces compute the
  verdict and lists identically; there is no second code path to drift. The
  classifier and the adapters' reads are pure, so re-runs are byte-identical.
- **Efficiency:** the read is bounded to the diff's changed files (D4) — only a
  changed file can change the public surface — so no full re-index of the base
  ref is paid (AR2 / BR3).
</rationale>

<alternatives>
- **A `DaemonQuery::ApiSurfaceDiff` warm leg** — rejected: the answer needs no
  warm graph, and routing through the daemon would force `gix` (and the parser)
  into the always-warm process, breaking the tested daemon-git-free invariant
  [src: docs/adr/0023-mcp-git-diff-dependency.md; tests/architecture.rs:148-154].
- **A persisted prior-surface snapshot** — rejected: adds a table + migration +
  a staleness surface; re-parsing the base blobs of changed files only is bounded
  and never stale (D4) [src: block-a plan.md D4].
- **A fourth driving adapter just to run git+parser** — rejected: new surface for
  nothing; the MCP server is already a composition root that can take the deps
  (mirrors ADR-0023's reasoning for `ariadne-git`).
</alternatives>

<consequences>
- `ariadne-mcp` gains an `ariadne-parser` dependency (workspace path dep);
  `ariadne-core`/`ariadne-daemon` are untouched, so no new `DaemonQuery` /
  `DaemonResponse` variant and no daemon git/parser edge.
- `tests/architecture.rs` stays green unchanged: `ariadne-parser` is a driven
  adapter, so `ariadne-mcp → ariadne-parser` passes the driving-adapter
  containment clause, and the daemon-no-git clause still holds.
- The MCP `api_surface_diff` input layer owns a `JsonSchema`-deriving
  `ApiSurfaceDiffInput { base, head }` mapped at the handler; `ariadne-core` and
  `ariadne-graph` stay `schemars`-free.
- The tool catalog reaches 21; the handshake snapshots are re-accepted.
</consequences>

<sources>
- `[src: .claude/plans/intelligence-platform/block-a/plan.md D3/D4/D6]`
- `[src: docs/adr/0007-cli-composition-root.md]`
- `[src: docs/adr/0023-mcp-git-diff-dependency.md]`
- `[src: tests/architecture.rs — adapter-isolation invariant + daemon-no-git clause]`
- `[src: crates/ariadne-git/src/adapters/gix/diff.rs:38 ; crates/ariadne-git/src/adapters/gix/blobs.rs:31]`
- `[src: crates/ariadne-parser/src/adapters/treesitter/surface.rs:31]`
- `[src: crates/ariadne-graph/src/api_surface.rs]`
- `[src: https://doc.rust-lang.org/cargo/reference/semver.html]`
</sources>
