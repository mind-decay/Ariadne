# ADR-0018: Git History Adapter on gix

<status>
Accepted
Date: 2026-06-01
Decider: claude
</status>

<context>
v1 analytics are static-only — they ignore how code changed over time. Block C
of the post-v1 roadmap adds history-derived signal; tier-11 ingests the
file-level slice (per-file churn + unordered co-change), persisted for the
tier-13 hotspot/coupling metrics [src: .claude/plans/post-v1-roadmap/plan.md
RD7]. Reading Git history is an outbound IO concern, so it must live in a
driven adapter that depends only on `ariadne-core` and never on the daemon or
other adapters (hexagonal invariant) [src: tests/architecture.rs]. The hard
constraint is "pure-Rust on the critical path; no cgo, no C" [src:
.claude/plans/ariadne-core/plan.md D5].
</context>

<decision>
Add a new driven adapter `ariadne-git` backed by `gix` 0.84.0 pinned exactly,
with `default-features = false` and only the local features `blob-diff`,
`revision`, and `sha1`. It walks `head_commit()` → `rev_walk([head]).all()`,
diffs each commit's tree against its first parent via `diff_tree_to_tree`, and
returns owned `ariadne-core` records (`FileChurn`, `CoChangePair`). The CLI
composition root wires it into `ariadne index`; no `gix` type crosses the
crate's public API.
</decision>

<rationale>
- **Reliability / efficiency:** `gix` is pure-Rust Git (Cargo itself depends on
  it), so the critical path takes no curl/C/cgo dependency [src:
  https://lib.rs/crates/gix; plan.md D5]. The walk uses the commit-graph file
  when present (R-C1) [src: https://docs.rs/gix/0.84.0/gix/struct.Repository.html].
- **Maintainability:** the prescribed feature set is the *minimal* local set.
  `blob-diff` enables `diff_tree_to_tree`; `revision` enables `rev_walk`; `sha1`
  selects the pure-Rust RustCrypto object-id backend. `default-features = false`
  drops the network/transport features (`*-network-client`,
  `*-http-transport-*`) that would pull curl/C [src:
  https://docs.rs/crate/gix/0.84.0/features]. RD7 lists only `blob-diff`;
  `revision` + `sha1` are the additional non-network features its own cited API
  (`rev_walk`) and object decoding require — they honour the no-network intent.
- **Scalability:** the walk runs once at index time and persists to redb; large
  commits are excluded from co-change since the pair set is O(n²) and sweeping
  refactors are coupling noise, not signal [src: Tornhill, "Your Code as a Crime
  Scene", 2015]. Bounded commit depth caps the walk on large repos (R-C1).
- Distinct authors are stored as an 8-byte FNV-1a/64 digest *set* (not a count),
  so tier-11a can merge incremental walks by union with no second migration
  [src: plan.md RD7]. FNV-1a is dependency-free and deterministic across runs.
</rationale>

<alternatives>
- **Shell out to `git`** — rejected: breaks "no external runtime", and parsing
  porcelain/plumbing output is fragile. `[src: plan.md RD7]`
- **`git2` / libgit2** — rejected: libgit2 is C, violating D5. `[src: plan.md
  D5, RD7]`
- **A `Git` port trait in `ariadne-core`** — rejected for tier-11: only the CLI
  consumes history and the daemon must not, so a direct composition-root call
  to the adapter is sufficient; a port can be introduced if a second driver
  ever needs history. `[src: tests/architecture.rs; docs/adr/0007-cli-composition-root.md]`
</alternatives>

<consequences>
- `tests/architecture.rs` classifies `ariadne-git` as a driven adapter
  (deps ⊆ {`ariadne-core`}); the daemon's dependency set stays git-free (RD7).
- The redb schema bumps v3 → v4 with one additive `MigrationStep` creating the
  `CHURN` + `CO_CHANGE` tables; pre-existing databases upgrade in place
  [src: docs/adr/0002-tech-stack.md; plan.md RD2].
- `gix` features are now part of the build contract: re-enabling defaults or
  adding a network feature would reintroduce C/curl and requires superseding
  this ADR.
- tier-11a (incremental re-walk via a HEAD-oid watermark) and tier-11b
  (per-symbol attribution, ADR-0019) build on this adapter.
</consequences>

<sources>
- `[src: https://lib.rs/crates/gix]`
- `[src: https://docs.rs/gix/0.84.0/gix/struct.Repository.html]`
- `[src: https://docs.rs/crate/gix/0.84.0/features]`
- `[src: .claude/plans/post-v1-roadmap/plan.md RD7, D5, R-C1, R-C2]`
- `[src: Tornhill, "Your Code as a Crime Scene", 2015]`
</sources>
