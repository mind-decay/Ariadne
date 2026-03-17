# Ariadne — Implementation Roadmap

## Overview

Ariadne is a standalone Rust CLI that builds structural dependency graphs from source code via tree-sitter. Two phases: build the graph engine, then add algorithms and queries.

---

## Phase 1: Core CLI — Parse, Build, Output

**Goal:** `ariadne build <path>` parses a multi-language project and outputs `graph.json` + `clusters.json`. `ariadne info` reports version and supported languages.

**Deliverables:**
- Rust project structure
- Tree-sitter integration with grammar crates
- `LanguageParser` trait definition
- Tier 1 language parsers:
  - TypeScript / JavaScript (import, require, export, dynamic import, barrel re-exports)
  - Go (import)
  - Python (import, from...import, relative imports)
  - Rust (use, mod, extern crate)
  - C# (using, using static)
  - Java (import, import static)
- Graph data model: nodes (files + metadata) and edges (imports, tests, re-exports, type-imports)
- File type detection (source, test, config, style, asset, type-def)
- Architectural layer inference (api, service, data, util, component, hook, config)
- Content hashing (xxHash64) for delta detection
- JSON serialization (graph.json — compact tuple format for edges)
- Directory-based clustering (Level 1) → clusters.json
- CLI: `ariadne build <path>` and `ariadne info`
- Installation via `cargo install` + prebuilt binaries (GitHub Releases CI)

**Testing:**
- Unit tests per language parser (known import patterns → expected edges)
- Integration test: parse a multi-language sample project → verify graph correctness
- Performance benchmark: 1000+ files under 3 seconds

**Key decisions:** D-001 (architecture), D-002 (language support), D-003 (graceful degradation).

---

## Phase 2: Algorithms, Queries & Views

**Goal:** Graph becomes queryable — blast radius, centrality, cycles, clusters, layers, markdown views.

**Depends on:** Phase 1.

**Deliverables:**
- Algorithms:
  - Reverse BFS (blast radius with depth tracking)
  - Brandes algorithm (betweenness centrality)
  - Tarjan's SCC (circular dependency detection)
  - Louvain community detection (Level 2 refinement — enhances Phase 1's directory-based clusters)
  - Topological sort on DAG (architectural layer assignment, populates `arch_depth`)
- Delta computation (`ariadne update` — incremental via content hash, 5% threshold for full recompute)
- Subgraph extraction (BFS in both directions + cluster inclusion)
- Output files: stats.json, enriched clusters.json (Louvain refinement + cohesion metrics)
- Markdown view generation:
  - L0: `views/index.md` — cluster list, critical files, cycles, layer summary
  - L1: `views/clusters/<name>.md` — per-cluster detail with files, deps, metrics
  - L2: `views/impact/` — on-demand blast radius / subgraph reports
- CLI commands:
  - `ariadne update <path>` (incremental)
  - `ariadne query blast-radius <file> [--depth N] [--format json|md]`
  - `ariadne query subgraph <file...> [--depth N] [--format json|md]`
  - `ariadne query stats [--format json|md]`
  - `ariadne query cluster <name> [--format json|md]`
  - `ariadne query file <path> [--format json|md]`
  - `ariadne query cycles [--format json|md]`
  - `ariadne query layers [--format json|md]`
  - `ariadne views generate [--output <dir>]`

**Testing:**
- Algorithm correctness tests on known graphs (hand-crafted adjacency lists)
- Delta computation test: modify subset of files → verify only affected edges updated
- View generation test: verify L0/L1 markdown structure and content accuracy
- Performance: all algorithms on 3000-node graph under 1 second

---

## Future: Integration with External Tools

Ariadne is designed as a standalone tool. Integration with orchestration systems (e.g., Moira) happens on the consumer side — Ariadne provides the data, consumers decide how to use it.

**Integration surface:**
- `ariadne build` / `ariadne update` — invoked by external tools
- `graph.json`, `clusters.json`, `stats.json` — consumed by external tools
- `ariadne query *` — CLI queries invoked by external tools
- Markdown views — loaded into LLM agent contexts

Ariadne itself has no dependency on any orchestration framework.
