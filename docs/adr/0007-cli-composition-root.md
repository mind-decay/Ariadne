# ADR-0007: ariadne-cli is the composition root

<status>
Accepted
Date: 2026-05-20
Decider: user
</status>

<context>
Tier-10 wires every prior tier behind the single `ariadne` binary. Its
`<files>` entry requires `ariadne-cli/Cargo.toml` to carry "workspace deps
+ every internal crate", and steps 5–7 make the CLI link two crates that
tier-00 classified as *driving adapters*: `ariadne-watcher` (`watch` runs
`NotifyWatcher`) and `ariadne-mcp` (`serve` hosts the MCP server; `query`
is an **in-process** call to `AriadneServer`'s tool router)
[src: .claude/plans/ariadne-core/tier-10-cli-e2e.md `<files>` + steps 5-7].

`tests/architecture.rs` rule 4 (a tier-00 invariant, audit-gated) asserts
that *nothing* in the workspace may depend on any crate in
`DRIVING_ADAPTERS = [ariadne-cli, ariadne-mcp, ariadne-watcher]`
[src: tests/architecture.rs lines 38-39, 104-113]. Adding `ariadne-cli →
ariadne-mcp` and `ariadne-cli → ariadne-watcher` fails that assertion. The
rule conflates two distinct hexagonal roles: a *driving adapter* (an entry
point that translates one external protocol into use-case calls) and the
*composition root* (the single place that wires the whole object graph).
`plan.md`'s architecture diagram already names only `ariadne-cli` and
`ariadne-mcp` as driving adapters and places `ariadne-watcher` under "use
cases / orchestration" [src: .claude/plans/ariadne-core/plan.md
`<architecture>`], and `serve.rs` states the watcher loop "sits in the CLI
(tier-10) where both adapters meet" [src: crates/ariadne-mcp/src/serve.rs
lines 9-13]. The forces: maintainability (one coherent invariant) and
reliability (the real protection — keeping the domain free of adapter
edges — must survive).
</context>

<decision>
`ariadne-cli` is the application's **composition root**: the unique entry
point that composes every module together. It may depend on any
in-workspace crate, including the driving adapters `ariadne-mcp` and
`ariadne-watcher`. No other crate may depend on a driving adapter, and no
crate may depend on `ariadne-cli`. `tests/architecture.rs` rule 4 is
amended to encode this split; `docs/folder-layout.md` rule 6 gains a
matching carve-out.
</decision>

<rationale>
- **Maintainability.** The composition root is "a (preferably) unique
  location in an application where modules are composed together",
  positioned at the entry point and never inside a library
  [src: https://blog.ploeh.dk/2011/07/28/CompositionRoot/]. A single
  binary that wires `redb` storage, the parser, the SCIP drivers, the
  graph, the MCP server, and the watcher is exactly that. Modelling it as
  a peer "driving adapter" forced an invariant that contradicts the
  tier-10 contract; naming the role removes the contradiction.
- **Reliability.** The substantive guarantee — `ariadne-core` and the
  use-case / driven-adapter crates never reach an adapter — is unchanged.
  The amended rule still rejects every inward edge; it only exempts the
  one crate whose job is composition. `ariadne-mcp` and `ariadne-watcher`
  still may not depend on each other.
- **Efficiency.** `query` stays an in-process `AriadneServer` call with no
  JSON-RPC handshake or child process, as tier-10 step 7 specifies; the
  alternative (subprocess IPC) would add spawn latency to every CLI query.
- **Scalability.** Future driving adapters (e.g. an HTTP surface) plug in
  under the same rule with no further ADR: they are forbidden inward edges
  and are wired solely by the composition root.
</rationale>

<alternatives>
- **CLI shells out to the `ariadne-mcp` binary.** Rejected — `serve` could
  exec the child, but `query` would lose its in-process path (tier-10 step
  7) and `watch` still needs the `ariadne-watcher` crate, so the conflict
  is not actually resolved. [src: .claude/plans/ariadne-core/tier-10-cli-e2e.md steps 5-7]
- **Leave rule 4 as-is and let the test fail.** Rejected — a red
  architecture invariant is an audit hard-fail and blocks the commit gate
  [src: .claude/settings.json audit-gate hook].
- **Delete rule 4.** Rejected — it still correctly forbids domain and
  use-case crates from importing adapters; only the composition-root case
  needed carving out.
</alternatives>

<consequences>
- `tests/architecture.rs` splits `DRIVING_ADAPTERS` into
  `COMPOSITION_ROOT = "ariadne-cli"` and `DRIVING_ADAPTERS =
  ["ariadne-mcp", "ariadne-watcher"]`; rule 4 forbids any dependency on
  the composition root and forbids depending on a driving adapter unless
  the dependent *is* the composition root.
- `docs/folder-layout.md` rule 6 carries a one-clause amendment citing
  this ADR.
- `ariadne-cli/Cargo.toml` depends on every internal crate; the
  architecture test stays green.
- Any second composition root (a separate release binary) requires a
  superseding ADR — there is exactly one today.
</consequences>

<sources>
- `[src: .claude/plans/ariadne-core/tier-10-cli-e2e.md `<files>`, steps 5-7]`
- `[src: .claude/plans/ariadne-core/plan.md `<architecture>`, `<decisions>` D13]`
- `[src: tests/architecture.rs lines 38-39, 104-113]`
- `[src: docs/folder-layout.md rule 6]`
- `[src: crates/ariadne-mcp/src/serve.rs lines 9-13]`
- `[src: https://blog.ploeh.dk/2011/07/28/CompositionRoot/]`
- `[src: docs/adr/0001-architecture-style.md]`
</sources>
</output>
