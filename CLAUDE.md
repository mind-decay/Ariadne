# Ariadne Development Rules

## What Is This Project

Ariadne is a structural dependency graph engine for source code. It parses projects via tree-sitter and produces a navigable dependency graph — files, imports, architectural layers, and module clusters.

Named after Ariadne of Greek mythology, who gave Theseus the thread to navigate the labyrinth. Ariadne gives developers the thread to navigate complex codebases.

**Design documents are the source of truth.** All implementation must conform to `design/` documents.

## Critical Files — Read Before ANY Work

1. `design/ROADMAP.md` — implementation phases and build order
2. `design/architecture.md` — full system design (data model, parsers, CLI, formats)
3. `design/decisions/log.md` — architectural decisions (D-001 through D-009)
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
- Define goal, deliverables, file list, design sources
- Get user approval before proceeding

### Step 2: Implementation Plan (`design/specs/YYYY-MM-DD-phaseN-implementation-plan.md`)
- Break into chunks with dependencies
- Plans describe WHAT, not full code
- Get user approval before proceeding

### Step 3: Implementation
- Follow the plan chunk by chunk
- `cargo test` after each chunk
- Final verification against spec success criteria

## Commit Messages

Format: `ariadne(<scope>): <description>`

Scopes: core, parser, graph, detect, cli, ci, test, design

Examples:
- `ariadne(core): implement data model types`
- `ariadne(parser): implement TypeScript/JavaScript parser`
- `ariadne(cli): implement build and info commands`

## File Structure

```
ariadne/
├── design/              # Design documents
│   ├── ROADMAP.md       # Implementation phases
│   ├── architecture.md  # Full system design
│   ├── decisions/       # Decision log
│   └── specs/           # Phase specs and plans
├── src/                 # Rust source
│   ├── main.rs          # CLI entry point
│   ├── lib.rs           # Public API
│   ├── graph/           # Graph model, builder, serialization, clustering
│   ├── parser/          # LanguageParser trait + per-language implementations
│   ├── detect/          # File type detection + layer inference
│   └── hash.rs          # Content hashing
├── tests/               # Integration tests, benchmarks, fixtures
├── Cargo.toml
└── .github/workflows/   # CI + release
```
