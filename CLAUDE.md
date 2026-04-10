# Ariadne Development Rules

## What Is This Project

Ariadne is a structural dependency graph engine for source code. It parses projects via tree-sitter and produces a navigable dependency graph — files, imports, architectural layers, and module clusters.

Named after Ariadne of Greek mythology, who gave Theseus the thread to navigate the labyrinth. Ariadne gives developers the thread to navigate complex codebases.

**Design documents are the source of truth.** All implementation must conform to `design/` documents.

## Documentation Map

Before working on any part of Ariadne, read the relevant docs:

| Document | When to read | What's in it |
|---|---|---|
| [design/ROADMAP.md](design/ROADMAP.md) | Before starting any phase | Implementation phases (1a-3c done), evolution plan (phases 4-12), Moira integration notes |
| [design/architecture.md](design/architecture.md) | Before ANY implementation work | Full system design: data model, parsers, CLI, formats, module boundaries |
| [design/decisions/log.md](design/decisions/log.md) | When questioning a design choice | Architectural decisions D-001 through D-123 with evidence and rejected alternatives |
| [design/path-resolution.md](design/path-resolution.md) | Before touching parser/config/ or import resolution | Path normalization, case sensitivity, monorepo support |
| [design/determinism.md](design/determinism.md) | Before changing output format or collection types | Byte-identical output strategy, BTreeMap requirement, sort invariants |
| [design/error-handling.md](design/error-handling.md) | Before adding error types or warnings | Error taxonomy (E001-E005, W001-W009), fault tolerance, --strict behavior |
| [design/performance.md](design/performance.md) | Before adding parallelism or changing data structures | Performance model, rayon usage, memory strategy |
| [design/testing.md](design/testing.md) | Before writing tests | 4-level test strategy: snapshots, fixtures, invariants, benchmarks |
| [design/distribution.md](design/distribution.md) | Before changing CI, releases, or install | Versioning, installation, release process |
| Relevant spec/plan | Before starting a phase | `design/specs/YYYY-MM-DD-phaseN-*.md` for deliverables and completion criteria |

**Rules:**
- If you're about to make a design change that contradicts anything in these docs, STOP. Read the relevant doc first. If you still think the change is needed, present evidence to the human — don't just do it.
- Before starting a phase, read its spec for completion criteria. A phase is DONE only when ALL its GIVEN/WHEN/THEN assertions pass.
- After completing a phase, update the Progress Tracking in ROADMAP.md.

## Locked Constraints (NOT debatable during implementation)

- **Language:** Rust only. No bash scripts, no Python, no TypeScript.
- **Parsing:** Tree-sitter queries for AST extraction. No regex-based parsing.
- **Determinism:** Same input → byte-identical output. `BTreeMap` not `HashMap`. All lists sorted. No timestamps in default output. (D-006)
- **Error handling:** `Result<T, E>` everywhere. No `panic!` in production code. No `.unwrap()` outside tests. No silent error swallowing.
- **Paths:** Canonical relative format — forward slashes, no `./` prefix, no `..` segments, relative to project root. (D-007)
- **Single graph:** One graph per invocation, even for monorepos. No per-package sub-graphs. (D-008)
- **Consumer-agnostic:** Zero knowledge of any specific consumer (Moira, IDEs, CI). No consumer-specific formats or export modes. (D-004)
- **Single binary:** CLI + MCP server in one binary (`ariadne serve`). No separate `ariadne-mcp`. (D-045)
- **No async runtime:** No tokio. IO is sequential (stdio), file watching is OS-native, rebuild is CPU-bound. (D-047)
- **Module boundaries:** `algo/` is pure computation (no `serial/` deps). `model/` is a leaf module (no deps). No circular imports between modules.
- **Module size:** <=300 lines per file. Split if approaching limit.

## Rejected Approaches (DO NOT re-introduce)

| Approach | Why rejected | Decision |
|---|---|---|
| Agent-driven / LLM parsing | Expensive in tokens, non-deterministic, slow. Tree-sitter does this deterministically. | D-001 |
| Regex-based parsing | Fragile, doesn't scale across languages. Tree-sitter queries are the standard. | D-001 |
| Consumer-specific formats (`format: "moira"`) | Couples releases to consumer schema. Ariadne must be consumer-agnostic. | D-004 |
| HashMap for graph data | Non-deterministic iteration breaks byte-identical output and git diffs. | D-006 |
| CLI-only integration (no MCP) | 100-500ms cold-start per query breaks agent workflows. Persistent MCP server chosen. | D-037 |
| Async runtime (tokio) | +1.5MB binary, +30 deps, no architectural benefit. IO is sequential. | D-047 |
| Separate MCP binary | Duplicates Composition Root, adds distribution complexity. Single binary with `serve`. | D-045 |
| Binary fresh/stale status | Single file change shouldn't mark entire graph stale. Hash-based per-file confidence instead. | D-039 |
| Manual incremental parsing | Premature optimization. Full rebuild is fast; true incremental belongs in in-memory phase. | D-050 |
| Verbose edge serialization | 60%+ larger output, worse git diffs. Compact tuples `[from, to, type, [symbols]]`. | D-012 |
| "Tests pass = done" | Tests verify author's assumptions, not real behavior. E2E on real project required. | — |
| Batched fixes without verification | Fixing N bugs at once introduces new bugs. Each fix verified individually. | — |
| LLM declares phase complete | Only human can sign off. LLM presents evidence, human decides. | — |

## Development Process (Per Phase)

### Phase Steps

1. **SPEC** — Human + LLM define WHAT + executable assertions (GIVEN/WHEN/THEN).
   Assertion count is FIXED — LLM cannot reduce it during implementation.

2. **TEST FIRST** — LLM writes tests, human reviews.
   Tests MUST FAIL before implementation exists.
   If test passes before implementation -> test is useless -> rewrite.

3. **IMPLEMENT** — One goal: pass all assertions.
   No scope changes. No "improvements". No skipping.

4. **VERIFY** — Three levels, ALL required:
   - `cargo test` passes (necessary but not sufficient — tests verify assumptions, not behavior)
   - E2E on real project: run the actual binary, read every line of output, check plausibility of all values
   - Present honest assessment: what works, what doesn't, what wasn't tested. Include evidence (actual e2e output).

5. **SIGN-OFF** — Human confirms phase works. Not just "tests pass".
   LLM never declares a phase complete. LLM presents evidence, human decides.
   Decision log: add new decisions to `design/decisions/log.md`

### E2E Verification Rules

Unit tests prove that code matches the author's assumptions. They do NOT prove the code works.
Every feature must be verified on a real e2e run before it can be called done.

**After every implementation step:**
1. Run e2e on a real project (not just `cargo test`)
2. Read every line of output as a user, not as the author
3. For every number: "is this plausible?" (50K tokens for 1 file = bug, not feature)
4. For every repeated section: "is this intentional?"

**For every code path (not just happy path):**
- List all branches: warnings, errors, edge cases
- For each: either trigger it in a real e2e, or explicitly state "NOT VERIFIED — requires [scenario]"
- Never mark a feature done based only on unit tests with synthetic data

**A fix is not done until verified on the same scenario that exposed the bug.**
Do not batch fixes. Fix one thing, verify it works, then fix the next.

**Never declare a phase complete.** Only the human can do that (SIGN-OFF step).
Present what works, what doesn't, and what hasn't been verified — the human decides.

### Protection from LLM Fabrication

- Architecture decisions LOCKED in this file before implementation begins
- "X doesn't work" requires a failing test as evidence, not theory
- "I think X is better" without evidence = rejected
- Assertion count fixed in spec — cannot be reduced or skipped
- Fixtures verified by human spot-checks
- Integration test on REAL project — not mock/fake controlled by LLM

## Phase Implementation Process

Every phase follows a strict 3-step process:

### Step 1: Spec (`design/specs/YYYY-MM-DD-phaseN-<name>.md`)

- Generate with `/write-spec` → review with `/review-spec`
- Define goal, deliverables, file list, design sources
- Get user approval before proceeding

### Step 2: Implementation Plan (`design/specs/YYYY-MM-DD-phaseN-implementation-plan.md`)

- Break into chunks with dependencies
- Review with `/review-plan` before proceeding
- Plans describe WHAT, not full code
- Get user approval before proceeding

### Step 3: Implementation

- Follow the plan chunk by chunk
- `cargo test` after each chunk
- Final verification against spec success criteria

### Reports

All commands write reports to `design/reports/{date}-{type}.md`. Previous reports are referenced for context (resolved/unresolved issues).

## Constraint Enforcement Tests

The following architectural invariants are enforced by tests and CI:

**Graph invariants (`tests/invariants.rs` — INV-1 through INV-18):**
- Edge referential integrity, no self-imports, no duplicate edges
- Cluster completeness and consistency
- Byte-identical determinism across builds
- Centrality values in [0.0, 1.0], layers cover all nodes

**Source code constraints (`tests/constraints.rs`):**
- `no_hashmap_in_model` — `HashMap` forbidden in `model/` and `serial/` (determinism)
- `no_god_modules` — no NEW file in `src/` may exceed 300 lines (legacy violations allowlisted)
- `no_new_hashmap_imports` — `HashMap` imports cannot spread to new modules without allowlisting
- `no_silent_errors` — no `let _ = ...` in production code (test code excluded, justified uses allowlisted)

When splitting a file below 300 lines, remove it from the `no_god_modules` allowlist.
When adding `HashMap` or `let _ =` to a new file, add to the allowlist with a justification comment.

## Commit Messages

Format: `ariadne(<scope>): <description>`

Scopes: core, parser, pipeline, graph, detect, serial, cli, ci, test, design, mcp, analysis, algo, temporal, semantic, conventions

Rules:
- Subject line: imperative mood, lowercase, no period, <72 chars
- Body (optional): wrap at 72 chars, explain WHY not WHAT (the diff shows what)
- One logical change per commit

Examples:

- `ariadne(core): implement data model types`
- `ariadne(parser): implement TypeScript/JavaScript parser`
- `ariadne(cli): implement build and info commands`

## File Structure

```
ariadne/
├── design/              # Design documents (source of truth)
│   ├── ROADMAP.md       # Implementation phases
│   ├── architecture.md  # Full system design
│   ├── decisions/       # Decision log (D-001 through D-123)
│   ├── specs/           # Phase specs and plans
│   └── reports/         # Architecture reviews, audit reports
├── src/                 # Rust source
│   ├── main.rs          # Composition Root: CLI (clap) + wires concrete types (D-020)
│   ├── lib.rs           # Public API re-exports
│   ├── model/           # Data types, newtypes, enums (leaf module, no deps) (D-017, D-023)
│   ├── parser/          # LanguageParser + ImportResolver traits, registry, per-language impls (D-018)
│   │   └── config/      # Config-aware resolution: tsconfig.json, go.mod, pyproject.toml (D-118..D-123) [Phase 10]
│   ├── pipeline/        # BuildPipeline, stage traits (FileWalker, FileReader), orchestration (D-019)
│   ├── detect/          # File type detection + architectural layer inference
│   ├── cluster/         # Directory-based clustering
│   ├── algo/            # Graph algorithms: SCC, BFS, centrality, topo sort, subgraph (D-033) [Phase 2a]
│   ├── views/           # Markdown view generation: L0 index, L1 cluster, L2 impact (D-033) [Phase 2a]
│   ├── analysis/        # Martin metrics, smell detection, structural diff (D-048) [Phase 3b]
│   ├── mcp/             # MCP server, tools, state management (D-045) [Phase 3a]
│   ├── serial/          # GraphSerializer + GraphReader traits, output types, JSON impl (D-022, D-032)
│   ├── temporal/         # Git history engine: churn, co-change, hotspots [Phase 7]
│   ├── semantic/         # Boundary extraction: HTTP routes, events, DI [Phase 8]
│   ├── diagnostic.rs    # FatalError, Warning, DiagnosticCollector (D-021)
│   └── hash.rs          # xxHash64 → ContentHash
├── tests/               # Integration tests, fixtures, snapshots
├── benches/             # Performance benchmarks (Phase 1b)
├── Cargo.toml
└── .github/workflows/   # CI + release (Phase 1b)
```
