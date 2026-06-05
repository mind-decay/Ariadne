# Project Architecture Overview

## Synopsis

12 crate(s) · 3 layer(s) · 1625 source symbol(s) · 992 dependency edge(s) · languages: typescript, rust.

## Architecture

![architecture](codebase-overview.svg)

| Crate | Layer | Role |
| --- | --- | --- |
| `ariadne-cli` | Interior | Volatile leaf module — depends outward, little depended upon. |
| `ariadne-core` | Domain | Stable foundational module — many dependents, few dependencies. |
| `ariadne-daemon` | Domain | Volatile leaf module — depends outward, little depended upon. |
| `ariadne-e2e` | Interior | Isolated module — no coupling to the rest of the graph. |
| `ariadne-git` | Adapter | Stable foundational module — many dependents, few dependencies. |
| `ariadne-graph` | Domain | Stable foundational module — many dependents, few dependencies. |
| `ariadne-mcp` | Interior | Volatile leaf module — depends outward, little depended upon. |
| `ariadne-parser` | Adapter | Isolated module — no coupling to the rest of the graph. |
| `ariadne-salsa` | Domain | Stable foundational module — many dependents, few dependencies. |
| `ariadne-scip` | Interior | Isolated module — no coupling to the rest of the graph. |
| `ariadne-storage` | Adapter | Volatile leaf module — depends outward, little depended upon. |
| `ariadne-watcher` | Adapter | Volatile leaf module — depends outward, little depended upon. |

## Boundary violations

- `ariadne-storage::migrate_v2_to_v3` → `ariadne-storage::decode_value` — domain → adapter
- `ariadne-storage::migrate_v2_to_v3` → `ariadne-storage::encode_value` — domain → adapter
- `ariadne-storage::migrate_v6_to_v7` → `ariadne-storage::decode_value` — domain → adapter
- `ariadne-storage::migrate_v6_to_v7` → `ariadne-storage::encode_value` — domain → adapter

## Cycle clusters

2 dependency cluster(s) detected.

- 2 members (`ariadne-scip::default_parallelism`, `ariadne-scip::new`) — suggested cut: `ariadne-scip::default_parallelism` → `ariadne-scip::new`
- 2 members (`ariadne-graph::add_edge`, `ariadne-graph::add_edge_weighted`) — suggested cut: `ariadne-graph::add_edge` → `ariadne-graph::add_edge_weighted`

## Risk hot-spots

| File | Churn | Complexity | Risk |
| --- | --- | --- | --- |
| `crates/ariadne-mcp/src/server.rs` | 12 | 148 | 0.15 |
| `crates/ariadne-parser/src/adapters/treesitter/facts.rs` | 9 | 130 | 0.10 |
| `crates/ariadne-storage/src/adapters/redb/mod.rs` | 6 | 99 | 0.05 |
| `crates/ariadne-cli/src/commands/query.rs` | 5 | 117 | 0.05 |
| `crates/ariadne-cli/src/domain/mod.rs` | 9 | 61 | 0.05 |
| `crates/ariadne-e2e/src/domain/mod.rs` | 4 | 98 | 0.03 |
| `crates/ariadne-graph/src/docgen_insights.rs` | 4 | 91 | 0.03 |
| `crates/ariadne-cli/src/commands/setup.rs` | 6 | 57 | 0.03 |
| `tools/ariadne-sfc-scip/src/index.ts` | 3 | 114 | 0.03 |
| `crates/ariadne-storage/src/domain/migration.rs` | 6 | 54 | 0.03 |

## Refactor & change-coupling

**God modules.** None detected.

**Hidden change-coupling.** 
- `crates/ariadne-scip/src/indexer/plan.rs` ⇄ `crates/ariadne-scip/src/lib.rs` — 5 shared commit(s), degree 0.83
- `crates/ariadne-core/src/domain/ports.rs` ⇄ `crates/ariadne-storage/src/adapters/redb/mod.rs` — 5 shared commit(s), degree 0.77
- `crates/ariadne-mcp/src/tools/mod.rs` ⇄ `crates/ariadne-mcp/src/types.rs` — 5 shared commit(s), degree 0.77
- `crates/ariadne-scip/src/indexer/mod.rs` ⇄ `crates/ariadne-scip/src/indexer/plan.rs` — 5 shared commit(s), degree 0.77
- `crates/ariadne-scip/src/indexer/mod.rs` ⇄ `crates/ariadne-scip/src/lib.rs` — 5 shared commit(s), degree 0.67
- `crates/ariadne-mcp/src/server.rs` ⇄ `crates/ariadne-mcp/src/types.rs` — 6 shared commit(s), degree 0.60
- `crates/ariadne-mcp/src/server.rs` ⇄ `crates/ariadne-mcp/src/tools/mod.rs` — 5 shared commit(s), degree 0.59
- `crates/ariadne-core/src/domain/ports.rs` ⇄ `crates/ariadne-core/src/lib.rs` — 5 shared commit(s), degree 0.56
