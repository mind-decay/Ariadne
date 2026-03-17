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

**Nodes: `BTreeMap` instead of `HashMap`.**

```rust
pub struct ProjectGraph {
    pub nodes: BTreeMap<String, Node>,  // NOT HashMap — sorted by path
    pub edges: Vec<Edge>,               // sorted before serialization
}

pub struct ClusterMap {
    pub clusters: BTreeMap<String, Cluster>,  // NOT HashMap — sorted by name
}
```

`BTreeMap` iterates in key order (lexicographic by path/name). Serde serializes JSON objects in iteration order. Result: stable key order in JSON output.

**During construction** we can use `HashMap` internally for O(1) lookups. Convert to `BTreeMap` at the serialization boundary:

```rust
// Build phase: HashMap for performance
let mut nodes: HashMap<String, Node> = HashMap::new();
// ... populate ...

// Serialization: convert to BTreeMap for deterministic output
let sorted_nodes: BTreeMap<String, Node> = nodes.into_iter().collect();
```

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

### Summary of Sort Points

| Data | Sort key | When |
|------|----------|------|
| `graph.json` nodes | BTreeMap by path | Construction time |
| `graph.json` edges | (from, to, edge_type) | Before serialization |
| `clusters.json` clusters | BTreeMap by name | Construction time |
| Cluster `files` | Lexicographic | Before serialization |
| Node `exports` | Lexicographic | Before serialization |
| Edge `symbols` | Lexicographic | Before serialization |

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

- **D2 (Data Model):** `ProjectGraph.nodes` type changes from `HashMap` to `BTreeMap`. `ClusterMap.clusters` same.
- **D14 (Graph Builder):** Sort edges before returning. Sort cluster files, node exports, edge symbols.
- **D15 (Serialization):** Remove `"generated"` from default output. Add `--timestamp` flag.
- **D17 (CLI):** Add `--timestamp` flag.
- **Performance:** BTreeMap is O(log n) vs HashMap O(1) for lookups. For 50k nodes, this adds ~20% to build phase. Sorting edges (O(n log n)) is negligible compared to parsing. Acceptable trade-off for deterministic output.
