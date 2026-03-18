# Ariadne

Structural dependency graph engine for source code. Parses projects and produces a navigable dependency graph — files, imports, architectural layers, and module clusters.

Named after Ariadne of Greek mythology, who gave Theseus the thread to navigate the labyrinth.

## Installation

### From source (cargo)

```sh
cargo install ariadne-graph
```

### Binary download

Download the latest release for your platform from [GitHub Releases](https://github.com/anthropics/ariadne/releases).

### Install script

```sh
curl -sSL https://raw.githubusercontent.com/anthropics/ariadne/main/install.sh | bash
```

## Usage

### Build a dependency graph

```sh
ariadne build .
```

Output is written to `.ariadne/graph/graph.json` and `.ariadne/graph/clusters.json`.

### Specify output directory

```sh
ariadne build . --output ./output
```

### Verbose mode (per-stage timing)

```sh
ariadne build . --verbose
```

### JSON warning output

```sh
ariadne build . --warnings json
```

### Strict mode (exit code 1 on warnings)

```sh
ariadne build . --strict
```

### Include generation timestamp

```sh
ariadne build . --timestamp
```

### Show supported languages

```sh
ariadne info
```

## Supported Languages

| Language | Extensions |
|----------|-----------|
| TypeScript/JavaScript | .ts, .tsx, .js, .jsx, .mjs, .cjs |
| Go | .go |
| Python | .py |
| Rust | .rs |
| C# | .cs |
| Java | .java |

## Output Format

### graph.json

Contains the dependency graph with nodes (files) and edges (imports):

- **nodes**: file type, architectural layer, line count, content hash, exports, cluster assignment
- **edges**: from file, to file, edge type (imports/tests/re_exports/type_imports), symbols

### clusters.json

Directory-based module clusters with:

- File lists per cluster
- Internal/external edge counts
- Cohesion metric

## Limitations

- Tier 1 languages only (6 languages listed above)
- No `exports` field resolution in package.json (uses main/module/default probing)
- npm/yarn/pnpm workspaces only (no Go modules, Cargo workspaces, Nx, Turbo)
- No architectural depth computation (placeholder value)

## Performance

| Scenario | Target |
|----------|--------|
| 100 files | <200ms |
| 1,000 files | <3s |
| 3,000 files | <10s |

## Design

See `design/` for comprehensive documentation:

- `design/architecture.md` — full system design
- `design/ROADMAP.md` — implementation phases
- `design/decisions/log.md` — architectural decision log

## License

MIT OR Apache-2.0
