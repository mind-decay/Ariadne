<project>
Ariadne v2. Rust code-intelligence platform that maintains an incrementally-updated multi-language semantic graph of any project and exposes it to Claude through an MCP stdio server. Source of truth for agent behavior is `.claude/skills/` + `.claude/plans/ariadne-core/` + this file. Implementation populated tier-by-tier via the spec lifecycle below; no code on disk until tier-00 builds [src: .claude/plans/ariadne-core/plan.md].
</project>

<workflow>
The repo enforces a three-step spec lifecycle. Each step is a separate Claude session.
1. `/spec-plan` — produce `.claude/plans/<slug>/plan.md` (and `tier-NN-<name>.md` files for multi-tier work).
2. `/spec-build <path-to-tier-or-plan>` — execute exactly one tier or a single-tier plan.
3. `/spec-audit <path-to-tier-or-plan>` — pedantic review; writes `.claude/plans/<slug>/audit/<id>-report.md` and `.claude/plans/<slug>/audit-state.json`.
Plans are versioned in git. Audit verdicts gate commit/push via `.claude/hooks/audit-gate.sh` [src: .claude/settings.json].
Tier execution order for ariadne-core: 00 → 01 → 02 → 03 → 04 → {05, 06} → 07 → {09, 08} → 10 [src: .claude/plans/ariadne-core/plan.md].
</workflow>

<architecture>
Style: Hexagonal / Ports & Adapters + TDD [src: https://alistair.cockburn.us/hexagonal-architecture/, https://www.howtocodeit.com/guides/master-hexagonal-architecture-in-rust].
- Domain interior: `ariadne-core` (types + ports), `ariadne-graph` (analytics use cases), `ariadne-salsa` (incremental query DB).
- Driving (inbound) adapters: `ariadne-cli`, `ariadne-mcp`, `ariadne-watcher`.
- Driven (outbound) adapters: `ariadne-storage` (redb), `ariadne-parser` (tree-sitter), `ariadne-scip` (subprocess+protobuf).
Hard invariants — enforced by tier-00 `tests/architecture.rs` + cargo-deny:
- `ariadne-core` has zero in-workspace deps.
- Adapter crates depend only on `ariadne-core`; never on each other.
- `src/lib.rs` is a façade — re-exports only, no logic.
- Domain code lives under `src/domain/`; IO lives under `src/adapters/`; one file per external tech.
- `thiserror` enums in public API; `anyhow` only inside `ariadne-cli` and `ariadne-e2e`.
Forward imports below load when tier-00 ships `docs/`:
@docs/architecture.md
@docs/folder-layout.md
</architecture>

<commands>
Planned per .claude/plans/ariadne-core/tier-00-foundations.md; verified at first tier-00 build. Until then, treat as authoritative for new tier sessions.
- Build:        `cargo build --workspace`
- Tests:        `cargo nextest run --workspace` (CI: `--profile ci`)
- Bench build:  `cargo bench --workspace --no-run`
- Lint:         `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- Format check: `cargo fmt --all --check`
- Format apply: `cargo fmt --all`
- Deny:         `cargo deny check`
- Audit:        `cargo audit`
- Docs:         `RUSTDOCFLAGS=-D warnings cargo doc --workspace --no-deps --document-private-items`
- Arch invariant: `cargo test --test architecture`
- Commit lint (local commit-msg hook):    `cog verify --file <msg-file>`
- Commit lint (CI on PR):                  `cog check origin/${BASE_REF}..HEAD`
- Changelog (release pipeline tier-10):    `cog changelog`
</commands>

<rules>
- Evidence first. Every external API, flag, library version, or architectural decision needs an inline `[src: …]` to fetched documentation or repo code with line numbers. Recall is not evidence.
- Per-session doc fetch. Use Context7 (`resolve-library-id` → `query-docs`) for every external technology; fall back to `WebSearch` + `WebFetch` only when Context7 has no entry or quota is exhausted [src: .claude/skills/spec-plan/SKILL.md].
- ≤200 lines per authored file (skills, rules, plan files, tier files, audit reports). Split, do not bloat. Project rule overriding the upstream 500-line skill guidance [src: https://platform.claude.com/docs/en/agents-and-tools/agent-skills/best-practices].
- XML tags wrap semantic sections in skills, rules, plans, and audit reports. Claude is trained on XML structure [src: https://platform.claude.com/docs/en/build-with-claude/prompt-engineering/claude-prompting-best-practices].
- Imperative voice in skill and rule files. No second person.
- Architectural lens is fixed: scalability, reliability, efficiency, maintainability. Delivery speed is not a tradeoff axis when choosing patterns.
- Do not add features, refactors, error handling, fallbacks, or abstractions beyond what the active plan tier requires [src: Claude Code system instructions, "Doing tasks"].
- Do not introduce a new dependency, technology, or architectural pattern outside the plan's `<decisions>` / `<tech_inventory>` without stopping and asking the user.
- Audit treats the diff as if written by Codex or the user. Never as own work [src: .claude/skills/spec-audit/SKILL.md].
- Validate by execution. No change is "done" until the build runs green, the relevant tests run green, the feature is exercised end-to-end (real run, not stub), and every observed result is compared against an explicit stated expectation. UI/frontend changes require launching the dev server and walking the golden path; type-check + unit tests alone do not count [src: https://code.claude.com/docs/en/best-practices]. Failures are root-caused, not silenced (`--no-verify`, weakened asserts, `try/except: pass`, deleted tests are hard fails). When validation cannot run in-session, state it explicitly — never claim success.
- TDD mandatory: each tier writes a failing test before implementation; tests are realistic (no mocks at module boundaries; in-memory adapters allowed only for unit tests of pure domain) [src: .claude/plans/ariadne-core/plan.md `<constraints>`].
- Hexagonal boundary rule: ariadne-core declares ports (traits); adapter crates implement them. No `pub use` of an adapter type from a domain crate. Violation = audit hard fail [src: .claude/plans/ariadne-core/plan.md D13].
- Commit format: Conventional Commits v1.0.0 `<type>(<scope>)<!>: <subject>` [src: https://www.conventionalcommits.org/en/v1.0.0/]. Types: feat, fix, docs, style, refactor, perf, test, build, ci, chore, revert. Scopes = crate names without `ariadne-` prefix (core, storage, parser, scip, graph, salsa, watcher, mcp, cli, daemon, e2e) + cross-cutting (docs, ci, deps). Subject ≤72 chars, imperative. Breaking change: `!` after scope OR `BREAKING CHANGE:` footer. Enforced by lefthook commit-msg hook + CI `cog check` + PR-title action [src: .claude/plans/ariadne-core/plan.md D14, https://github.com/cocogitto/cocogitto].
- Architectural decisions require an ADR under `docs/adr/NNNN-kebab-title.md` using the template; ADR cited from plan or tier file [src: .claude/plans/ariadne-core/tier-00-foundations.md].
- Per-tier memory probe required: after tier-04 ships, every tier touching Salsa or in-RAM graph reports `memory_report()` deltas; >256MB per table is a hard fail (R1) [src: .claude/plans/ariadne-core/plan.md `<risks>`].
- Performance SLOs (verified per tier ≥10): cold full-index <60s, incremental update p95 <500ms, query p95 <100ms on a 100K-file workload [src: .claude/plans/ariadne-core/plan.md `<constraints>`].
- No cgo, no Node runtime, no JVM in production binaries. Pure-Rust deps only on the critical path [src: .claude/plans/ariadne-core/plan.md D5, D14].
</rules>

<conventions>
- Filenames and paths use forward slashes only [src: https://platform.claude.com/docs/en/agents-and-tools/agent-skills/best-practices].
- Memory files, skill files, plan files: kebab-case, ASCII.
- Frontmatter `description` is always third person and includes both *what* and *when to use* with explicit trigger phrases [src: https://platform.claude.com/docs/en/agents-and-tools/agent-skills/best-practices].
- `name` field obeys `[a-z0-9-]{1,64}` and excludes reserved words (`anthropic`, `claude`) [src: same].
- Crate naming: `ariadne-<role>` (one role per crate). Internal modules follow `src/domain/`, `src/adapters/<tech>.rs`, `src/errors.rs` [src: .claude/plans/ariadne-core/tier-00-foundations.md].
- Public API in adapter crates re-exports its single port impl + types; never leaks the underlying lib's types (redb, tree-sitter, prost) [src: same].
- ADRs numbered sequentially `docs/adr/NNNN-kebab-title.md`; status field one of: Proposed | Accepted | Superseded by ADR-XXXX [src: .claude/plans/ariadne-core/tier-00-foundations.md].
</conventions>

<authoring>
- New skill → invoke `/skill-writer` (meta-skill). It enforces alignment loop, doc fetch, frontmatter validation, and ≤200-line body.
- New or updated rules / `CLAUDE.md` / `AGENTS.md` → invoke `/rules-writer`. Same gates.
- New architectural decision → write an ADR under `docs/adr/NNNN-…md`; link from the relevant plan/tier file.
- Never hand-edit a SKILL.md or memory file without first running its meta-skill or matching its non-negotiables.
</authoring>

<imports>
@.claude/skills/skill-writer/SKILL.md
@.claude/skills/rules-writer/SKILL.md
@.claude/skills/spec-plan/SKILL.md
@.claude/skills/spec-build/SKILL.md
@.claude/skills/spec-audit/SKILL.md
@.claude/plans/ariadne-core/plan.md
@docs/adr/0001-architecture-style.md
@docs/adr/0002-tech-stack.md
@docs/adr/0003-commit-convention.md
</imports>

<sources>
- [Manage Claude's memory — Claude Code docs](https://code.claude.com/docs/en/memory)
- [Skill authoring best practices — Anthropic](https://platform.claude.com/docs/en/agents-and-tools/agent-skills/best-practices)
- [Extend Claude with skills — Claude Code docs](https://code.claude.com/docs/en/skills)
- [Prompting best practices (XML tags) — Anthropic](https://platform.claude.com/docs/en/build-with-claude/prompt-engineering/claude-prompting-best-practices)
- [Hooks reference — Claude Code docs](https://code.claude.com/docs/en/hooks-guide)
- [Hexagonal Architecture — Cockburn 2005](https://alistair.cockburn.us/hexagonal-architecture/)
- [Conventional Commits v1.0.0](https://www.conventionalcommits.org/en/v1.0.0/)
- [cocogitto](https://github.com/cocogitto/cocogitto)
</sources>

<!-- BEGIN ARIADNE -->
## Ariadne code intelligence

The Ariadne MCP server is configured for this project (`.mcp.json`). It exposes
a read-only semantic graph — symbols, references, and dependency edges — kept
current with the code.

Prefer the Ariadne MCP tools over `grep` / `Read` for any question about
symbols, references, impact, or architecture: the graph answers in one call
where text search needs many and misses cross-file edges.

- Navigate — `list_symbols`, `find_definition`, `find_references`. Use when
  locating a symbol or its call sites ("where is `X` defined?").
- Search / Read — `search_code`, `read_symbol`, `read_outline`. Use to find
  code by pattern, read a symbol's source, or fold a whole file to a token-
  cheap skeleton (then expand bodies with `read_symbol`); the `ariadne
  outline` CLI prints the same skeleton.
- Impact — `blast_radius`, `plan_assist`, `diff_blast_radius`. Use when scoping a
  change ("what breaks if I change `X`?", "what does my current diff affect?").
- Architecture — `coupling_report`, `weak_spots`, `refactor_suggestions`. Use
  when assessing structural health ("what are the worst modules?").
- History analytics — `hotspots`, `complexity`, `co_change`. Use when triaging
  risk from Git churn × complexity ("what's the riskiest code?", "what changes
  together?").
- Docs — `doc_for`, `doc_for_module`, `doc_for_project`. Use when summarizing a
  symbol, file, or the whole project ("document the `X` module").
- Freshness — `project_status`. Use to confirm the index is current ("is the
  index up to date?").
- Economy — the growable tools (`find_references`, `blast_radius`,
  `coupling_report`, `weak_spots`, `co_change`, `hotspots`, `complexity`,
  `refactor_suggestions`, `diff_blast_radius`, `affected_tests`) return a concise
  default page; pass `verbosity: detailed` for every field and follow the opaque
  `next_cursor` to page the rest.
<!-- END ARIADNE -->
