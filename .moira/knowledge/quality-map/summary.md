<!-- moira:freshness ariadne-init 2026-04-04 λ=0.07 -->

# Quality Map Summary

## Problematic
- CircularDependency: src/algo/blast_radius.rs
- CircularDependency: src/parser/config/mod.rs
- CircularDependency: src/semantic/events.rs
- CircularDependency: tests/fixtures/edge-cases/circular-a.ts
- DeadCluster: .claude/CLAUDE.md
- DeadCluster: .github/workflows/ci.yml
- DeadCluster: .moira/config.yaml
- DeadCluster: benches/algo_bench.rs
- DeadCluster: design/ROADMAP.md
- DeadCluster: tests/callgraph_tests.rs
- HubAndSpoke: src/diagnostic.rs
- HubAndSpoke: src/model/mod.rs
- HubAndSpoke: src/serial/mod.rs
- Circular dependency: src/algo/blast_radius.rs, src/algo/centrality.rs, src/algo/context.rs, src/algo/impact.rs, src/algo/mod.rs, src/algo/pagerank.rs, src/algo/reading_order.rs, src/algo/scc.rs, src/algo/spectral.rs, src/algo/stats.rs, src/algo/test_map.rs, src/algo/topo_sort.rs
- Circular dependency: src/parser/config/mod.rs, src/parser/config/tsconfig.rs, src/parser/go.rs, src/parser/mod.rs, src/parser/python.rs, src/parser/registry.rs, src/parser/typescript.rs
- Circular dependency: src/semantic/events.rs, src/semantic/http.rs, src/semantic/mod.rs
- Circular dependency: tests/fixtures/edge-cases/circular-a.ts, tests/fixtures/edge-cases/circular-b.ts
- Hotspot: src/parser/typescript.rs
- Hotspot: src/parser/rust_lang.rs
- Hotspot: src/diagnostic.rs
- Hotspot: src/algo/louvain.rs
- Hotspot: src/parser/python.rs
- Hotspot: src/analysis/smells.rs
- Hotspot: src/mcp/tools.rs
- Hotspot: src/parser/go.rs
- Hotspot: src/pipeline/mod.rs
- Hotspot: src/parser/csharp.rs
- Hotspot: src/parser/java.rs
- Hotspot: src/parser/registry.rs
- Hotspot: src/model/types.rs


<!-- moira:freshness:previous ariadne-init 2026-04-04 λ=0.07 -->
