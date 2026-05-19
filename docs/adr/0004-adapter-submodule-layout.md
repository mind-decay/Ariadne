# ADR-0004: Adapter submodule layout

<status>
Accepted
Date: 2026-05-19
Decider: user
</status>

<context>
[ADR-0001](0001-architecture-style.md) commits the workspace to hexagonal architecture; [`docs/folder-layout.md`](../folder-layout.md) rule 4 originally said "Each adapter file matches one external tech". That phrasing assumed a single `.rs` file per adapter. The tier-02 redb adapter ([`crates/ariadne-storage/src/adapters/redb/`](../../crates/ariadne-storage/src/adapters/redb/)) shipped as a directory (`mod.rs`, `apply.rs`, `snapshot.rs`, `tables.rs`) so each submodule covers a single concern — table definitions, the single-txn write path, the read accessors, and the port impl glue. Audit [tier-02-report.md I1](../../.claude/plans/ariadne-core/audit/tier-02-report.md) flagged the deviation as INFO.

The forces in play:

1. **Maintainability** — putting the write txn, read snapshot, table constants, and trait impls in one 400+ line file makes diffs noisy and obscures the layered structure (port surface → write path → read path → schema).
2. **Reliability** — Rust idiom in projects of comparable size (rust-analyzer, zed, sled) splits adapter internals into focused submodules; reviewers expect that shape.
3. **Encapsulation guarantee preserved** — the public re-export (`pub use adapters::redb::{RedbStorage, RedbWriteTxn, RedbReadSnapshot};`) still exposes exactly one port impl per external tech. No `redb::*` type leaks across the crate boundary.
4. **Single-tech invariant preserved** — the directory name still matches one external tech.
</context>

<decision>
Amend [`docs/folder-layout.md`](../folder-layout.md) rule 4 so that "one external tech per adapter location" is satisfied by either `adapters/<tech>.rs` (a single file) or `adapters/<tech>/` (a submodule directory whose `mod.rs` is the port-impl entry point). The constraint that the underlying crate's types (`redb::Database`, `tree_sitter::Parser`, `prost::Message`) never appear in the public API is unchanged; the constraint that exactly one port implementation lives under that location is unchanged.
</decision>

<rationale>
- **Maintainability** — Splitting the redb adapter dropped average file size from a hypothetical ~350 lines into four cohesive ≤175-line files, each addressing a single concern. Reviewers can read `tables.rs` without scrolling through write-path logic, and a future Tarjan-style change to the read path is isolated to `snapshot.rs`.
- **Reliability** — The encapsulation guarantee (no leaking `redb::*` types past the adapter boundary) is enforced by the `lib.rs` re-export list, not by file granularity. Splitting submodules does not weaken that guarantee; `cargo test --test architecture` continues to pass.
- **Efficiency** — No runtime cost. Submodules compile into the same crate.
- **Scalability** — Future adapters (`ariadne-parser`/tree-sitter, `ariadne-scip`/protobuf) will likely exceed the threshold where a single file harms readability. Codifying the option avoids a recurring ad-hoc decision.
</rationale>

<alternatives>
- **Keep rule 4 literal and flatten redb back into a single file.** Rejected — moves 350+ lines into one file purely to satisfy a wording choice, and would force the same fight for every non-trivial adapter that follows.
- **Allow free-form layout per adapter.** Rejected — the invariant that "one location ↔ one external tech ↔ one port impl" is what audits check; removing it removes the guard.
- **Use `<tech>/lib.rs` style internal crate per adapter.** Rejected — Cargo does not support nested crates inside `src/`, and the workspace already gives each adapter its own crate at the top level.
</alternatives>

<consequences>
- [`docs/folder-layout.md`](../folder-layout.md) rule 4 is updated to permit `adapters/<tech>/` directories. Audits no longer flag the directory form for tier-02's redb adapter.
- Adapter directories must still satisfy: (a) `mod.rs` is the port-impl entry point; (b) the public API re-exports a single port impl + its error type; (c) no underlying-library types appear in the public API; (d) the directory name matches one external tech.
- The "façade `lib.rs`" rule (rule 3) is unaffected — `crates/<crate>/src/lib.rs` remains a re-export-only file.
- Future adapters (tree-sitter, scip subprocess, notify) may use the directory form by default; they must still pass [`tests/architecture.rs`](../../tests/architecture.rs).
- This ADR does not retroactively require splitting existing single-file adapters; the single-file form remains valid.
</consequences>

<sources>
- [ADR-0001 — Hexagonal architecture with TDD](0001-architecture-style.md)
- [`docs/folder-layout.md`](../folder-layout.md)
- [`.claude/plans/ariadne-core/audit/tier-02-report.md`](../../.claude/plans/ariadne-core/audit/tier-02-report.md) (finding I1)
- [`crates/ariadne-storage/src/adapters/redb/`](../../crates/ariadne-storage/src/adapters/redb/)
</sources>
