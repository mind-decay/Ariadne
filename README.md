<p align="center">
  <strong>A R I A D N E</strong><br>
  <em>Structural dependency graph engine for source code</em>
</p>

<p align="center">
  Parse any project. Map every dependency. See the architecture.
</p>

<p align="center">
  <code>curl -fsSL https://raw.githubusercontent.com/mind-decay/Ariadne/master/remote-install.sh | bash</code>
</p>

---

## What is Ariadne

In Greek mythology, Ariadne gave Theseus a thread to navigate the labyrinth and find his way back. This project does the same for codebases.

Ariadne parses source code via [tree-sitter](https://tree-sitter.github.io/tree-sitter/), extracts every import and dependency relationship, and produces a navigable structural graph — files, imports, architectural layers, module clusters, and cross-cutting metrics. The result is a complete map of how your project is wired together.

It works as a CLI tool for one-off analysis and as a long-running **MCP server** that gives AI agents, IDEs, and CI tools instant access to architectural intelligence — no re-parsing, no context overflow, no guesswork.

```
your-project/
├── src/
│   ├── api/            ← layer: interface
│   ├── services/       ← layer: logic
│   ├── models/         ← layer: data
│   └── utils/          ← layer: utility
└── ...

        ariadne build .
              │
              ▼

.ariadne/graph/
├── graph.json          ← nodes (files) + edges (imports)
├── clusters.json       ← module clusters with cohesion metrics
├── stats.json          ← centrality, cycles, layers, PageRank
└── views/              ← markdown architecture views
```

---

## Why this exists

Understanding a codebase by reading files is slow, incomplete, and doesn't scale. You miss cyclic dependencies, don't see which files are structural bottlenecks, and can't answer "what breaks if I change this?" without grepping through everything.

AI coding agents face the same problem at a larger scale — they spend tens of thousands of tokens re-reading source files every session, building a shallow and inconsistent understanding that evaporates when context resets.

Ariadne makes architecture **queryable**. Build the graph once, ask questions instantly:

- What is the blast radius of changing `auth/middleware.ts`?
- Which files are the most central to the project?
- Are there circular dependencies? Where?
- How coupled are these clusters to each other?
- What does the dependency structure look like at a high level?

---

## Quick start

**Install:**

```bash
curl -fsSL https://raw.githubusercontent.com/mind-decay/Ariadne/master/remote-install.sh | bash
```

**Build a dependency graph:**

```bash
cd your-project
ariadne build .
```

**Query it:**

```bash
ariadne query stats                          # project overview
ariadne query blast-radius src/auth.ts       # what breaks if this changes?
ariadne query cycles                         # circular dependencies
ariadne query centrality --top 10            # structural bottlenecks
ariadne query smells                         # architectural code smells
ariadne query importance --top 10            # PageRank + centrality ranking
ariadne query spectral                       # monolith score, algebraic connectivity
```

**Generate architecture views:**

```bash
ariadne views generate                       # markdown views per cluster
```

**Start the MCP server:**

```bash
ariadne serve                                # instant queries for AI agents & IDEs
```

---

## Installation

### Install script (recommended)

Clones the repo, builds from source with `cargo build --release`, and installs the binary to `/usr/local/bin` (or `~/.local/bin` as fallback). The temp clone is cleaned up automatically.

```bash
curl -fsSL https://raw.githubusercontent.com/mind-decay/Ariadne/master/remote-install.sh | bash
```

**Requirements:** git, [Rust toolchain](https://rustup.rs) (cargo).

### From crates.io

```bash
cargo install ariadne-graph
```

### Binary download

When release binaries are available, grab the latest for your platform from [GitHub Releases](https://github.com/mind-decay/Ariadne/releases).

| Platform | Binary |
|---|---|
| macOS (Apple Silicon) | `ariadne-darwin-arm64` |
| macOS (Intel) | `ariadne-darwin-x64` |
| Linux (x86_64) | `ariadne-linux-x64` |
| Linux (ARM64) | `ariadne-linux-arm64` |

---

## Supported languages

| Language | Extensions | Parser |
|---|---|---|
| TypeScript / JavaScript | `.ts` `.tsx` `.js` `.jsx` `.mjs` `.cjs` | tree-sitter-typescript, tree-sitter-javascript |
| Go | `.go` | tree-sitter-go |
| Python | `.py` | tree-sitter-python |
| Rust | `.rs` | tree-sitter-rust |
| C# | `.cs` | tree-sitter-c-sharp |
| Java | `.java` | tree-sitter-java |

Each parser extracts imports, exports, re-exports, and type-only imports where the language supports them. Broken files are skipped with a warning — never a crash.

---

## Commands

### `ariadne build`

Parse a project and produce the full dependency graph.

```bash
ariadne build <path> [options]
```

| Flag | Effect |
|---|---|
| `--output <dir>` | Output directory (default: `<path>/.ariadne/graph/`) |
| `--verbose` | Per-stage timing output |
| `--warnings json` | Machine-readable warning format |
| `--strict` | Exit code 1 on any warnings |
| `--timestamp` | Include generation timestamp in output |

Output: `graph.json`, `clusters.json`, `stats.json`.

### `ariadne query`

Query the built graph without rebuilding.

| Subcommand | What it shows |
|---|---|
| `stats` | Project-wide statistics — file count, edge count, SCC count, layer distribution |
| `blast-radius <file>` | All files transitively affected by a change |
| `subgraph <files...>` | Extract the local neighborhood around specific files |
| `centrality [--top N]` | Betweenness centrality — which files are structural bridges |
| `importance [--top N]` | Combined centrality + PageRank ranking |
| `cycles` | Circular dependency detection (Tarjan SCC) |
| `layers` | Topological layers — build order of the dependency graph |
| `cluster <name>` | Details for a specific cluster |
| `file <path>` | Full details for a specific file node |
| `metrics` | Martin stability/abstractness metrics per cluster |
| `smells` | Architectural smell detection (God files, circular deps, high coupling) |
| `spectral` | Algebraic connectivity, monolith score, Fiedler bisection |
| `compressed [--level]` | Compressed graph view at project / cluster / file level |

### `ariadne views generate`

Generate markdown architecture documentation — L0 project index and L1 per-cluster views.

### `ariadne update`

Incremental update via delta computation. Detects changed files by content hash, rebuilds only what's needed.

### `ariadne serve`

Start a long-running MCP server over stdio. Loads the graph into memory, answers queries instantly, and auto-rebuilds on file changes.

```bash
ariadne serve [options]
```

| Flag | Effect |
|---|---|
| `--project <path>` | Project root to serve (default: `.`) |
| `--output <dir>` | Output directory |
| `--debounce <ms>` | File watcher debounce (default: 2000ms) |
| `--no-watch` | Disable automatic rebuild on file changes |

### `ariadne info`

Show version and list of supported languages.

---

## Output format

### graph.json

The core dependency graph.

**Nodes** (one per source file):
- File type and detected language
- Architectural layer (`interface`, `logic`, `data`, `utility`, `config`, `test`, `unknown`)
- Line count, content hash (xxHash64)
- Exported symbols
- Cluster assignment

**Edges** (one per dependency):
- Source → target file
- Edge type: `imports`, `re_exports`, `type_imports`, `tests`
- Imported symbols

### clusters.json

Directory-based module clusters:
- Files per cluster
- Internal / external edge counts
- Cohesion metric

### stats.json

Project-wide structural analysis:
- Strongly connected components (cycles)
- Betweenness centrality scores
- Topological layer assignment
- PageRank scores
- Martin stability / abstractness metrics
- Summary statistics

---

## MCP server

Ariadne's MCP server makes the dependency graph available to any MCP-compatible consumer — AI coding agents, IDE extensions, CI pipelines.

```json
{
  "mcpServers": {
    "ariadne": {
      "command": "ariadne",
      "args": ["serve", "--project", "/path/to/your/project"]
    }
  }
}
```

The server keeps the graph in memory, watches for file changes, and automatically rebuilds when the codebase changes. Queries that would take seconds via CLI return in milliseconds.

This is how [Moira](https://github.com/mind-decay/Moira) integrates Ariadne — agents get architectural intelligence (blast radius, coupling, cycles, bottlenecks) without reading a single source file.

---

## Performance

| Scenario | Target |
|---|---|
| 100 files | < 200ms |
| 1,000 files | < 3s |
| 3,000 files | < 10s |
| SCC detection | < 10ms |
| Blast radius (BFS) | < 10ms |
| Centrality (Brandes) | < 500ms |
| Topological sort | < 10ms |

Output is **deterministic** — byte-identical across repeated builds on the same input (BTreeMap ordering, sorted serialization, no timestamps by default).

---

## Design

Ariadne is designed before it is built. All implementation conforms to design documents.

| Document | Contents |
|---|---|
| [Architecture](design/architecture.md) | Full system design — data model, parsers, pipeline, CLI, output formats |
| [Roadmap](design/ROADMAP.md) | Implementation phases and build order |
| [Decision Log](design/decisions/log.md) | Architectural decisions (D-001 through D-050+) |
| [Path Resolution](design/path-resolution.md) | Normalization, case sensitivity, monorepo support |
| [Determinism](design/determinism.md) | Byte-identical output strategy |
| [Error Handling](design/error-handling.md) | Error taxonomy (E001–E005, W001–W009), fault tolerance |
| [Performance](design/performance.md) | Performance model, parallelism, memory strategy |
| [Testing](design/testing.md) | 4-level test strategy — snapshots, fixtures, invariants, benchmarks |

---

## License

MIT OR Apache-2.0
