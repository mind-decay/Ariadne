# Contributing to Ariadne

This repo is built tier-by-tier under a strict spec lifecycle. Before opening a PR, read [`CLAUDE.md`](CLAUDE.md), [`docs/architecture.md`](docs/architecture.md), and the active tier file under [`.claude/plans/ariadne-core/`](.claude/plans/ariadne-core/).

## Dev environment

Required toolchain (pure-Rust, no Node, no JVM, no cgo on the critical path — [ADR-0002](docs/adr/0002-tech-stack.md)):

| Tool | Min version | Install |
| --- | --- | --- |
| Rust toolchain | 1.85 (edition 2024 stable) | https://rustup.rs |
| `cargo-nextest` | latest | `cargo install cargo-nextest --locked` |
| `cargo-deny` | latest | `cargo install cargo-deny --locked` |
| `cargo-audit` | latest | `cargo install cargo-audit --locked` |
| `cocogitto` (`cog`) | latest | `cargo install cocogitto --locked` |
| `lefthook` | latest | `cargo install lefthook --locked` (or see https://lefthook.dev/) |

Optional but recommended:

| Tool | Use |
| --- | --- |
| `actionlint` | local lint of `.github/workflows/*.yml` |
| `heaptrack` | per-tier memory probes for Salsa (R1) |

## Hooks

After cloning, install lefthook once:

```sh
lefthook install
cog install-hook commit-msg
```

`lefthook install` activates the `pre-commit` (fmt + clippy on staged Rust files) and `commit-msg` (cog verify) hooks. Bypass via `LEFTHOOK=0 git commit …` only when explicitly justified.

## Commit format

Conventional Commits v1.0.0 — see [ADR-0003](docs/adr/0003-commit-convention.md).

```
<type>(<scope>)<!>: <subject>
```

Allowed `type` values:

| type | meaning |
| --- | --- |
| `feat` | user-visible new capability |
| `fix` | bug fix |
| `docs` | documentation only |
| `style` | formatting/whitespace only |
| `refactor` | no behaviour change |
| `perf` | performance change without API change |
| `test` | tests only |
| `build` | build system or external deps |
| `ci` | CI/CD config |
| `chore` | repo housekeeping |
| `revert` | reverts a prior commit |

Allowed `scope` values (mirrors crates without the `ariadne-` prefix, plus three cross-cutting):

`core`, `storage`, `parser`, `scip`, `graph`, `salsa`, `watcher`, `mcp`, `cli`, `e2e`, `docs`, `ci`, `deps`.

Subject ≤72 characters, imperative mood. Breaking changes use `!` after the scope or a `BREAKING CHANGE:` footer.

Examples:

```
feat(graph): add Tarjan SCC traversal
fix(storage)!: change FileId layout to 64-bit content hash
chore(deps): bump rmcp to 1.7.0
```

CI gates this three ways: local `cog verify` hook, the `commits` CI job (`cog check origin/main..HEAD`), and the `pr-title` action for squash merges.

## Spec lifecycle

Every code change flows through three separate Claude sessions:

1. `/spec-plan` — produces `.claude/plans/<slug>/plan.md` (and tier files for multi-tier work).
2. `/spec-build <path-to-tier-or-plan>` — implements exactly one tier.
3. `/spec-audit <path-to-tier-or-plan>` — pedantic review. Writes `audit/<id>-report.md` and `audit-state.json`. The audit gate hook blocks commits/pushes until the latest audit reads `PASS` against the current HEAD.

Tier execution order is fixed in the plan's frontmatter `tiers:` field. A tier session refuses to start until its `deps` are all `status: completed`.

## Adding a crate

1. Confirm the new crate matches the canonical layout in [`docs/folder-layout.md`](docs/folder-layout.md).
2. Add it to the workspace `members` list (root `Cargo.toml`, tier-01).
3. Add its scope to [`cog.toml`](cog.toml) `[packages]` AND to the `pr-title` job in [`.github/workflows/ci.yml`](.github/workflows/ci.yml).
4. Update the crate table in [`docs/architecture.md`](docs/architecture.md).
5. Write a failing test before any implementation (TDD per [ADR-0001](docs/adr/0001-architecture-style.md)).

## Adding a port

1. Add the trait to `ariadne-core::domain::ports`. Signatures use only domain types.
2. Default the trait `Error` to a `thiserror` enum in `ariadne-core::errors`.
3. Implement the trait in exactly one driven-adapter crate. The adapter re-exports its impl + constructor; the underlying library's types do not leak.
4. Add an integration test under `crates/ariadne-<adapter>/tests/` against a real fixture.

## Writing an ADR

1. Copy [`docs/adr/_template.md`](docs/adr/_template.md) to the next sequential number: `docs/adr/NNNN-kebab-title.md`.
2. Fill in Status, Context, Decision, Rationale, Alternatives, Consequences, Sources.
3. Cite every external claim inline with `[src: <url-or-repo-path>]`.
4. Link the ADR from the plan or tier file that motivated it.

## Audit gate

The hook at `.claude/hooks/audit-gate.sh` blocks `git commit` and `git push` when the most-recent `audit-state.json` reads `FAIL`, or when HEAD has advanced past the audited commit. Re-run `/spec-audit <tier>` on the current HEAD to re-arm the gate.

## Reporting

- Open issues for v1 risks (see plan `<risks>` table) under the `risk-rN` labels.
- Memory probes for tier-04+ go in the PR description under `## Memory probe`, in the format `table=<name> bytes=<n> delta=<n>`.
