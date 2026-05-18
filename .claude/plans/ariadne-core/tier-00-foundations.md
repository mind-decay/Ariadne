---
tier_id: tier-00
title: Foundations — architecture style, folder layout, CI/CD, lint, ADR, governance
deps: []
exit_criteria:
  - docs/architecture.md exists with hexagonal layout, port/adapter mapping per crate, dependency graph.
  - docs/adr/0001-architecture-style.md (ADR for hexagonal + TDD) + ADR template at docs/adr/_template.md.
  - docs/folder-layout.md fixes the per-crate internal layout (domain/, adapters/, errors.rs, tests/, benches/, fixtures/).
  - .github/workflows/ci.yml runs on push + PR: fmt, clippy --deny warnings, nextest, deny, bench-build, doc-build, audit, msrv, arch-invariants, commits (cocogitto), pr-title (semantic-pull-request).
  - .github/workflows/release.yml scaffolded with cargo-dist for x86_64/aarch64 × linux/macos/windows (no-op until tier-10 binaries exist).
  - Lint configs committed: rustfmt.toml, clippy.toml (pedantic+nursery selective), deny.toml (licenses + advisories + bans).
  - Commit convention enforced: cog.toml at repo root with allowed types + scopes; lefthook commit-msg hook runs `cog verify`; CI job `cog check`; PR-title action validates squash titles.
  - Pre-commit hook via lefthook: fmt + clippy on staged files; bypassable only with explicit env var.
  - CODEOWNERS, CONTRIBUTING.md, PULL_REQUEST_TEMPLATE.md, dependabot.yml committed.
  - Architecture invariant test scaffolded: forbids ariadne-core depending on any adapter crate (cargo-deny `bans.deny` rule + a tests/architecture.rs walk).
status: completed
completed: 2026-05-19
---

<context>
Foundation tier. Zero application code. Locks the architectural style (Hexagonal / Ports & Adapters [src: https://alistair.cockburn.us/hexagonal-architecture/]) and TDD discipline as commitments enforced by CI + ADR + folder layout. Without this tier, every downstream tier risks drifting into ad-hoc layering.
Hexagonal in Rust maps naturally to trait-based DI: ports = traits in `ariadne-core`; adapters = crates implementing them [src: https://www.howtocodeit.com/guides/master-hexagonal-architecture-in-rust].
</context>

<files>
- docs/architecture.md — system overview, hexagonal port/adapter mapping per crate, dataflow diagram (Mermaid).
- docs/folder-layout.md — canonical internal layout for any new crate; what goes in domain/ vs adapters/.
- docs/adr/_template.md — ADR template (status, context, decision, consequences, sources).
- docs/adr/0001-architecture-style.md — first ADR fixing hexagonal + TDD; cites sources.
- docs/adr/0002-tech-stack.md — pins the D1–D12 decisions from plan.md into one place for new contributors.
- docs/adr/0003-commit-convention.md — ADR fixing Conventional Commits + per-crate scopes + cocogitto enforcement (D14).
- cog.toml — cocogitto config at repo root: allowed types, scope allowlist, pre/post bump hooks, changelog template, branch_whitelist [src: https://docs.cocogitto.io/config/].
- rustfmt.toml — edition 2024, max_width 100, group_imports, imports_granularity = "Crate".
- clippy.toml — `msrv` pinned, `cognitive-complexity-threshold = 25`, `too-many-arguments-threshold = 5`.
- deny.toml — `[licenses] allow = ["MIT","Apache-2.0","BSD-3-Clause","ISC","Unicode-DFS-2016"]`; `[bans] multiple-versions = "warn"`; `[advisories] yanked = "deny"`.
- .github/workflows/ci.yml — jobs as listed in exit_criteria.
- .github/workflows/release.yml — cargo-dist scaffolding (`dist init` output), no-op until tagged.
- .github/CODEOWNERS — default owner = repo owner; per-crate owners reserved for post-v1.
- .github/PULL_REQUEST_TEMPLATE.md — checklist: tests added (TDD), ADR if architectural, docs updated, audit gate passes.
- .github/dependabot.yml — cargo + github-actions ecosystems, weekly.
- CONTRIBUTING.md — how to add a crate (must follow folder-layout.md), how to add a port/adapter, how to write an ADR.
- lefthook.yml — pre-commit: `cargo fmt --check`, `cargo clippy --workspace -- -D warnings` on staged Rust files.
- tests/architecture.rs (repo-root integration test) — parses `cargo metadata`, asserts no edge from `ariadne-core` to any other workspace crate; asserts every `ariadne-*` adapter crate depends on `ariadne-core`.
</files>

<steps>
1. **Failing test first** (tests/architecture.rs at repo root, registered as workspace integration crate or inside ariadne-core later): use `cargo_metadata` crate to load workspace, assert: (a) `ariadne-core` has zero workspace-internal dependencies; (b) `ariadne-graph` and `ariadne-salsa` depend only on `ariadne-core` and `ariadne-storage` (read-only port); (c) `ariadne-mcp` and `ariadne-cli` are not depended on by any other crate. Fails until workspace exists (tier-01) — that is OK; commit the failing test so tier-01 inherits the invariant.
2. Write `docs/architecture.md` (Mermaid `flowchart LR`):
   ```
   [CLI] [MCP client]            ─┐ driving (inbound) adapters
   ariadne-cli  ariadne-mcp       │
        │             │            │
        └─── use cases (ariadne-graph + ariadne-salsa) ───┐
                     │                                     │ HEXAGON INTERIOR
                     ▼                                     │
                ariadne-core (ports: Storage, Parser,      │
                              Indexer, Watcher, Sink)      │
                     ▲                                     │
   ┌─────────────────┴─────────────────┐                  │
   │ ariadne-storage (redb)            │ driven           │
   │ ariadne-parser (tree-sitter)      │ (outbound)       │
   │ ariadne-scip (subprocess+proto)   │ adapters         │
   │ ariadne-watcher (notify-rs)       │                  │
   └───────────────────────────────────┘                  ─┘
   ```
   Map each crate's hexagonal role explicitly; reference [src: https://alistair.cockburn.us/hexagonal-architecture/, https://www.howtocodeit.com/guides/master-hexagonal-architecture-in-rust].
3. Write `docs/folder-layout.md`:
   ```
   crates/ariadne-<name>/
     Cargo.toml
     src/
       lib.rs              façade: pub use domain::*; pub use adapters::*;
       domain/             pure core, no IO, no external crates beyond core types
         mod.rs
         types.rs          domain entities
         ports.rs          trait definitions (only for ariadne-core)
         service.rs        pure use-case functions
       adapters/           IO implementations (one file per external tech)
         mod.rs
         <tech>.rs         e.g., redb.rs, treesitter.rs, scip_subprocess.rs
       errors.rs           thiserror enum, never `anyhow::Error` in public API
     tests/                integration tests (use real adapters)
     benches/              criterion benches
     fixtures/             test data, license-clean
   ```
   Hard rules: (i) ariadne-core has only `domain/`, no `adapters/`; (ii) adapter crates depend on ariadne-core, never on each other; (iii) `lib.rs` only re-exports — no logic.
4. Write `docs/adr/_template.md`: `Status | Context | Decision | Consequences | Sources`. ADRs numbered sequentially `NNNN-kebab-title.md`.
5. Write `docs/adr/0001-architecture-style.md` fixing Hexagonal + TDD with citations to Cockburn 2005 and the Rust guide.
6. Write `docs/adr/0002-tech-stack.md` pinning D1–D12 from plan.md.
7. `rustfmt.toml`: `edition = "2024"`, `max_width = 100`, `imports_granularity = "Crate"`, `group_imports = "StdExternalCrate"`, `wrap_comments = true` [src: https://rust-lang.github.io/rustfmt/].
8. `clippy.toml`: `msrv = "1.83"` (or whatever rust-toolchain pin), `cognitive-complexity-threshold = 25`. In each crate's `Cargo.toml`: `[lints.clippy] pedantic = "warn"`, `nursery = "warn"`, `unwrap_used = "deny"`, `expect_used = "warn"`, `panic = "deny"` [src: https://rust-lang.github.io/rust-clippy/master/].
9. `deny.toml`: licenses allowlist (MIT/Apache-2.0/BSD-3-Clause/ISC/Unicode-DFS-2016); `[advisories] vulnerability = "deny"`, `yanked = "deny"`; `[bans] multiple-versions = "warn"`; `[sources] unknown-registry = "deny"`, `unknown-git = "deny"` [src: https://embarkstudios.github.io/cargo-deny/].
10. `.github/workflows/ci.yml` jobs (matrix linux + macos; cache `~/.cargo` and `target/` via `Swatinem/rust-cache@v2`):
    - `fmt`: `cargo fmt --all --check`
    - `clippy`: `cargo clippy --workspace --all-targets --all-features -- -D warnings`
    - `test`: `cargo nextest run --workspace --profile ci`
    - `deny`: `cargo deny check`
    - `audit`: `cargo audit` (advisory db)
    - `docs`: `cargo doc --workspace --no-deps --document-private-items` with `RUSTDOCFLAGS=-D warnings`
    - `bench-build`: `cargo bench --workspace --no-run`
    - `msrv`: pin Rust to clippy.toml `msrv`, run `cargo build --workspace`
    - `arch-invariants`: run `cargo test --test architecture` (forbids cross-crate dep violations)
    - `commits`: install cocogitto, run `cog check origin/${{ github.base_ref }}..HEAD` on PRs (push: `cog check --from-latest-tag`)
    - `pr-title`: use `amannn/action-semantic-pull-request@v5` to validate PR title against the same type+scope allowlist (relevant for squash merges) [src: https://github.com/amannn/action-semantic-pull-request]
11. `.github/workflows/release.yml` — generated by `dist init` (cargo-dist [src: https://opensource.axo.dev/cargo-dist/]); 5-target matrix; triggered on `v*` tags only. Empty artifact set until tier-10.
12. `lefthook.yml` [src: https://lefthook.dev/]:
    ```yaml
    pre-commit:
      commands:
        fmt:   { glob: "*.rs", run: "cargo fmt --check {staged_files}" }
        lint:  { glob: "*.rs", run: "cargo clippy --workspace -- -D warnings" }
    commit-msg:
      commands:
        cog: { run: "cog verify --file {1}" }
    ```
    Install instructions in CONTRIBUTING.md; bypass only via `LEFTHOOK=0`.
13. `dependabot.yml`: cargo + github-actions, weekly, group all minor/patch into a single PR per ecosystem.
14. `CODEOWNERS`: `* @<repo-owner>` initially; per-crate owners reserved.
15. `PULL_REQUEST_TEMPLATE.md` checklist:
    - [ ] Failing test added before implementation (TDD)
    - [ ] If architectural decision: new ADR under `docs/adr/`
    - [ ] If new external dep: justified in PR description with link to docs
    - [ ] `cargo nextest run --workspace` green locally
    - [ ] Audit verdict file present (post tier-01, via spec-audit)
16. `CONTRIBUTING.md` sections: dev setup, lefthook install, cocogitto install + `cog install-hook commit-msg`, commit format quick-ref (table of types + scope allowlist with one-line meaning each), "add a crate" walkthrough referencing folder-layout.md, "add a port" walkthrough, "write an ADR" walkthrough, audit-gate explanation [src: .claude/settings.json `audit-gate.sh`].
17. `cog.toml` (repo root) [src: https://docs.cocogitto.io/config/]:
    ```toml
    from_latest_tag = false
    ignore_merge_commits = true
    branch_whitelist = ["main", "release/**", "feat/**", "fix/**", "chore/**"]
    [commit_types]
    feat = { changelog_title = "Features" }
    fix  = { changelog_title = "Bug Fixes" }
    perf = { changelog_title = "Performance" }
    refactor = { changelog_title = "Refactors" }
    docs = { changelog_title = "Documentation", omit_from_changelog = false }
    test = { changelog_title = "Tests" }
    build = { changelog_title = "Build" }
    ci    = { changelog_title = "CI" }
    chore = { omit_from_changelog = true }
    revert = { changelog_title = "Reverts" }
    style  = { omit_from_changelog = true }
    [packages]                       # mirrors crate scope allowlist
    core = { path = "crates/ariadne-core" }
    storage = { path = "crates/ariadne-storage" }
    parser  = { path = "crates/ariadne-parser" }
    scip    = { path = "crates/ariadne-scip" }
    graph   = { path = "crates/ariadne-graph" }
    salsa   = { path = "crates/ariadne-salsa" }
    watcher = { path = "crates/ariadne-watcher" }
    mcp     = { path = "crates/ariadne-mcp" }
    cli     = { path = "crates/ariadne-cli" }
    e2e     = { path = "crates/ariadne-e2e" }
    # cross-cutting scopes accepted without path binding: docs, ci, deps
    ```
    Add ADR-0003 documenting the choice + format examples (`feat(graph): add Tarjan SCC`, `fix(storage)!: change FileId layout`, `chore(deps): bump rmcp to 1.7.0`).

17. Update root `CLAUDE.md` `<commands>` block via `/rules-writer` (out-of-band note, do not edit by hand) with the pinned commands above.
</steps>

<verification>
- All files exist at exact paths above.
- `.github/workflows/ci.yml` parses with `actionlint` (run locally or via pre-commit step).
- `lefthook install` succeeds; a deliberately-misformatted staged Rust file is rejected by pre-commit (rustfmt runs without a manifest; the cargo-clippy command is `skip`ed until tier-01 introduces `Cargo.toml`).
- `cog verify --file <(echo 'bad commit')` exits non-zero; `cog verify --file <(echo 'feat(core): seed types')` exits zero.
- `cog check --from-latest-tag` passes on the seeded history once tier-00 lands a `v0.0.0` baseline tag (cocogitto's `from_latest_tag = false` config also lets `cog check` traverse full history when no tag exists).
- `docs/architecture.md` Mermaid renders on GitHub (manually preview).
- ADR 0001 + 0002 + 0003 round-trip through the template; numbering reserved.
- tests/architecture.rs is committed in failing state with a comment "becomes valid once tier-01 introduces the workspace" — tier-01 turns it green.
- `cargo deny check` deferred to tier-01 (no root `Cargo.toml` in tier-00); CI `deny` job auto-skips while the manifest is absent.
</verification>

<rollback>
All additions live in `docs/`, `.github/`, root config files. Rollback = `git rm -r docs/ .github/workflows/ci.yml .github/workflows/release.yml .github/CODEOWNERS .github/PULL_REQUEST_TEMPLATE.md .github/dependabot.yml rustfmt.toml clippy.toml deny.toml lefthook.yml CONTRIBUTING.md tests/architecture.rs`. No code state altered.
</rollback>
