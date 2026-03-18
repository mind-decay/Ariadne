# Ariadne — Implementation Roadmap

## Overview

Ariadne is a standalone Rust CLI that builds structural dependency graphs from source code via tree-sitter.

**Crate name:** `ariadne-graph` (binary: `ariadne`) — D-010.

---

## Phase 1a: MVP — Parse and Output

**Goal:** `ariadne build <path>` works. Parses a multi-language project, outputs `graph.json` + `clusters.json`. Basic error handling (skip broken files, log to stderr). No frills.

**Deliverables:**

- Cargo project (`ariadne-graph` crate, `ariadne` binary)
- Core data model (BTreeMap for determinism — D-006)
- Tree-sitter integration with partial parse handling
- 6 Tier 1 language parsers (TS/JS, Go, Python, Rust, C#, Java)
- File type detection + architectural layer inference
- xxHash64 content hashing
- Directory-based clustering
- Graph builder pipeline (walk → read → parse → resolve → cluster → sort → output)
- JSON serialization (deterministic, sorted, atomic writes)
- CLI: `ariadne build <path> [--output <dir>]` and `ariadne info`
- Basic tests: parser snapshots (insta), fixture graph tests, invariant checks

**NOT in 1a (deferred to 1b):**

- Structured warning system (W001-W009 codes, JSON format)
- CLI flags: --verbose, --warnings, --strict, --timestamp, --max-file-size, --max-files
- Workspace/monorepo detection
- Case-insensitive FS handling
- Per-stage timing output
- Property-based tests, performance benchmarks
- CI/CD workflows, install.sh
- README.md

**Testing:** Parser snapshots (L1), fixture graph snapshots (L2), invariant checks (L3 basic). No benchmarks.

**Success criteria:**

1. `cargo build --release` compiles
2. `ariadne info` lists 6 languages
3. `ariadne build` on each fixture project produces correct graph.json
4. Output is byte-identical on repeated builds (determinism)
5. Broken files are skipped with stderr warning (not crash)
6. All `cargo test` pass

---

## Phase 1b: Hardening

**Goal:** Production-quality error handling, full CLI, workspace support, comprehensive tests, CI/CD.

**Depends on:** Phase 1a.

**Deliverables:**

- Structured warning system (W001-W009, human + JSON format)
- All CLI flags (--verbose, --warnings, --strict, --timestamp, --max-file-size, --max-files)
- npm/yarn/pnpm workspace detection and workspace-aware import resolution (D-008)
- Path normalization with case-insensitive FS detection (D-007)
- Per-stage --verbose timing output
- Property-based tests (proptest)
- Performance benchmarks (criterion)
- GitHub Actions CI + release workflows
- install.sh script
- README.md

**Testing:** Full L1-L4 suite. Workspace fixture. Path normalization + traversal + case sensitivity tests.

---

## Phase 2: Algorithms, Queries & Views

**Goal:** Graph becomes queryable — blast radius, centrality, cycles, clusters, layers, markdown views.

**Depends on:** Phase 1b.

**Deliverables:**

- Algorithms: Reverse BFS, Brandes centrality, Tarjan SCC, Louvain clustering, topological sort
- Delta computation (`ariadne update` — incremental via content hash)
- Subgraph extraction
- Output: stats.json, enriched clusters.json
- Markdown views (L0/L1/L2)
- CLI: `ariadne update`, `ariadne query *`, `ariadne views generate`

---

## Future

- Tier 2/3 language parsers
- Config file (.ariadne.toml)
- Plugin system for external parsers
- `ariadne self-update`
- Package manager distribution (brew, nix, AUR)
- Integration with orchestration frameworks (Moira etc.)
