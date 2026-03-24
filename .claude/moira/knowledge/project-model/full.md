<!-- moira:freshness init 2026-03-21 -->
<!-- moira:knowledge project-model L2 -->

---
layout_pattern: "rust-cargo-workspace"
source_root: "src/"
entry_points:
  - "src/main.rs"
  - "src/lib.rs"
test_pattern: "separate"
test_roots:
  - "tests/"
  - "benches/"
test_naming: "<module>_tests.rs for integration tests; mod tests inline for unit tests"
do_not_modify:
  - ".git/"
  - "Cargo.lock"
  - "LICENSE-APACHE"
  - "LICENSE-MIT"
modify_with_caution:
  - "design/"
  - "Cargo.toml"
  - ".github/workflows/"
dir_source: "src/"
dir_tests: "tests/"
dir_benches: "benches/"
dir_design: "design/"
dir_ci: ".github/workflows/"
dir_claude: ".claude/"
dir_docs: "docs/"
---

# Structure Scan — Ariadne

**Scanned:** 2026-03-21
**Root:** `/Users/minddecay/Documents/Projects/Ariadne`

## 1. Top-Level Structure

```
.claude/            # Claude Code commands and Moira orchestration state
.git/               # Git repository
.github/            # CI/CD workflows
.gitignore          # Ignores: /target/, .vscode/, .worktrees/
benches/            # Performance benchmarks (6 files)
design/             # Design documents — source of truth (31 files)
docs/               # Documentation (1 subdirectory: superpowers/)
src/                # Rust source code (64 files)
tests/              # Integration tests and fixtures (85 files)
Cargo.lock          # Dependency lock file
Cargo.toml          # Rust project manifest
CLAUDE.md           # Claude Code project instructions
install.sh          # Local install script
LICENSE-APACHE      # Apache 2.0 license
LICENSE-MIT         # MIT license
README.md           # Project README
remote-install.sh   # Remote install script
```

## 2. Source Directory Layout (`src/`)

### Flat files at `src/`
| File | Role |
|------|------|
| `main.rs` | Application entry point — CLI (clap) + composition root |
| `lib.rs` | Public API re-exports |
| `diagnostic.rs` | FatalError, Warning, DiagnosticCollector |
| `hash.rs` | xxHash64 → ContentHash |

### Subdirectories at `src/`
| Directory | Files | Contents |
|-----------|-------|----------|
| `algo/` | 12 | Graph algorithms: blast_radius, centrality, compress, delta, louvain, pagerank, scc, spectral, stats, subgraph, topo_sort, mod.rs |
| `analysis/` | 4 | diff, metrics, smells, mod.rs |
| `cluster/` | 1 | mod.rs (directory-based clustering) |
| `detect/` | 5 | case_sensitivity, filetype, layer, workspace, mod.rs |
| `mcp/` | 6 | lock, server, state, tools, watch, mod.rs |
| `model/` | 11 | compress, diff, edge, graph, node, query, smell, stats, types, workspace, mod.rs |
| `parser/` | 9 | csharp, go, java, python, rust_lang, typescript, registry, traits, mod.rs |
| `pipeline/` | 5 | build, read, resolve, walk, mod.rs |
| `serial/` | 4 | convert, json, mod.rs (note: 3 listed but dir shows convert.rs, json.rs, mod.rs) |
| `views/` | 4 | cluster, impact, index, mod.rs |

## 3. Entry Points

| File | Evidence |
|------|----------|
| `src/main.rs` | Composition root: CLI (clap) + wires concrete types (per CLAUDE.md, D-020) |
| `src/lib.rs` | Public API re-exports (library entry point) |

## 4. Generated Directories

| Directory | Status |
|-----------|--------|
| `target/` | Not present in working tree; listed in `.gitignore` |
| `.vscode/` | Not present; listed in `.gitignore` |
| `.worktrees/` | Not present; listed in `.gitignore` |
| `dist/`, `build/`, `node_modules/` | Not present; not in `.gitignore` |

No generated directories exist in the working tree.

## 5. Vendored Directories

| Directory | Status |
|-----------|--------|
| `vendor/` | Not present |
| `third_party/` | Not present |

No vendored code found.

## 6. Configuration Files

| File | Purpose |
|------|---------|
| `Cargo.toml` | Rust project manifest — dependencies, build config, metadata |
| `Cargo.lock` | Pinned dependency versions |
| `.gitignore` | Git ignore rules: `/target/`, `.vscode/`, `.worktrees/` |
| `CLAUDE.md` | Claude Code project instructions and development protocol |

## 7. Test Organization

**Pattern:** Separate test directory (`tests/`) with dedicated fixture directories.

### Integration tests (`tests/`)
| File | Purpose |
|------|---------|
| `graph_tests.rs` | Graph integration tests |
| `pipeline_tests.rs` | Pipeline integration tests |
| `mcp_tests.rs` | MCP server integration tests |
| `invariants.rs` | Invariant tests |
| `properties.rs` | Property-based tests |
| `helpers.rs` | Shared test helpers |

**Naming convention:** `<module>_tests.rs` for module-specific integration tests.

### Test fixtures (`tests/fixtures/`)
9 fixture directories simulating real project types:
- `csharp-project/`
- `edge-cases/`
- `go-service/`
- `java-project/`
- `mixed-project/`
- `python-package/`
- `rust-crate/`
- `typescript-app/`
- `workspace-project/`

Each fixture contains a minimal project structure (e.g., `typescript-app/` contains `package.json`, `tsconfig.json`, `src/`, `.ariadne`).

### Benchmarks (`benches/`)
| File | Purpose |
|------|---------|
| `algo_bench.rs` | Algorithm benchmarks |
| `analysis_bench.rs` | Analysis benchmarks |
| `build_bench.rs` | Build pipeline benchmarks |
| `mcp_bench.rs` | MCP server benchmarks |
| `parser_bench.rs` | Parser benchmarks |
| `helpers.rs` | Shared benchmark helpers |

**Naming convention:** `<module>_bench.rs`

## 8. CI/CD

| File | Purpose |
|------|---------|
| `.github/workflows/ci.yml` | Continuous integration |
| `.github/workflows/release.yml` | Release automation |

## 9. Design Documents (`design/`)

| Path | Contents |
|------|----------|
| `design/*.md` | 8 design docs (architecture, ROADMAP, determinism, distribution, error-handling, path-resolution, performance, testing) |
| `design/decisions/log.md` | Architectural decision log |
| `design/specs/archive/` | 16 archived phase specs and implementation plans |
| `design/reports/archive/` | 5 archived architecture reviews and audits |

## 10. Claude/Moira Infrastructure (`.claude/`)

| Path | Purpose |
|------|---------|
| `.claude/commands/` | 5 slash commands: audit-docs, review-architecture, review-plan, review-spec, write-spec |
| `.claude/moira/config/` | budgets.yaml |
| `.claude/moira/knowledge/` | 7 knowledge categories: conventions, decisions, failures, libraries, patterns, project-model, quality-map |
| `.claude/moira/state/` | State directories: audits, init, metrics, reflection, tasks |

## 11. File Counts

| Directory | File Count |
|-----------|------------|
| `src/` | 64 |
| `tests/` | 85 |
| `design/` | 31 |
| `benches/` | 6 |
| `.claude/` | 25 |
| `.github/` | 2 |
| `docs/` | 1 |

## 12. Items Searched For But Not Found

- No `build.rs` (custom build script) at project root
- No `rust-toolchain.toml` or `rust-toolchain` file
- No `.cargo/config.toml` configuration
- No `vendor/` or `third_party/` directories
- No `target/` directory in working tree (expected; gitignored)
- No snapshot files (e.g., `*.snap`) observed at top level of `tests/` (may exist deeper in fixtures)
- No `examples/` directory
- No `src/bin/` directory (single binary via `src/main.rs`)
