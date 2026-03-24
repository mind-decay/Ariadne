# Ariadne Development Rules

## What Is This Project

Ariadne is a structural dependency graph engine for source code. It parses projects via tree-sitter and produces a navigable dependency graph — files, imports, architectural layers, and module clusters.

Named after Ariadne of Greek mythology, who gave Theseus the thread to navigate the labyrinth. Ariadne gives developers the thread to navigate complex codebases.

**Design documents are the source of truth.** All implementation must conform to `design/` documents.

## Critical Files — Read Before ANY Work

1. `design/ROADMAP.md` — implementation phases (1a-3c done), evolution plan (phases 4-10), Moira integration notes
2. `design/architecture.md` — full system design (data model, parsers, CLI, formats)
3. `design/decisions/log.md` — architectural decisions (D-001 through D-076)
4. `design/path-resolution.md` — path normalization, case sensitivity, monorepo support
5. `design/determinism.md` — byte-identical output strategy
6. `design/error-handling.md` — error taxonomy (E001-E005, W001-W009), fault tolerance
7. `design/performance.md` — performance model, parallelism, memory strategy
8. `design/testing.md` — 4-level test strategy (snapshots, fixtures, invariants, benchmarks)
9. `design/distribution.md` — versioning, installation, releases
10. The specific spec/plan relevant to your current task

## Development Protocol

### Before Changes
1. Read the relevant design docs
2. Impact analysis: which components are affected?
3. Design-first: if implementation deviates from design, update design docs FIRST with user approval

### During Changes
4. One goal per session
5. Additive over modifying
6. No speculative improvements

### After Changes
7. Regression check: `cargo test`
8. Conformance check: does implementation match design docs?
9. Decision log: add new decisions to `design/decisions/log.md`

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

## Quality Commands

Project-specific commands in `.claude/commands/`. Use them at the appropriate phase.

### Spec & Plan Workflow
| Command | When to use | What it does |
|---------|------------|--------------|
| `/write-spec [phase]` | Starting a new phase | Generates spec from ROADMAP + architecture docs (3 agents: requirements, risk, gaps) |
| `/review-spec [path]` | After writing/updating a spec | Verifies design source accuracy, completeness, cross-phase impact (3 agents) |
| `/review-plan [path]` | After writing/updating a plan | Verifies spec coverage, file accuracy, design compliance, dependency order (4 agents) |

### Architecture & Documentation Health
| Command | When to use | What it does |
|---------|------------|--------------|
| `/review-architecture [focus]` | Periodically, or before major phases | Deep architectural critique — design quality, complexity, robustness (4 agents). Dual-mode: pre-impl (docs only) / post-impl (docs + code) |
| `/audit-docs [mode]` | After design doc changes, or before implementation | Consistency audit across all design docs + code conformance. Modes: `docs`, `code`, or auto-detect (3-4 agents). Can apply fixes after audit |

### Reports
All commands write reports to `design/reports/{date}-{type}.md`. Previous reports are referenced for context (resolved/unresolved issues).

## Commit Messages

Format: `ariadne(<scope>): <description>`

Scopes: core, parser, pipeline, graph, detect, serial, cli, ci, test, design, mcp, analysis, algo, temporal, semantic

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
│   ├── decisions/       # Decision log (D-001 through D-076, planned D-077 through D-090)
│   ├── specs/           # Phase specs and plans
│   └── reports/         # Architecture reviews, audit reports
├── src/                 # Rust source
│   ├── main.rs          # Composition Root: CLI (clap) + wires concrete types (D-020)
│   ├── lib.rs           # Public API re-exports
│   ├── model/           # Data types, newtypes, enums (leaf module, no deps) (D-017, D-023)
│   ├── parser/          # LanguageParser + ImportResolver traits, registry, per-language impls (D-018)
│   ├── pipeline/        # BuildPipeline, stage traits (FileWalker, FileReader), orchestration (D-019)
│   ├── detect/          # File type detection + architectural layer inference
│   ├── cluster/         # Directory-based clustering
│   ├── algo/            # Graph algorithms: SCC, BFS, centrality, topo sort, subgraph (D-033) [Phase 2a]
│   ├── views/           # Markdown view generation: L0 index, L1 cluster, L2 impact (D-033) [Phase 2a]
│   ├── analysis/        # Martin metrics, smell detection, structural diff (D-048) [Phase 3b]
│   ├── mcp/             # MCP server, tools, state management (D-045) [Phase 3a]
│   ├── serial/          # GraphSerializer + GraphReader traits, output types, JSON impl (D-022, D-032)
│   ├── temporal/         # Git history engine: churn, co-change, hotspots [Phase 7, planned]
│   ├── semantic/         # Boundary extraction: HTTP routes, events, DI [Phase 8, planned]
│   ├── diagnostic.rs    # FatalError, Warning, DiagnosticCollector (D-021)
│   └── hash.rs          # xxHash64 → ContentHash
├── tests/               # Integration tests, fixtures, snapshots
├── benches/             # Performance benchmarks (Phase 1b)
├── Cargo.toml
└── .github/workflows/   # CI + release (Phase 1b)
```
