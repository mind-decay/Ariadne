# ADR-0003: Conventional Commits + per-crate scopes, enforced by cocogitto

<status>
Accepted
Date: 2026-05-19
Decider: user
</status>

<context>
Ariadne is a multi-crate workspace with a release pipeline (tier-10) and per-crate semver (tier-08 `rmcp` API ceiling, tier-02 redb on-disk schema). The release pipeline needs:

1. machine-readable history for changelog generation per crate,
2. deterministic semver-bump inference (`feat` → minor, `fix` → patch, `!` or `BREAKING CHANGE:` → major),
3. pre-merge enforcement so junk commit messages never land,
4. a single tool that works on both local commits (commit-msg hook) and squash-merge PR titles.

The toolchain is pure-Rust ([ADR-0002](0002-tech-stack.md) D5 / D14); a Node-only commitlint binary would drag in a runtime we otherwise avoid on the critical path.
</context>

<decision>
Adopt **Conventional Commits v1.0.0** with a fixed type allowlist and per-crate scopes, enforced by **cocogitto**.

Format: `<type>(<scope>)<!>: <subject>` `[src: https://www.conventionalcommits.org/en/v1.0.0/]`.

Allowed types:
`feat`, `fix`, `docs`, `style`, `refactor`, `perf`, `test`, `build`, `ci`, `chore`, `revert`.

Allowed scopes (crate names without the `ariadne-` prefix, plus three cross-cutting):
`core`, `storage`, `parser`, `scip`, `graph`, `salsa`, `watcher`, `mcp`, `cli`, `e2e`, `docs`, `ci`, `deps`.

Subject ≤72 characters, imperative mood. Breaking change is signalled by `!` after the scope (`feat(storage)!: …`) or by a `BREAKING CHANGE:` footer.

Enforcement layers:

1. **Local** — `lefthook.yml`'s `commit-msg` runs `cog verify --file {1}` on the in-progress message; the hook is installed via `lefthook install`. Bypass only via `LEFTHOOK=0`.
2. **CI on PRs** — `.github/workflows/ci.yml` job `commits` installs cocogitto and runs `cog check origin/${{ github.base_ref }}..HEAD`.
3. **PR title** — `.github/workflows/ci.yml` job `pr-title` runs `amannn/action-semantic-pull-request@v5` with the same type + scope allowlists, so squash-merge titles get the same gate `[src: https://github.com/amannn/action-semantic-pull-request]`.
4. **Changelog** — `cog changelog` is wired into tier-10's release pipeline; the `[commit_types]` and `[packages]` blocks in [`cog.toml`](../../cog.toml) determine the per-crate changelog format.
</decision>

<rationale>
- **Reliability** — Conventional Commits is the industry standard with the largest ecosystem `[src: https://www.conventionalcommits.org/en/v1.0.0/]`. Three enforcement layers (local hook + CI commits + PR-title action) ensure no malformed message lands regardless of merge strategy.
- **Efficiency** — cocogitto is a single Rust binary that handles verify, check, changelog, and bump in one tool `[src: https://docs.cocogitto.io/]`. No Node runtime is added.
- **Maintainability** — Scopes mirror the crate list 1:1, so PR scope = blast radius at a glance. The `cog.toml` `[packages]` block declares the same list once for changelog generation and for scope validation, eliminating drift.
- **Scalability** — As crates are added (post-v1: cross-repo, IDE plugins), the scope allowlist grows by appending entries to `cog.toml`. No infrastructure change.
</rationale>

<alternatives>
- **Gitmoji** — rejected. Emoji-only types do not encode semver bump intent; changelog grouping is ad-hoc `[src: ../../.claude/plans/ariadne-core/plan.md D14]`.
- **Free-form commit messages** — rejected. No auto-changelog, no semver gating, audits become subjective.
- **commitlint-rs** — rejected. Smaller scope than cocogitto (no changelog/bump tooling); we would still need a second tool for the release pipeline.
- **JS commitlint** — rejected. Drags Node into a pure-Rust toolchain; conflicts with ADR-0002 D5/D14 `[src: ../../.claude/plans/ariadne-core/plan.md D14]`.
</alternatives>

<consequences>
- `cog.toml` at repo root is the canonical type/scope allowlist; updating either field requires touching it.
- `lefthook.yml` is required for the local commit-msg hook to fire; `CONTRIBUTING.md` documents the install step.
- The CI `commits` job blocks PRs with malformed messages; squash-merge maintainers must keep the PR title valid because the merge commit inherits it.
- Tier-10's release pipeline calls `cog changelog` per-package; if a package is added without a `[packages.<scope>]` entry in `cog.toml`, the release fails. This is intentional — it forces a single point of truth.
- Manual amendments to past commits are allowed only on feature branches before merge; once on `main`, history is immutable.
</consequences>

<examples>

```
feat(graph): add Tarjan SCC traversal for cycle detection
fix(storage)!: change FileId layout to 64-bit content hash

  BREAKING CHANGE: existing .ariadne/index.redb must be rebuilt.
chore(deps): bump rmcp to 1.7.0
docs(core): clarify Storage port contract for read txn semantics
test(salsa): add proptest for incremental rebuild parity
ci(docs): publish rustdoc to gh-pages on tag
```

</examples>

<sources>
- [Conventional Commits v1.0.0](https://www.conventionalcommits.org/en/v1.0.0/)
- [cocogitto](https://github.com/cocogitto/cocogitto); [cocogitto docs](https://docs.cocogitto.io/)
- [amannn/action-semantic-pull-request](https://github.com/amannn/action-semantic-pull-request)
- [`.claude/plans/ariadne-core/plan.md` D14](../../.claude/plans/ariadne-core/plan.md)
- [ADR-0002 — Tech stack](0002-tech-stack.md)
</sources>
