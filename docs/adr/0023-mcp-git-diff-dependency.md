# ADR-0023: MCP Links the Git Diff Adapter; the Daemon Does Not

<status>
Accepted
Date: 2026-06-02
Decider: claude
</status>

<context>
Tier-15c wires `diff_blast_radius` — "what does *this change* affect" — onto the
tier-14 pieces: `ariadne_git::diff(root, spec)` resolves a `DiffSpec`
(`WorkingTree | Commit | RefRange`) to `(hunks, changed_paths)`, and
`GraphIndex::diff_blast` joins those line hunks to the symbol graph [src:
crates/ariadne-git/src/adapters/gix/diff.rs:38; crates/ariadne-graph/src/diff_blast.rs:81].

The diff read must run where `ariadne-git` is linked. The warm daemon owns the
graph (RD6), so the obvious move is to compute the diff in the daemon — but RD7
bars the daemon from depending on `ariadne-git`: the daemon stays a thin graph
host, and git history is ingested at the CLI composition root, never linked into
the long-running process [src: .claude/plans/post-v1-roadmap/plan.md RD7;
docs/adr/0018-git-history-adapter.md]. The MCP server, by contrast, is already a
cold composition root — it wires the driven `ariadne-storage` adapter for its
cold-fallback path [src: crates/ariadne-mcp/Cargo.toml; docs/adr/0007-cli-composition-root.md].
</context>

<decision>
`ariadne-mcp` depends on `ariadne-git`; `ariadne-daemon` does not.

The `diff_blast_radius` `#[tool]` runs `ariadne_git::diff(&root, &spec)` in the
MCP process, then routes the **result**, not the spec:
- **Warm path** — sends `DaemonQuery::DiffBlast { hunks, changed_paths, depth,
  kinds }` over the local socket. `LineHunk` already lives in `ariadne-core`
  (tier-11b), so only pure data crosses the wire. The daemon builds the
  `FileSymbolSpans` from its warm symbols + the changed files' bytes and runs
  `GraphIndex::diff_blast` — it never sees a `DiffSpec`, never opens the repo,
  never links git.
- **Cold path** — when no daemon answers, the MCP server builds the `Catalog`
  and runs `diff_blast` in-process, exactly like the other cold-fallback tools.

The span build (group changed-file symbols, read on-disk bytes, drop on `blake3`
mismatch, line-index) is the shared `ariadne_graph::spans_from` helper, so the
CLI, daemon, and cold-MCP paths agree (tier-15c D3).
</decision>

<rationale>
- **Maintainability (RD7 preserved):** the daemon's dependency set is unchanged
  — it receives pre-computed hunks and stays git-free, so the "daemon is a thin
  graph host" invariant holds. A `tests/architecture.rs` assertion pins it:
  `ariadne-daemon` must not depend on `ariadne-git`.
- **Maintainability (hexagonal):** `ariadne-mcp → ariadne-git` is a driving →
  driven edge, which the architecture invariant already permits — the same
  composition-root pattern by which the MCP server links `ariadne-storage` for
  its cold path (ADR-0007). `ariadne-git` itself stays `deps ⊆ {core}`, and
  nothing depends on a driving adapter, so the invariant is intact [src:
  tests/architecture.rs].
- **Efficiency:** the warm path sends only the diff (a handful of hunks +
  paths); the daemon already holds the symbols, so re-sending whole
  `FileSymbolSpans` would be redundant.
- **Reliability:** `diff_blast` and the adapter's `diff` are pure; the must∪may
  union equals the per-seed `blast_radius` union (the tier-14 invariant,
  re-asserted through the live tool). A file stale against its index degrades to
  `unresolved`, never a wrong seed (D3).
</rationale>

<alternatives>
- **Git in the daemon** — rejected: violates RD7 and the adapter-isolation
  precedent; pulls `gix` into the always-warm process for a per-call read.
- **A fourth driving adapter just to run git** — rejected: new surface for
  nothing; the MCP server is already a composition root that can take the dep.
- **Send the whole `FileSymbolSpans` over the wire** — rejected: the daemon
  already holds the symbols; only the client-side diff is new information.
- **Cold-only (build a graph per call)** — rejected: pays a full graph build per
  invocation and abandons the warm-daemon routing every other tool uses.
</alternatives>

<consequences>
- `ariadne-mcp` gains an `ariadne-git` dependency (workspace path dep);
  `ariadne-core` gains `DaemonQuery::DiffBlast` + `DaemonResponse::DiffBlast`
  with `DiffBlastReport` / `DiffSeed` mirror DTOs.
- `tests/architecture.rs` stays green and gains a clause asserting
  `ariadne-daemon` does not depend on `ariadne-git`.
- The MCP `diff_blast_radius` input layer owns a `JsonSchema`-deriving
  `DiffSpecInput`, mapped to `ariadne_core::DiffSpec` at the handler; core stays
  `schemars`-free (D4), as it already does for `EdgeKindFilter`.
- The tool catalog reaches 17; the handshake snapshot is re-accepted.
</consequences>

<sources>
- `[src: .claude/plans/post-v1-roadmap/plan.md RD6, RD7]`
- `[src: .claude/plans/post-v1-roadmap/tier-15c-diff-blast-radius-tool.md]`
- `[src: docs/adr/0007-cli-composition-root.md]`
- `[src: docs/adr/0015-daemon-mode-ipc.md]`
- `[src: docs/adr/0018-git-history-adapter.md]`
- `[src: docs/adr/0022-diff-aware-blast-radius.md]`
- `[src: tests/architecture.rs — adapter-isolation invariant + daemon-no-git clause]`
- `[src: crates/ariadne-git/src/adapters/gix/diff.rs:38]`
- `[src: crates/ariadne-graph/src/diff_blast.rs:81]`
</sources>
