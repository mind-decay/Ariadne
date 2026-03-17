# Performance

## Performance Model

Ariadne's pipeline has 5 stages. Each has different performance characteristics:

```
Walk → Read → Parse → Resolve → Serialize
 I/O    I/O    CPU     CPU       I/O
```

### Time Budget (target: 3000 files in <10s)

| Stage | Operation | Per-file cost | 3000 files | % of total |
|-------|-----------|---------------|------------|------------|
| Walk | Directory traversal + gitignore matching | ~0.01ms | ~30ms | 1% |
| Read | Read file bytes + UTF-8 check + hash | ~0.05ms | ~150ms | 3% |
| Parse | Tree-sitter AST + import/export extraction | ~1ms | ~3000ms | 60% |
| Resolve | Import path resolution (FS lookups) | ~0.3ms | ~900ms | 18% |
| Serialize | JSON generation + file write | — | ~500ms | 10% |
| Overhead | Clustering, edge dedup, metadata | — | ~400ms | 8% |

**Parsing is the bottleneck** (~60% of time). This is why parallelism matters most here.

### Scaling Characteristics

| Project size | Files | Expected time | Memory |
|-------------|-------|---------------|--------|
| Small | 100 | <200ms | ~5MB |
| Medium | 1,000 | <3s | ~10MB |
| Large | 3,000 | <10s | ~25MB |
| Very large | 10,000 | <30s | ~80MB |
| Huge | 50,000 | ~2min | ~400MB |

Scaling is approximately linear in file count. Parsing dominates, and rayon parallelism gives near-linear speedup with core count.

## Parallelism Strategy

### What Parallelizes

**File parsing (rayon):** Tree-sitter parsing + import/export extraction is per-file with no shared state. This is embarrassingly parallel.

```rust
// Conceptual structure
files.par_iter()                    // rayon parallel iterator
    .map(|file| {
        let content = read(file);   // I/O — not parallelized within file
        let tree = parse(content);  // CPU — main benefit of parallelism
        let imports = extract(tree); // CPU
        let exports = extract(tree); // CPU
        let hash = xxhash(content);  // CPU
        (file, imports, exports, hash)
    })
    .collect()
```

**Expected speedup:**
- 1 core: baseline
- 4 cores: ~3.5x (near-linear, slight overhead from rayon scheduling)
- 8 cores: ~6x (diminishing returns from I/O contention on read)
- 16 cores: ~8x (I/O becomes the bottleneck)

### What Does NOT Parallelize

- **Directory walking:** Sequential by nature (filesystem tree traversal). Fast enough (~30ms for 3000 files).
- **Path resolution:** Depends on the full file list being known (need to check if resolved path exists in the graph). Runs after parsing. Could parallelize per-file but lookups are fast (HashMap).
- **Edge deduplication:** Sequential pass over all edges. Fast (linear).
- **Clustering:** Sequential assignment + metric computation. Fast (linear).
- **JSON serialization:** serde_json is single-threaded. Could parallelize graph.json and clusters.json writes, but not worth the complexity (~500ms total).

### Rayon Configuration

Default thread pool — rayon auto-detects core count. No manual configuration needed.

If users need to limit CPU usage: `RAYON_NUM_THREADS=4 ariadne build .`

Environment variable is standard rayon convention — no custom flag needed.

## Memory Strategy

### Memory Model

All data lives in memory during build. No streaming, no disk-backed storage. This is the right trade-off for projects up to 50k files.

```
Memory layout:
  Nodes:  HashMap<String, Node>     ~500 bytes/node (path + enums + hash + exports vec)
  Edges:  Vec<Edge>                 ~200 bytes/edge (two paths + type + symbols vec)
  Source: Not retained              read once, parsed, discarded
  ASTs:   Not retained              tree-sitter Tree is per-file, dropped after extraction
```

**Key insight:** Source content and ASTs are NOT kept in memory. Each file is: read → parse → extract → drop. Only extracted metadata (Node + edges) is retained. This means memory scales with graph size, not total source size.

### Memory Estimates

| Files | Avg edges/file | Nodes memory | Edges memory | Total |
|-------|---------------|-------------|-------------|-------|
| 100 | 3 | 50KB | 60KB | ~1MB |
| 1,000 | 3 | 500KB | 600KB | ~5MB |
| 3,000 | 3 | 1.5MB | 1.8MB | ~15MB |
| 10,000 | 3 | 5MB | 6MB | ~50MB |
| 50,000 | 3 | 25MB | 30MB | ~250MB |

These are conservative estimates. Real-world average edges/file varies (2-5 depending on language/project).

### Memory Protection

- `--max-files 50000` (default): hard cap on node count. Walk stops after limit.
- `--max-file-size 1MB` (default): prevents reading huge generated files into memory.
- No file content retained after parsing: memory proportional to graph, not source.
- Vec pre-allocation: edges Vec allocated with estimated capacity (files × 3) to avoid reallocations.

## I/O Strategy

### File Reading

```rust
// Read entire file into memory at once
let content = std::fs::read(path)?;
```

**Why `read()` not `mmap()`:**
- Files are small (avg <10KB for source, max 1MB by limit)
- Each file is read exactly once
- mmap overhead (page table setup, TLB) not justified for small sequential reads
- mmap complicates error handling (SIGBUS on truncated files)
- `std::fs::read` is simpler, fast enough, and portable

**Why not buffered streaming:**
- Tree-sitter needs the full source as `&[u8]` — can't parse a stream
- xxHash needs full content for deterministic hash
- Full-read is the simplest correct approach

### Output Writing

```rust
// Atomic write: temp file → rename
let tmp = output_dir.join("graph.json.tmp");
std::fs::write(&tmp, json_bytes)?;
std::fs::rename(&tmp, output_dir.join("graph.json"))?;
```

**Buffered writing:** `serde_json::to_writer` with `BufWriter` for large graphs. Avoids building full JSON string in memory.

```rust
let file = BufWriter::new(File::create(&tmp)?);
serde_json::to_writer_pretty(file, &graph_output)?;
```

This means serialization memory is O(1) (buffer size), not O(graph size).

## Optimization Strategy

### Phase 1: Don't Optimize Prematurely

The pipeline is straightforward: walk → read → parse → resolve → serialize. First implementation should be correct, not fast. Rayon parallelism for parsing is the one optimization included from the start (it's architectural, not micro).

### When to Optimize

Optimize when benchmarks show a specific stage exceeding its time budget:

| Stage | Budget | Optimize if | How |
|-------|--------|-------------|-----|
| Walk | <100ms | >200ms on 3000 files | Profile ignore crate usage, check gitignore patterns |
| Read | <200ms | >500ms on 3000 files | Investigate I/O scheduling, pre-read queue |
| Parse | <3000ms | >5000ms on 3000 files | Profile tree-sitter queries, reduce AST traversals |
| Resolve | <1000ms | >2000ms on 3000 files | Cache resolved paths, batch FS existence checks |
| Serialize | <500ms | >1000ms on 3000 files | Use `serde_json::to_writer` with BufWriter (should already) |

### Profiling Tools

```bash
# CPU profiling (macOS)
cargo instruments --release --template "Time Profiler" -- build /path/to/project

# CPU profiling (Linux)
cargo build --release
perf record ./target/release/ariadne build /path/to/project
perf report

# Memory profiling
cargo build --release
valgrind --tool=massif ./target/release/ariadne build /path/to/project
ms_print massif.out.*

# Quick timing per stage (built-in)
ariadne build /path/to/project --verbose
# Prints per-stage timing in verbose mode
```

### Built-in Timing

In `--verbose` mode, Ariadne prints per-stage timing:

```
[walk]      42ms    3,247 files found
[read+hash] 198ms   3,201 files read (46 skipped)
[parse]     2,341ms 3,201 files parsed (12 warnings)
[resolve]   876ms   8,432 edges created (142 unresolved)
[cluster]   34ms    18 clusters
[serialize] 412ms   graph.json (2.1MB) + clusters.json (24KB)
[total]     3,903ms
```

This is free instrumentation — always available, no profiling tools needed.

## Phase 2 Performance Considerations

When algorithms are added (blast radius, centrality, cycles, Louvain):

| Algorithm | Complexity | 3000 nodes budget | Notes |
|-----------|-----------|-------------------|-------|
| Blast radius (BFS) | O(V + E) | <10ms per query | Linear, fast |
| Betweenness centrality (Brandes) | O(VE) | <500ms | Run once, cache in stats.json |
| Tarjan's SCC | O(V + E) | <10ms | Linear, fast |
| Louvain clustering | O(n log n) | <200ms | Iterative, converges fast |
| Topological sort | O(V + E) | <10ms | Linear, fast |
| Delta computation | O(changed files) | <1s typical | Only re-parses changed files |

All algorithms run on the in-memory graph (already loaded). No I/O during computation.

**Delta computation** is the key Phase 2 performance feature: instead of full rebuild, only re-parse files whose content hash changed. Typical delta (10 files changed out of 3000) should complete in <1s.

## Benchmark Targets Summary

| Benchmark | Target | Regression threshold |
|-----------|--------|---------------------|
| 100 files build | <200ms | >20% slower |
| 1000 files build | <3s | >20% slower |
| 3000 files build | <10s | >20% slower |
| Single file parse (TS, 50 imports) | <5ms | >50% slower |
| Single file parse (Go, 30 imports) | <3ms | >50% slower |
| Single file parse (Python, 40 imports) | <3ms | >50% slower |
| xxHash64 1MB file | <1ms | >100% slower |
| Clustering 3000 nodes | <100ms | >50% slower |
| JSON serialization 3000 nodes | <500ms | >50% slower |

Thresholds are intentionally generous. A 20% regression on build is significant (human-noticeable). A 50% regression on single-file parse may be noise on small inputs.
