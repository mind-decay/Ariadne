# Canonical per-crate folder layout

<purpose>
Every `ariadne-*` crate uses the layout below. This is what tier-00's `tests/architecture.rs` invariant + `docs/architecture.md` invariants assume. Deviations need an ADR.
</purpose>

<layout>

```
crates/ariadne-<name>/
  Cargo.toml
  src/
    lib.rs              façade — pub use domain::*; pub use adapters::*; no logic
    domain/             pure core. zero IO. zero external crates beyond domain primitives
      mod.rs
      types.rs          domain entities, ids, value objects
      ports.rs          trait definitions (only inside ariadne-core)
      service.rs        pure use-case functions (graph algos, validation, derivation)
    adapters/           IO implementations of ports. one location per external tech
      mod.rs
      <tech>.rs         single-file form, e.g. treesitter.rs, scip_subprocess.rs, notify.rs
      <tech>/           directory form (ADR-0004); mod.rs is the port-impl entry point
        mod.rs
        ...             tech-internal submodules, e.g. tables.rs, apply.rs, snapshot.rs
    errors.rs           thiserror enum. anyhow::Error allowed only in cli + e2e crates
  tests/                integration tests against real adapters + real fixtures
  benches/              criterion benches (perf gates per tier)
  fixtures/             test data. license-clean. ≤1 MB per file
```

</layout>

<rules>
Hard rules — violations are audit hard-fails:

1. `ariadne-core` has only `src/domain/`. No `src/adapters/`. Zero in-workspace deps.
2. Adapter crates depend on `ariadne-core` only; never on each other. (`ariadne-salsa` is the one exception: it depends on `ariadne-core` and `ariadne-storage`, because Salsa orchestration owns the storage call-site.)
3. `src/lib.rs` re-exports only. No `fn`, no `impl`, no inline logic.
4. Each adapter location matches one external tech and re-exports a single port implementation plus its error type. A "location" is either `adapters/<tech>.rs` (single file) or `adapters/<tech>/` (submodule directory whose `mod.rs` is the port-impl entry point) — see [ADR-0004](adr/0004-adapter-submodule-layout.md). The underlying crate's types (`redb::Database`, `tree_sitter::Parser`, `prost::Message`) never leak into the public API.
5. `errors.rs` uses `thiserror::Error` for the crate's public error enum. `anyhow::Error` is permitted only inside `ariadne-cli` (binary entrypoint) and `ariadne-e2e` (harness). All other crates return concrete `thiserror` enums or `Result<T, OurError>`.
6. Driving adapter crates may depend on use-case crates; driven adapter crates may not.
7. Test code lives in `tests/` (integration) or as `#[cfg(test)] mod tests` next to the unit under test. In-memory fakes for port traits are allowed for unit tests of pure domain logic; module-boundary mocks are not ([plan `<constraints>`](../.claude/plans/ariadne-core/plan.md)).
</rules>

<adding-a-crate>
Checklist for introducing a new crate `ariadne-<name>`:

1. Add it to the workspace `members` list in the root `Cargo.toml` (tier-01).
2. Scaffold the directory above. Run `cargo fmt`, `cargo clippy --workspace -- -D warnings`.
3. Decide its hexagonal position:
   - Domain interior → no `adapters/` folder. Lives under interior set.
   - Use case → depends on `ariadne-core`. May depend on `ariadne-storage` if it persists derived state.
   - Driven adapter → depends on `ariadne-core` only. One external tech per file.
   - Driving adapter → may depend on use cases + `ariadne-core`.
4. Add the crate's scope to [`cog.toml`](../cog.toml) `[packages]` and to the type/scope allowlist in [`.github/workflows/ci.yml`](../.github/workflows/ci.yml) PR-title job ([ADR-0003](adr/0003-commit-convention.md)).
5. Add the failing test (`tests/` or `#[cfg(test)]`) before any implementation. TDD per [ADR-0001](adr/0001-architecture-style.md).
6. Update `docs/architecture.md` crate table.
7. Update `docs/folder-layout.md` only if the layout itself changes (then ADR).
</adding-a-crate>

<adding-a-port>
1. Add the trait to `ariadne-core::domain::ports`. Use only domain types in its signature.
2. Default the trait `Error` to a `thiserror` enum in `ariadne-core::errors`.
3. Add an in-memory `Fake<Port>` under `ariadne-core::domain::ports::fakes` only if a unit test in another domain module needs it.
4. Implement the trait in exactly one driven-adapter crate. The adapter's public API re-exports the impl plus a constructor.
5. Add an integration test under `crates/ariadne-<adapter>/tests/` that exercises the real adapter against a fixture.
</adding-a-port>

<sources>
- [`docs/architecture.md`](architecture.md)
- [ADR-0001 — Architecture style](adr/0001-architecture-style.md)
- [`.claude/plans/ariadne-core/plan.md`](../.claude/plans/ariadne-core/plan.md)
</sources>
