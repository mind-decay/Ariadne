# Determinism

## Problem

Ariadne's output is designed to be committed to git (`architecture.md` Git Tracking Policy). This requires **byte-identical output** when the input hasn't changed. Otherwise, every `ariadne build` produces a meaningless diff.

Three things currently break determinism:

1. **`HashMap` iteration order** — Rust's `HashMap` uses random hashing. JSON object key order for `nodes` and `clusters` will vary between runs.
2. **`rayon` parallel collection order** — `par_iter().collect()` does not guarantee order. Edge list order will vary.
3. **`"generated"` timestamp** — Different every run.

## Solution

### Rule: Same input → byte-identical output

If no source files have changed, `ariadne build` must produce identical `graph.json` and `clusters.json` to the previous run. `diff` on the output files must show zero changes.

### Data Structures

**Internal model uses `BTreeMap` with newtype keys (D-017):**

```rust
pub struct ProjectGraph {
    pub nodes: BTreeMap<CanonicalPath, Node>,  // sorted by canonical path
    pub edges: Vec<Edge>,                       // sorted before conversion
}

pub struct ClusterMap {
    pub clusters: BTreeMap<ClusterId, Cluster>,  // sorted by cluster name
}
```

`CanonicalPath` and `ClusterId` implement `Ord`, so `BTreeMap` sorts by their natural ordering. This eliminates the need for post-hoc sorting of node keys.

**Output model converts newtypes to strings (D-022):**

```rust
pub struct GraphOutput {
    pub nodes: BTreeMap<String, NodeOutput>,     // sorted string keys
    pub edges: Vec<(String, String, String, Vec<String>)>,  // compact tuples, sorted
}
```

Conversion via `project_graph_to_output(graph, project_root)` free function — single place where sort-point enforcement happens. Internal pipeline code doesn't need to worry about serialization ordering.

**BTreeMap everywhere (KISS decision):** We use `BTreeMap` in both internal and output models rather than `HashMap` internally + conversion. The ~20% lookup overhead is negligible compared to parsing. This avoids a category of "forgot to sort" bugs.

**Exception:** `ParserRegistry.extension_index` uses `HashMap` because it is lookup-only (never iterated for output). Its public accessors sort results before returning: `supported_extensions()` sorts the extension list lexicographically.

### Edge Sorting

Edges are serialized as a JSON array. Arrays are order-dependent. Sort before serialization:

```rust
edges.sort_by(|a, b| {
    a.from.cmp(&b.from)
        .then(a.to.cmp(&b.to))
        .then(a.edge_type.cmp(&b.edge_type))
});
```

Sort order: `from` path → `to` path → edge type. This is deterministic and human-readable (edges from the same file are grouped).

### Symbol Sorting (D-077)

`Node.symbols` is a `Vec<SymbolDef>` where `SymbolDef` derives `Ord`. Symbols are sorted via `symbols.sort()` before persistence. The sort key is `(name, kind, visibility, span, signature, parent)` — the natural `Ord` derivation order. This ensures deterministic output even when tree-sitter traversal order varies.

Sort point: `BuildPipeline::extract_symbols_for_file()` sorts after extraction, before storing in `ParsedFile`.

### Parallel Collection

`rayon`'s `par_iter()` on a sorted slice preserves order if you use `par_iter().map().collect::<Vec<_>>()` (rayon maintains index order for indexed iterators).

Strategy:

1. Walk files → collect into `Vec<PathBuf>` → **sort by path**
2. `sorted_files.par_iter().map(|f| process(f)).collect::<Vec<_>>()` — result is in same order as input
3. Edges collected per-file are already grouped — flatten and then sort globally

This gives us deterministic output WITH parallel performance.

### Timestamp

**Remove `"generated"` from default output.** It serves no purpose in a deterministic, git-tracked file — git itself tracks when the file was modified.

Add it only with `--timestamp` flag for debugging:

```bash
ariadne build .                    # no timestamp, deterministic
ariadne build . --timestamp        # includes "generated" field
```

Without `--timestamp`, `graph.json` has no `"generated"` field. Schema version 1 defines it as optional.

### Cluster `files` Lists

Cluster `files` arrays must also be sorted:

```rust
pub struct Cluster {
    pub files: Vec<String>,  // sorted lexicographically
    // ...
}
```

### Exports Lists

Node `exports` and edge `symbols` lists must be sorted:

```rust
node.exports.sort();
edge.symbols.sort();
```

### Floating-Point Determinism

Cluster cohesion is `f64`. To ensure byte-identical JSON output across platforms, cohesion values are rounded to 4 decimal places before serialization (e.g., `0.6500` not `0.6499999999999999`). This prevents platform-dependent float-to-string conversion from producing spurious git diffs.

### Summary of Sort Points

| Data                     | Sort key                  | When                 |
| ------------------------ | ------------------------- | -------------------- |
| `graph.json` nodes       | BTreeMap by path          | Construction time    |
| `graph.json` edges       | (from, to, edge_type)     | Before serialization |
| `clusters.json` clusters | BTreeMap by name          | Construction time    |
| Cluster `files`          | Lexicographic             | Before serialization |
| Node `exports`           | Lexicographic             | Before serialization |
| Edge `symbols`           | Lexicographic             | Before serialization |
| `stats.json` centrality  | BTreeMap by path          | Construction time    |
| `stats.json` sccs        | Inner: lexicographic. Outer: by first element | Before serialization |
| `stats.json` layers      | BTreeMap by layer number (string). Files: lexicographic | Construction time |
| `stats.json` bottleneck_files | Centrality descending, then path ascending | Before serialization |
| `stats.json` orphan_files | Lexicographic by path    | Before serialization |
| `raw_imports.json` keys | `BTreeMap<String, Vec<RawImportOutput>>` — outer keys sorted lexicographically by file path | Construction time |
| `raw_imports.json` import entries | `Vec` order matches `parsed_files` iteration order (sorted by `CanonicalPath` before parsing) | Before serialization |

**Note:** All `stats.json` float values (`centrality`, `avg_in_degree`, `avg_out_degree`) are rounded to 4 decimal places before serialization, following the same pattern as cluster cohesion. See D-034, D-049.

## Verification

**INV-11 (from testing.md) is strengthened:**

Old: "build(project) at T1 = build(project) at T2 IF project files unchanged (edges may be in different order — compare as sets)"

New: **"build(project) at T1 = build(project) at T2 IF project files unchanged (byte-identical output)"**

Test:

```rust
#[test]
fn deterministic_output() {
    let graph1 = build_and_serialize(fixture_path);
    let graph2 = build_and_serialize(fixture_path);
    assert_eq!(graph1, graph2);  // byte-identical, not set comparison
}
```

## Impact on Spec

- **Graph Data Model (architecture.md, D-006, D-017):** `ProjectGraph.nodes` uses `BTreeMap<CanonicalPath, Node>`. `ClusterMap.clusters` uses `BTreeMap<ClusterId, Cluster>`. Newtypes implement `Ord`.
- **Output Model (architecture.md, D-022):** `GraphOutput` uses `BTreeMap<String, NodeOutput>`. Conversion from internal model enforces all sort points in one place.
- **Graph Builder (architecture.md):** Sort edges before conversion. Sort cluster files, node exports, edge symbols.
- **Storage Format (architecture.md):** Remove `"generated"` from default output. Add `--timestamp` flag.
- **CLI Interface (architecture.md):** Add `--timestamp` flag.
- **Performance:** BTreeMap is O(log n) vs HashMap O(1) for lookups. For 50k nodes, this adds ~20% to build phase. Sorting edges (O(n log n)) is negligible compared to parsing. Acceptable trade-off for deterministic output.
