<!-- moira:deep-scan dependencies 2026-03-21 -->

# Deep Dependency Analysis — 2026-03-21

Source: `/Users/minddecay/Documents/Projects/Ariadne/Cargo.toml` and 65 source files under `src/`.

---

## 1. Package Versions — All Declared Dependencies

### Runtime Dependencies (`[dependencies]`)

| Crate | Version | Features | Optional |
|---|---|---|---|
| clap | 4 | derive | no |
| tree-sitter | 0.24 | — | no |
| tree-sitter-typescript | 0.23 | — | no |
| tree-sitter-javascript | 0.23 | — | no |
| tree-sitter-go | 0.23 | — | no |
| tree-sitter-python | 0.23 | — | no |
| tree-sitter-rust | 0.23 | — | no |
| tree-sitter-c-sharp | 0.23 | — | no |
| tree-sitter-java | 0.23 | — | no |
| serde | 1 | derive | no |
| serde_json | 1 | — | no |
| xxhash-rust | 0.8 | xxh64 | no |
| ignore | 0.4 | — | no |
| rayon | 1 | — | no |
| thiserror | 2 | — | no |
| dunce | 1 | — | no |
| time | 0.3 | formatting | no |
| glob | 0.3 | — | no |
| rmcp | 1.2 | server, transport-io | yes (feature: serve) |
| schemars | 1 | — | yes (feature: serve) |
| tokio | 1 | rt-multi-thread, macros, signal, sync, time | yes (feature: serve) |
| tokio-util | 0.7 | — | yes (feature: serve) |
| arc-swap | 1 | — | yes (feature: serve) |
| notify | 8 | — | yes (feature: serve) |
| notify-debouncer-full | 0.7 | — | yes (feature: serve) |

### Dev Dependencies (`[dev-dependencies]`)

| Crate | Version | Features |
|---|---|---|
| insta | 1 | yaml |
| tempfile | 3 | — |
| proptest | 1 | — |
| criterion | 0.5 | html_reports |
| serde_json | 1 | — |

### Feature Configuration

- Default features: `["serve"]`
- `serve` feature enables: rmcp, tokio, tokio-util, arc-swap, notify, notify-debouncer-full, schemars

---

## 2. Unused Imports — Declared but Never Imported in Source

### Runtime Dependencies

| Crate | Declared in | Evidence of non-use |
|---|---|---|
| **dunce** | `Cargo.toml` line 28 | Zero matches for `dunce` across all 65 files in `src/`. No `use dunce`, no `dunce::` references. |
| **tree-sitter-javascript** | `Cargo.toml` line 16 | Zero references to `tree_sitter_javascript` in any source file. The TypeScript parser at `src/parser/typescript.rs` uses `tree_sitter_typescript::LANGUAGE_TYPESCRIPT` but never references the JavaScript grammar. |

### Dev Dependencies

| Crate | Declared in | Evidence of non-use |
|---|---|---|
| **insta** | `Cargo.toml` line 46 | Zero matches for `insta` in `src/` or `tests/`. No snapshot test macros (`assert_snapshot!`, `assert_yaml_snapshot!`) found anywhere. |

### Notes on Marginal Usage

- `thiserror` — used once: `src/diagnostic.rs` line 9 (`#[derive(thiserror::Error)]`)
- `glob` — used once: `src/detect/workspace.rs` line 174 (`glob::glob(...)`)
- `xxhash-rust` — used once: `src/hash.rs` line 5 (`xxhash_rust::xxh64::xxh64(...)`)
- `rayon` — used once: `src/pipeline/mod.rs` line 9 (`use rayon::prelude::*`)
- `time` (crate, not std::time) — used in 2 locations: `src/mcp/lock.rs:128` and `src/pipeline/mod.rs:606`

---

## 3. Circular Dependencies — Internal Module Analysis

### Module Dependency Graph (top-level modules)

Derived from all `use crate::` statements across `src/`:

```
model        → (leaf — no internal crate dependencies)
diagnostic   → model
hash         → model
detect       → model, diagnostic
parser       → model (parser submodules reference parser::traits internally)
cluster      → model
serial       → model, diagnostic
algo         → model (algo submodules reference algo:: internally)
views        → model, diagnostic
analysis     → model, algo
pipeline     → model, diagnostic, hash, detect, cluster, parser, serial, algo
mcp          → model, diagnostic, algo, analysis, pipeline, serial, parser
```

### Circular Dependencies Found: **None**

The module graph is a strict DAG. No top-level module cycle exists. The dependency flow is:

```
model (leaf)
  ↑
diagnostic, hash
  ↑
detect, parser, cluster, serial, algo
  ↑
views, analysis
  ↑
pipeline
  ↑
mcp (root)
```

Internal sub-module references (`algo::round4` from `algo::pagerank`, etc.) are within-module and do not constitute cross-module cycles.

### Cross-Module Dependency Details

| Module | Depends on (other modules) | Evidence files |
|---|---|---|
| model | — | (leaf) |
| diagnostic | model | `src/diagnostic.rs:5` |
| hash | model | `src/hash.rs:1` |
| detect | model, diagnostic | `src/detect/layer.rs:1`, `src/detect/workspace.rs:4-6`, `src/detect/filetype.rs:1`, `src/detect/case_sensitivity.rs:3` |
| parser | model | `src/parser/traits.rs:1-2`, `src/parser/typescript.rs:1-2`, and all language parsers |
| cluster | model | `src/cluster/mod.rs:3` |
| serial | model, diagnostic | `src/serial/mod.rs:9-10`, `src/serial/json.rs:8-9` |
| algo | model | `src/algo/mod.rs:15`, `src/algo/scc.rs:4`, `src/algo/centrality.rs:4`, etc. |
| views | model, diagnostic | `src/views/mod.rs:8-9`, `src/views/cluster.rs:4`, etc. |
| analysis | model, algo | `src/analysis/metrics.rs:5-6`, `src/analysis/smells.rs:3-5`, `src/analysis/diff.rs:3-6` |
| pipeline | model, diagnostic, hash, detect, cluster, parser, serial, algo | `src/pipeline/mod.rs:11-17`, `src/pipeline/build.rs:3-7`, etc. |
| mcp | model, diagnostic, algo, analysis, pipeline, serial, parser | `src/mcp/state.rs:5-9`, `src/mcp/server.rs:11-18`, `src/mcp/tools.rs:14-17`, `src/mcp/watch.rs:11-15` |

---

## 4. Duplicate Functionality — Packages Serving Same Purpose

### Observed Overlaps

| Area | Crates | Status |
|---|---|---|
| **JSON serialization** | `serde_json` (runtime) + `serde_json` (dev-dependency) | The dev-dependency re-declaration at `Cargo.toml` line 50 is redundant — runtime `serde_json` is already available to tests. Cargo handles this correctly but the declaration is unnecessary. |
| **File watching** | `notify` + `notify-debouncer-full` | Not a duplicate — `notify-debouncer-full` wraps `notify` to provide debounced events. Both are required for the `serve` feature. Used together in `src/mcp/watch.rs`. |
| **Async runtime** | `tokio` + `tokio-util` | Not a duplicate — `tokio-util` provides `CancellationToken` (used in `src/mcp/server.rs:9`) which is not in core `tokio`. |

### No True Duplicates Found

All declared dependencies serve distinct purposes. The only redundancy is the `serde_json` double declaration in both `[dependencies]` and `[dev-dependencies]`.

---

## 5. External Crate Usage Summary by Source File

| Crate | Files using it |
|---|---|
| clap | `src/main.rs` |
| tree-sitter | `src/parser/registry.rs`, `src/parser/traits.rs`, `src/parser/typescript.rs`, `src/parser/python.rs`, `src/parser/go.rs`, `src/parser/rust_lang.rs`, `src/parser/java.rs`, `src/parser/csharp.rs` |
| tree-sitter-typescript | `src/parser/typescript.rs` |
| tree-sitter-javascript | (none) |
| tree-sitter-go | `src/parser/go.rs` |
| tree-sitter-python | `src/parser/python.rs` |
| tree-sitter-rust | `src/parser/rust_lang.rs` |
| tree-sitter-c-sharp | `src/parser/csharp.rs` |
| tree-sitter-java | `src/parser/java.rs` |
| serde | `src/serial/mod.rs`, `src/model/node.rs`, `src/model/edge.rs`, `src/model/diff.rs`, `src/model/compress.rs`, `src/model/stats.rs`, `src/model/smell.rs`, `src/algo/spectral.rs`, `src/analysis/metrics.rs`, `src/mcp/lock.rs`, `src/mcp/tools.rs` |
| serde_json | `src/serial/json.rs`, `src/main.rs`, `src/diagnostic.rs` (tests), `src/mcp/tools.rs`, `src/mcp/lock.rs`, `src/algo/compress.rs`, `src/detect/workspace.rs` |
| xxhash-rust | `src/hash.rs` |
| ignore | `src/pipeline/walk.rs` |
| rayon | `src/pipeline/mod.rs` |
| thiserror | `src/diagnostic.rs` |
| dunce | (none) |
| time | `src/mcp/lock.rs`, `src/pipeline/mod.rs` |
| glob | `src/detect/workspace.rs` |
| rmcp | `src/mcp/server.rs`, `src/mcp/tools.rs` |
| schemars | `src/mcp/tools.rs` |
| tokio | `src/main.rs` (runtime creation), `src/mcp/server.rs` |
| tokio-util | `src/mcp/server.rs` |
| arc-swap | `src/mcp/watch.rs`, `src/mcp/server.rs`, `src/mcp/tools.rs` |
| notify | `src/mcp/watch.rs` |
| notify-debouncer-full | `src/mcp/watch.rs` |

---

*Scan covered 65 source files in `src/`, test files in `tests/`, and bench files in `benches/`. All findings are based on static text analysis of import statements and crate references.*
