<p align="center">
  <img src="assets/ariadne-wordmark.svg" alt="Ariadne" width="420">
</p>

<p align="center">
  <strong>Local-first code intelligence for Claude.</strong><br>
  A live semantic graph of your repository — symbols, references, and dependency
  edges — answered in one call, kept fresh as you edit.
</p>

<p align="center">
  <a href="LICENSE.md"><img src="https://img.shields.io/badge/license-PolyForm--NC--1.0.0-orange" alt="License: PolyForm Noncommercial 1.0.0"></a>
  <img src="https://img.shields.io/badge/rust-1.85%2B-blue" alt="Rust 1.85+">
  <a href="https://github.com/mind-decay/ariadne/actions/workflows/ci.yml"><img src="https://img.shields.io/github/actions/workflow/status/mind-decay/ariadne/ci.yml" alt="CI status"></a>
  <a href="https://github.com/mind-decay/ariadne/releases"><img src="https://img.shields.io/github/v/release/mind-decay/ariadne" alt="Latest release"></a>
</p>

Ariadne indexes a project into an incrementally-updated, multi-language semantic
graph and exposes it to Claude Code through an
[MCP](https://modelcontextprotocol.io) stdio server. Ask "what breaks if I change
this?" and get an answer from real edges instead of a guess.

## Table of Contents

- [Why Ariadne](#why-ariadne)
- [See it in action](#see-it-in-action)
- [What you can ask](#what-you-can-ask)
- [Performance](#performance)
- [Install](#install)
- [Quickstart](#quickstart)
- [Claude Code integration](#claude-code-integration)
- [Commands](#commands)
- [Tool catalog](#tool-catalog)
- [Language support](#language-support)
- [Troubleshooting](#troubleshooting)
- [Architecture](#architecture)
- [Contributing](#contributing)
- [Commercial licensing](#commercial-licensing)
- [License](#license)
- [Acknowledgements](#acknowledgements)

## Why Ariadne

Claude opens every session blind to the repository. To answer "who calls this?"
or "what does this change affect?", the agent runs `grep`, opens files, greps
again — each round burns context, and text search still misses the edges that
matter: a call through a trait, a re-export, a dynamic dispatch, a cross-crate
dependency. The answers come back as plausible guesses.

Ariadne indexes the project **once** into a graph of symbols, references, and
dependency edges, keeps that index **fresh** as files change, and answers
structural questions — blast radius, plan-assist, coupling, dead code, cycles —
in a **single call** from the real graph.

|                              | `grep` + read                       | Ariadne                                      |
|------------------------------|-------------------------------------|----------------------------------------------|
| "Who calls `X`?"             | Many searches; misses indirect uses | One call, the full reference set             |
| "What breaks if I change `X`?" | Guesswork                         | `blast_radius` — must-touch + may-touch      |
| Cross-file / cross-crate edges | Missed by string match            | Resolved via the SCIP semantic layer         |
| Context cost                 | Grows with every search             | One tool call, one structured result         |
| Freshness                    | Re-search every time                | Incremental — the index tracks your edits    |

Everything runs locally. There is no network egress: the MCP server is spawned
per Claude session and reads a per-project `.ariadne/` index. A background daemon
holds the graph in RAM so queries skip cold start.

## See it in action

Inside a Claude Code session you ask in plain language — *"what's the blast
radius of `DaemonClient`?"* — and Claude calls the tool. The same query from the
shell, `ariadne query <tool> '<json>'`, returns the raw result:

```sh
$ ariadne query blast_radius '{"symbol":"DaemonClient"}'
{
  "symbol": { "name": "DaemonClient", "kind": "struct",
              "file": "crates/ariadne-mcp/src/adapters/daemon_client.rs" },
  "must_touch": [
    { "name": "new",       "file": "crates/ariadne-mcp/src/adapters/daemon_client.rs" },
    { "name": "try_query", "file": "crates/ariadne-mcp/src/adapters/daemon_client.rs" },
    { "name": "run",       "file": "crates/ariadne-mcp/benches/concurrent.rs" }
  ],
  "may_touch": [ ... ]
}
```

```sh
$ ariadne query hotspots '{"limit":3}'
{ "rows": [
    { "file": "crates/ariadne-mcp/src/server.rs",                 "churn": 18, "complexity": 175 },
    { "file": "crates/ariadne-parser/src/adapters/treesitter/facts.rs", "churn": 9, "complexity": 130 },
    { "file": "crates/ariadne-cli/src/commands/query.rs",         "churn":  9, "complexity": 127 } ],
  "note": "Showing 3 of 850 hotspots — call again with next_cursor for the next page." }
```

Questions Claude answers from the graph instead of guessing:

- *Where is `setup` defined, and who calls it?*
- *What files do I need to touch to change the indexing pipeline?*
- *Which modules are the most coupled / the riskiest to edit?*
- *What tests does my current diff actually reach?*
- *Is this change a breaking (major) API bump?*
- *Document the `ariadne-graph` module / the whole project.*

## What you can ask

| Question | Tools |
|---|---|
| Where is a symbol defined or used? | `find_definition`, `find_references`, `list_symbols`, `search_code` |
| What does a change affect? | `blast_radius`, `plan_assist`, `diff_blast_radius`, `affected_tests` |
| Is this change API-breaking? | `api_surface_diff`, `fitness_report` |
| Where is the structural risk? | `coupling_report`, `weak_spots`, `refactor_suggestions` |
| What is churning and complex? | `hotspots`, `complexity`, `co_change` |
| Read code cheaply | `read_symbol`, `read_outline`, `file_summary` |
| Summarize a symbol, file, or project | `doc_for`, `doc_for_module`, `doc_for_project` |

See the full [Tool catalog](#tool-catalog) for all 23.

## Performance

A background daemon keeps the in-RAM graph warm, so a query is a round-trip, not
a cold start:

- **Warm query: ~3–4 ms** end-to-end (CLI → daemon → JSON) on this repository
  (419 files, 4.2K symbols, 12.7K edges) — measured with `ariadne query`.
- **Incremental edit: ~0.6 ms** per single-token change on a 10 MB file
  [src: [ADR-0005](docs/adr/0005-tier-03-parse-slo-baseline.md)].

The project holds itself to these budgets, verified end-to-end on a 100K-file
workload [src: [plan constraints](.claude/plans/ariadne-core/plan.md);
[ADR-0005](docs/adr/0005-tier-03-parse-slo-baseline.md)]:

| Operation | Budget (100K-file workload) |
|---|---|
| Cold full index | < 60 s |
| Incremental update | p95 < 500 ms |
| Query | p95 < 100 ms |

`ariadne mem` reports salsa per-table memory against a 256 MiB-per-table budget.

## Install

Ariadne ships a single static `ariadne` binary. Pick a channel:

### Homebrew (macOS, Linux)

```sh
brew install mind-decay/homebrew-tap/ariadne-cli
```

### Shell installer (macOS, Linux)

```sh
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/mind-decay/ariadne/releases/latest/download/ariadne-cli-installer.sh | sh
```

### Prebuilt archives

Download a `.tar.xz` for your platform from the
[Releases page](https://github.com/mind-decay/ariadne/releases) — macOS
(arm64, x64) and Linux (arm64, x64) — and put `ariadne` on your `PATH`. Windows
is not yet a published release target.

### From crates.io

```sh
cargo install ariadne-cli
```

> Available after the crates.io publish (tier-06).

### From source

```sh
cargo install --path crates/ariadne-cli
```

Requires a stable Rust toolchain (1.85+).

## Quickstart

```sh
cd your-project
ariadne setup     # write .ariadne/ config + .mcp.json + the CLAUDE.md block
ariadne index     # cold-index the repository into .ariadne/index.redb
ariadne status    # confirm file / symbol / edge counts
```

`ariadne setup` is the one-command onboarding path: it scaffolds `.ariadne/`,
registers the `ariadne` MCP server in the project's `.mcp.json`, and writes a
discoverability block into `CLAUDE.md` so Claude prefers the Ariadne tools. It
runs no index — `ariadne index` stays an explicit step. Then open a Claude Code
session in the project and ask structural questions; verify the indexers you
have with `ariadne status`.

## Claude Code integration

`ariadne setup` writes the project's `.mcp.json` automatically:

```json
{
  "mcpServers": {
    "ariadne": {
      "command": "/absolute/path/to/ariadne",
      "args": ["serve", "--watch"],
      "env": {}
    }
  }
}
```

`command` is the absolute path of the `ariadne` binary that ran `setup`, so it
resolves even when `ariadne` is not on `PATH`. A pre-existing `.mcp.json` is
merged non-destructively — only the `ariadne` key is inserted or replaced.

`ariadne serve` hosts the MCP stdio server against the project's `.ariadne/`
index. `--watch` also runs the file watcher in the same process so the index
tracks edits made during the session.

## Commands

| Command | Purpose |
|---|---|
| `ariadne setup [root]` | One-shot onboarding: `.ariadne/` config + `.mcp.json` + `CLAUDE.md` block. No index. |
| `ariadne init [root]` | Scaffold `.ariadne/`, write `config.toml`, gitignore it. |
| `ariadne index [root] [--fresh]` | Cold-index the repository into `.ariadne/index.redb`. |
| `ariadne watch [root]` | Watch the repo, log invalidations + apply latency until Ctrl-C. |
| `ariadne serve [root] [--watch]` | Host the MCP stdio server. |
| `ariadne query <tool> [args_json]` | Route one tool query to the warm daemon; JSON in, JSON out. |
| `ariadne outline <file>` | Print a token-cheap folded code skeleton of a file. |
| `ariadne affected-tests` | List the tests a changeset transitively reaches. |
| `ariadne api-diff <base>..<head>` | Classify the public-API delta between two refs as a SemVer bump. |
| `ariadne fitness check` | Check the project against its `ariadne-fitness.toml` architecture rules. |
| `ariadne digest` | Print a compact, agent-shaped project digest for session bootstrap. |
| `ariadne doc` | Write the architecture overview to Markdown + an SVG diagram. |
| `ariadne status [root]` | Print index counts + the indexer availability matrix. |
| `ariadne mem [root]` | Report salsa per-table memory against the 256 MiB budget. |
| `ariadne daemon <start\|stop\|status>` | Manage the background daemon. |

Configuration lives in `.ariadne/config.toml` (generated by `init`). Every
field can be overridden per-run with an `ARIADNE_*` environment variable
(`ARIADNE_ENABLED_LANGS`, `ARIADNE_IGNORE`, `ARIADNE_RESPECT_GITIGNORE`).

## Tool catalog

The MCP server exposes 23 read-only tools. Most are also reachable from the
shell via `ariadne query <tool> '<json-args>'` (`diff_blast_radius` is MCP-only —
it needs the live working-tree diff).

**Navigate & read**

| Tool | What it answers |
|---|---|
| `list_symbols` | Symbols matching a substring + kind filter. |
| `find_definition` | The defining record for a canonical name. |
| `find_references` | Reference / call sites of a symbol. |
| `search_code` | Symbols by name pattern (substring or regex) + path/kind filters. |
| `read_symbol` | A symbol's source straight from disk, by name. |
| `read_outline` | A whole file folded to a token-cheap skeleton + symbol index. |
| `file_summary` | A file's symbols, fan-in/out, top dependencies. |

**Impact & change**

| Tool | What it answers |
|---|---|
| `blast_radius` | Must-touch + may-touch callers of a symbol. |
| `plan_assist` | Ranked files implicated by a symbol change. |
| `diff_blast_radius` | Blast radius of a diff (working tree, a commit, or a ref range). |
| `affected_tests` | Tests a change transitively reaches. |
| `api_surface_diff` | The public-API delta between two refs as a SemVer bump. |

**Architecture & health**

| Tool | What it answers |
|---|---|
| `coupling_report` | Per-file Martin coupling metrics (Ca/Ce/I/A/D). |
| `weak_spots` | Cycles, god modules, dead-code candidates. |
| `refactor_suggestions` | God-module splits, cycle breaks, misplaced symbols. |
| `fitness_report` | Violations of the project's architecture-fitness rules. |

**History & docs**

| Tool | What it answers |
|---|---|
| `hotspots` | Files or symbols ranked by churn × complexity. |
| `complexity` | Files or symbols ranked by McCabe cyclomatic complexity. |
| `co_change` | File pairs that change together in Git history. |
| `doc_for` | A structured doc-like summary for one symbol. |
| `doc_for_module` | Markdown documentation for one file/module. |
| `doc_for_project` | Markdown architecture overview of the project. |
| `project_status` | Project-wide counts, revision, and root. |

Growable tools return a concise default page; pass `verbosity: "detailed"` for
every field and follow the opaque `next_cursor` to page the rest.

## Language support

Any tree-sitter language is indexed *syntactically* (declarations + call edges)
— 14 out of the box: Rust, TypeScript/TSX, JavaScript, Python, Go, Java, Kotlin,
C, C++, C#, Astro, Svelte, Vue. Semantic indexing additionally consumes
[SCIP](https://scip-code.org) from a per-language indexer that must be on
`PATH`. `ariadne status` prints the availability matrix.

| Language | Indexer | Typical install |
|---|---|---|
| Rust | `rust-analyzer` | `rustup component add rust-analyzer` |
| TypeScript / JS | `scip-typescript` | `npm install -g @sourcegraph/scip-typescript` |
| Python | `scip-python` | `npm install -g @sourcegraph/scip-python` |
| Java / Kotlin | `scip-java` | `cs install scip-java` (Coursier) |
| C# / .NET | `scip-dotnet` | `dotnet tool install --global scip-dotnet` |
| C / C++ | `scip-clang` | prebuilt binary from `sourcegraph/scip-clang` releases |
| Go | `lsif-go` + `scip` | `go install github.com/sourcegraph/lsif-go/cmd/lsif-go@latest` |

Go has no first-party SCIP indexer: Ariadne converts `lsif-go` output with the
`scip` CLI (`scip convert`). If an indexer cannot be installed in your
environment, that language still indexes syntactically — only the semantic layer
is skipped.

## Troubleshooting

- **Memory.** `ariadne mem` reports salsa per-table memory; any table over the
  256 MiB budget is flagged and the command exits non-zero. Pair it with
  `ariadne status` to confirm index size.
- **Watcher.** `ariadne watch` logs every invalidation with the wall-clock
  cost of applying it (`<n> us apply`). If edits are not picked up, confirm the
  path is not excluded by `.gitignore` or the `ignore` list in `config.toml`.
- **Stale index.** Re-run `ariadne index --fresh` to discard `.ariadne/index.redb`
  and rebuild from scratch.
- **Missing indexer.** `ariadne status` shows `MISSING` for any SCIP indexer
  not on `PATH`. Install it from the matrix above; syntactic indexing works
  regardless.

## Architecture

Ariadne follows a hexagonal / ports-and-adapters design. See
[`docs/architecture.md`](docs/architecture.md) for the layering,
[`docs/folder-layout.md`](docs/folder-layout.md) for the per-crate layout, and
[`docs/codebase-overview.md`](docs/codebase-overview.md) for a generated
structural snapshot. Architectural decisions are recorded under
[`docs/adr/`](docs/adr/).

## Contributing

Issues and pull requests are welcome. Start with
[`CONTRIBUTING.md`](CONTRIBUTING.md) for the build, test, and commit-message
workflow, and review the [`CODE_OF_CONDUCT.md`](CODE_OF_CONDUCT.md). To report a
security vulnerability, follow [`SECURITY.md`](SECURITY.md) — do not open a
public issue.

## Commercial licensing

Noncommercial use — personal, research, education, nonprofit, and government —
is free under PolyForm Noncommercial. **Any commercial use requires a separate,
paid commercial license** from the copyright holder. See
[`LICENSE-COMMERCIAL.md`](LICENSE-COMMERCIAL.md) for what counts as commercial
use and how to obtain a license.

## License

Ariadne is licensed to the public under the [PolyForm Noncommercial License
1.0.0](LICENSE.md) (SPDX: `PolyForm-Noncommercial-1.0.0`). This license does not
grant commercial rights — see [Commercial licensing](#commercial-licensing).

## Acknowledgements

Ariadne stands on [tree-sitter](https://tree-sitter.github.io) for syntactic
parsing, [SCIP](https://scip-code.org) and Sourcegraph's language indexers for
semantics, the [Model Context Protocol](https://modelcontextprotocol.io) for the
Claude Code integration, and [`dist`](https://github.com/axodotdev/cargo-dist)
for cross-platform releases.
