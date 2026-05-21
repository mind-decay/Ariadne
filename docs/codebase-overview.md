# Project Architecture Overview

## Overview

187 modules · 1991 symbols · 1851 edges · 3 dependency cycle(s).

## Layers

```mermaid
flowchart TD
    g0["crates/ariadne-salsa/tests/smoke.rs"]
    g1["crates/ariadne-salsa/tests/equivalence.rs"]
    g2["crates/ariadne-salsa/tests/durability.rs"]
    g3["crates/ariadne-salsa/benches/edit.rs"]
    g4["crates/ariadne-salsa/src/lib.rs"]
    g5["crates/ariadne-cli/src/commands/mem.rs"]
    g6["crates/ariadne-salsa/src/memory.rs"]
    g7["crates/ariadne-graph/tests/golden_repo.rs"]
    g8["crates/ariadne-graph/tests/synthetic.rs"]
    g9["crates/ariadne-graph/benches/blast.rs"]
    g10["crates/ariadne-watcher/benches/sink.rs"]
    g11["crates/ariadne-watcher/src/adapters/notify.rs"]
    g12["crates/ariadne-watcher/tests/reconcile.rs"]
    g13["crates/ariadne-watcher/src/adapters/reconcile.rs"]
    g14["crates/ariadne-watcher/tests/events.rs"]
    g15["crates/ariadne-mcp/src/tools/blast_radius.rs"]
    g16["crates/ariadne-mcp/src/tools/doc_for.rs"]
    g17["crates/ariadne-salsa/src/derived.rs"]
    g18["crates/ariadne-graph/tests/docgen_fixture.rs"]
    g19["crates/ariadne-graph/tests/refactor_cases.rs"]
    g20["crates/ariadne-mcp/src/tools/doc_project.rs"]
    g21["crates/ariadne-mcp/src/tools/doc_module.rs"]
    g22["crates/ariadne-graph/src/docgen.rs"]
    g23["crates/ariadne-mcp/src/tools/refactor.rs"]
    g24["crates/ariadne-graph/src/refactor.rs"]
    g25["crates/ariadne-e2e/tests/mcp_session.rs"]
    g26["crates/ariadne-e2e/tests/repos/typescript.rs"]
    g27["crates/ariadne-e2e/tests/repos/java.rs"]
    g28["crates/ariadne-e2e/tests/repos/rust.rs"]
    g29["crates/ariadne-e2e/tests/repos/go.rs"]
    g30["crates/ariadne-e2e/tests/repos/python.rs"]
    g31["crates/ariadne-e2e/tests/repos/csharp.rs"]
    g32["crates/ariadne-e2e/tests/slo.rs"]
    g33["crates/ariadne-storage/tests/changeset.rs"]
    g34["crates/ariadne-parser/fixtures/csharp/Sample.cs"]
    g35["crates/ariadne-mcp/src/tools/weak_spots.rs"]
    g36["crates/ariadne-mcp/src/tools/coupling_report.rs"]
    g37["crates/ariadne-graph/src/coupling.rs"]
    g38["crates/ariadne-graph/src/dead.rs"]
    g39["crates/ariadne-graph/src/plan_assist.rs"]
    g40["crates/ariadne-mcp/src/tools/file_summary.rs"]
    g41["crates/ariadne-graph/src/blast.rs"]
    g42["crates/ariadne-storage/benches/apply.rs"]
    g43["crates/ariadne-scip/tests/ingest_java.rs"]
    g44["crates/ariadne-scip/tests/ingest_typescript.rs"]
    g45["crates/ariadne-scip/tests/ingest_python.rs"]
    g46["crates/ariadne-scip/tests/ingest_go.rs"]
    g47["crates/ariadne-scip/tests/ingest_clang.rs"]
    g48["crates/ariadne-scip/tests/ingest_csharp.rs"]
    g49["crates/ariadne-scip/tests/ingest_rust.rs"]
    g50["crates/ariadne-mcp/src/tools/find_definition.rs"]
    g51["crates/ariadne-mcp/src/tools/list_symbols.rs"]
    g52["crates/ariadne-scip/tests/roundtrip.rs"]
    g53["crates/ariadne-scip/src/indexer/scip_java.rs"]
    g54["crates/ariadne-scip/src/indexer/scip_python.rs"]
    g55["crates/ariadne-scip/src/indexer/lsif_go.rs"]
    g56["crates/ariadne-scip/src/indexer/scip_dotnet.rs"]
    g57["crates/ariadne-scip/src/indexer/scip_clang.rs"]
    g58["crates/ariadne-scip/src/indexer/scip_typescript.rs"]
    g59["crates/ariadne-scip/src/indexer/rust_analyzer.rs"]
    g60["crates/ariadne-parser/fixtures/javascript/sample.js"]
    g61["crates/ariadne-cli/src/main.rs"]
    g62["crates/ariadne-cli/src/commands/init.rs"]
    g63["crates/ariadne-watcher/tests/ignore.rs"]
    g64["crates/ariadne-watcher/src/adapters/sink.rs"]
    g65["crates/ariadne-watcher/src/adapters/ignore.rs"]
    g66["crates/ariadne-storage/tests/mvcc.rs"]
    g67["crates/ariadne-cli/src/commands/index.rs"]
    g68["crates/ariadne-cli/src/commands/status.rs"]
    g69["crates/ariadne-cli/src/commands/query.rs"]
    g70["crates/ariadne-mcp/tests/tools_coupling_report.rs"]
    g71["crates/ariadne-mcp/tests/shutdown.rs"]
    g72["crates/ariadne-mcp/tests/tools_plan_assist.rs"]
    g73["crates/ariadne-mcp/tests/tools_doc_for.rs"]
    g74["crates/ariadne-mcp/tests/tools_weak_spots.rs"]
    g75["crates/ariadne-mcp/tests/tools_doc.rs"]
    g76["crates/ariadne-mcp/tests/handshake.rs"]
    g77["crates/ariadne-mcp/tests/tools_find_definition.rs"]
    g78["crates/ariadne-mcp/tests/tools_refactor.rs"]
    g79["crates/ariadne-mcp/tests/tools_list_symbols.rs"]
    g80["crates/ariadne-mcp/tests/tools_file_summary.rs"]
    g81["crates/ariadne-mcp/tests/tools_blast_radius.rs"]
    g82["crates/ariadne-mcp/tests/tools_find_references.rs"]
    g83["crates/ariadne-mcp/tests/tools_project_status.rs"]
    g84["crates/ariadne-mcp/tests/support.rs"]
    g85["crates/ariadne-mcp/benches/concurrent.rs"]
    g86["crates/ariadne-mcp/benches/cold_start.rs"]
    g87["crates/ariadne-mcp/src/tools/project_status.rs"]
    g88["crates/ariadne-mcp/src/tools/plan_assist.rs"]
    g89["crates/ariadne-mcp/src/tools/mod.rs"]
    g90["crates/ariadne-mcp/src/tools/find_references.rs"]
    g91["crates/ariadne-mcp/src/catalog.rs"]
    g92["crates/ariadne-graph/src/cycles.rs"]
    g93["crates/ariadne-scip/tests/normalize.rs"]
    g94["crates/ariadne-scip/src/normalize/grammar.rs"]
    g95["crates/ariadne-scip/src/indexer/subprocess.rs"]
    g96["crates/ariadne-core/benches/ids.rs"]
    g97["crates/ariadne-parser/tests/real_world.rs"]
    g98["crates/ariadne-parser/tests/facts_rust.rs"]
    g99["crates/ariadne-parser/tests/facts_csharp.rs"]
    g100["crates/ariadne-parser/tests/facts_python.rs"]
    g101["crates/ariadne-parser/tests/facts_kotlin.rs"]
    g102["crates/ariadne-parser/tests/facts_go.rs"]
    g103["crates/ariadne-parser/tests/facts_typescript.rs"]
    g104["crates/ariadne-parser/tests/facts_javascript.rs"]
    g105["crates/ariadne-parser/tests/facts_java.rs"]
    g106["crates/ariadne-parser/tests/common/mod.rs"]
    g107["crates/ariadne-mcp/src/bin/ariadne-mcp.rs"]
    g108["crates/ariadne-parser/benches/parse.rs"]
    g109["crates/ariadne-storage/tests/roundtrip.rs"]
    g110["crates/ariadne-storage/src/adapters/redb/scan.rs"]
    g111["tests/architecture.rs"]
    g112["crates/ariadne-parser/tests/incremental.rs"]
    g113["crates/ariadne-cli/src/commands/serve.rs"]
    g114["crates/ariadne-cli/src/commands/watch.rs"]
    g115["crates/ariadne-mcp/src/server.rs"]
    g116["crates/ariadne-storage/tests/support.rs"]
    g117["crates/ariadne-mcp/src/serve.rs"]
    g118["crates/ariadne-scip/build.rs"]
    g119["crates/ariadne-core/tests/ids.rs"]
    g120["crates/ariadne-cli/src/config.rs ⇄ crates/ariadne-cli/src/domain/mod.rs ⇄ crates/ariadne-core/src/domain/changeset.rs ⇄ crates/ariadne-core/src/domain/types/ids.rs ⇄ crates/ariadne-e2e/src/domain/mod.rs ⇄ crates/ariadne-graph/src/build.rs ⇄ crates/ariadne-graph/src/heuristics.rs ⇄ crates/ariadne-graph/tests/support.rs ⇄ crates/ariadne-parser/fixtures/java/Sample.java ⇄ crates/ariadne-parser/fixtures/javascript/jquery.js ⇄ crates/ariadne-parser/src/adapters/treesitter/cache.rs ⇄ crates/ariadne-parser/src/adapters/treesitter/facts.rs ⇄ crates/ariadne-parser/src/adapters/treesitter/incremental.rs ⇄ crates/ariadne-parser/src/adapters/treesitter/registry.rs ⇄ crates/ariadne-salsa/src/db.rs ⇄ crates/ariadne-scip/src/indexer/mod.rs ⇄ crates/ariadne-scip/src/indexer/plan.rs ⇄ crates/ariadne-scip/src/normalize/mod.rs ⇄ crates/ariadne-scip/tests/common/mod.rs ⇄ crates/ariadne-scip/tests/ingest_plan.rs ⇄ crates/ariadne-storage/src/adapters/codec.rs ⇄ crates/ariadne-storage/src/adapters/redb/apply.rs ⇄ crates/ariadne-storage/src/adapters/redb/mod.rs ⇄ crates/ariadne-storage/src/adapters/redb/snapshot.rs"]
    g121["crates/ariadne-salsa/src/errors.rs"]
    g122["crates/ariadne-salsa/src/inputs.rs"]
    g123["crates/ariadne-graph/src/lib.rs"]
    g124["crates/ariadne-graph/src/errors.rs"]
    g125["crates/ariadne-e2e/tests/smoke.rs"]
    g126["crates/ariadne-e2e/src/lib.rs"]
    g127["crates/ariadne-e2e/src/errors.rs"]
    g128["crates/ariadne-scip/tests/fixtures/sample-rust/src/lib.rs"]
    g129["crates/ariadne-scip/src/lib.rs"]
    g130["crates/ariadne-scip/src/errors.rs"]
    g131["crates/ariadne-core/src/lib.rs"]
    g132["crates/ariadne-core/src/errors.rs"]
    g133["crates/ariadne-core/src/domain/types/lang.rs"]
    g134["crates/ariadne-core/src/domain/types/span.rs"]
    g135["crates/ariadne-core/src/domain/types/mod.rs"]
    g136["crates/ariadne-core/src/domain/ports.rs"]
    g137["crates/ariadne-core/src/domain/records.rs"]
    g138["crates/ariadne-core/src/domain/mod.rs"]
    g139["crates/ariadne-core/src/domain/watcher.rs"]
    g140["crates/ariadne-parser/tests/smoke.rs"]
    g141["crates/ariadne-parser/fixtures/go/sample.go"]
    g142["crates/ariadne-parser/fixtures/python/sample.py"]
    g143["crates/ariadne-parser/fixtures/typescript/sample.ts"]
    g144["crates/ariadne-parser/fixtures/rust/sample.rs"]
    g145["crates/ariadne-parser/fixtures/kotlin/sample.kt"]
    g146["crates/ariadne-parser/src/lib.rs"]
    g147["crates/ariadne-parser/src/adapters/treesitter/mod.rs"]
    g148["crates/ariadne-parser/src/adapters/mod.rs"]
    g149["crates/ariadne-parser/src/errors.rs"]
    g150["crates/ariadne-watcher/src/lib.rs"]
    g151["crates/ariadne-watcher/src/adapters/mod.rs"]
    g152["crates/ariadne-watcher/src/errors.rs"]
    g153["crates/ariadne-storage/tests/smoke.rs"]
    g154["crates/ariadne-storage/src/lib.rs"]
    g155["crates/ariadne-storage/src/adapters/mod.rs"]
    g156["crates/ariadne-storage/src/errors.rs"]
    g157["crates/ariadne-cli/tests/smoke.rs"]
    g158["crates/ariadne-cli/src/errors.rs"]
    g159["crates/ariadne-cli/src/commands/mod.rs"]
    g160["crates/ariadne-mcp/tests/smoke.rs"]
    g161["crates/ariadne-mcp/src/types.rs"]
    g162["crates/ariadne-mcp/src/lib.rs"]
    g163["crates/ariadne-mcp/src/errors.rs"]
    g1 --> g17
    g1 --> g120
    g2 --> g17
    g2 --> g120
    g3 --> g17
    g3 --> g120
    g5 --> g6
    g5 --> g120
    g6 --> g120
    g7 --> g17
    g7 --> g120
    g7 --> g123
    g7 --> g156
    g8 --> g17
    g8 --> g92
    g8 --> g120
    g9 --> g17
    g9 --> g120
    g10 --> g14
    g10 --> g120
    g11 --> g13
    g11 --> g14
    g11 --> g65
    g11 --> g120
    g11 --> g159
    g12 --> g13
    g12 --> g120
    g12 --> g123
    g13 --> g14
    g13 --> g65
    g13 --> g120
    g13 --> g123
    g14 --> g17
    g14 --> g64
    g14 --> g120
    g14 --> g123
    g15 --> g17
    g15 --> g91
    g15 --> g120
    g16 --> g17
    g16 --> g91
    g16 --> g120
    g17 --> g120
    g18 --> g22
    g18 --> g120
    g19 --> g24
    g19 --> g92
    g19 --> g120
    g20 --> g22
    g20 --> g36
    g20 --> g120
    g21 --> g22
    g21 --> g36
    g21 --> g120
    g22 --> g37
    g22 --> g38
    g22 --> g41
    g22 --> g92
    g22 --> g120
    g22 --> g156
    g23 --> g24
    g23 --> g36
    g23 --> g91
    g23 --> g92
    g23 --> g120
    g24 --> g37
    g24 --> g41
    g24 --> g120
    g24 --> g156
    g25 --> g120
    g26 --> g120
    g27 --> g120
    g28 --> g120
    g29 --> g120
    g30 --> g120
    g31 --> g120
    g32 --> g120
    g33 --> g116
    g33 --> g120
    g33 --> g156
    g34 --> g120
    g35 --> g36
    g35 --> g37
    g35 --> g38
    g35 --> g91
    g35 --> g92
    g35 --> g120
    g36 --> g37
    g36 --> g91
    g36 --> g120
    g37 --> g120
    g37 --> g156
    g38 --> g120
    g39 --> g91
    g39 --> g120
    g39 --> g156
    g40 --> g41
    g40 --> g91
    g40 --> g120
    g41 --> g120
    g42 --> g120
    g43 --> g120
    g44 --> g120
    g45 --> g120
    g46 --> g120
    g47 --> g120
    g48 --> g120
    g49 --> g120
    g50 --> g91
    g50 --> g120
    g51 --> g120
    g52 --> g120
    g52 --> g156
    g53 --> g95
    g53 --> g120
    g54 --> g95
    g54 --> g120
    g55 --> g95
    g55 --> g120
    g56 --> g95
    g56 --> g120
    g57 --> g95
    g57 --> g120
    g58 --> g95
    g58 --> g120
    g59 --> g95
    g59 --> g120
    g60 --> g120
    g61 --> g120
    g62 --> g120
    g63 --> g65
    g63 --> g120
    g63 --> g123
    g64 --> g120
    g64 --> g122
    g64 --> g156
    g65 --> g120
    g65 --> g128
    g66 --> g120
    g66 --> g156
    g67 --> g120
    g68 --> g120
    g68 --> g123
    g69 --> g120
    g69 --> g123
    g70 --> g84
    g70 --> g120
    g71 --> g84
    g71 --> g120
    g72 --> g84
    g72 --> g120
    g73 --> g84
    g73 --> g120
    g74 --> g84
    g74 --> g120
    g75 --> g84
    g75 --> g120
    g76 --> g84
    g76 --> g120
    g77 --> g84
    g77 --> g120
    g78 --> g84
    g78 --> g120
    g79 --> g84
    g79 --> g120
    g80 --> g84
    g80 --> g120
    g81 --> g84
    g81 --> g120
    g82 --> g84
    g82 --> g120
    g83 --> g84
    g83 --> g120
    g84 --> g120
    g84 --> g156
    g84 --> g159
    g85 --> g115
    g85 --> g120
    g85 --> g123
    g86 --> g120
    g86 --> g156
    g86 --> g159
    g87 --> g120
    g88 --> g91
    g88 --> g120
    g88 --> g123
    g89 --> g91
    g89 --> g120
    g89 --> g156
    g90 --> g91
    g90 --> g120
    g91 --> g120
    g92 --> g120
    g93 --> g120
    g94 --> g120
    g95 --> g120
    g96 --> g120
    g97 --> g120
    g98 --> g106
    g99 --> g106
    g100 --> g106
    g101 --> g106
    g102 --> g106
    g103 --> g106
    g104 --> g106
    g105 --> g106
    g106 --> g120
    g106 --> g156
    g107 --> g117
    g107 --> g120
    g107 --> g123
    g107 --> g156
    g108 --> g120
    g108 --> g156
    g109 --> g120
    g110 --> g120
    g110 --> g156
    g111 --> g120
    g112 --> g120
    g112 --> g156
    g113 --> g117
    g113 --> g120
    g113 --> g123
    g114 --> g120
    g114 --> g123
    g115 --> g120
    g115 --> g123
    g116 --> g120
    g117 --> g120
    g117 --> g123
    g117 --> g159
    g118 --> g120
    g119 --> g120
    g120 --> g123
    g120 --> g128
    g120 --> g133
    g120 --> g143
    g120 --> g156
    g120 --> g159
```

## Hot-Spots

| Module | Ce | Cycles | Dead | Score |
| --- | --- | --- | --- | --- |
| `crates/ariadne-parser/fixtures/javascript/jquery.js` | 5 | 1 | 857 | 863 |
| `crates/ariadne-cli/src/domain/mod.rs` | 28 | 0 | 6 | 34 |
| `crates/ariadne-storage/src/adapters/redb/mod.rs` | 13 | 0 | 19 | 32 |
| `crates/ariadne-mcp/src/types.rs` | 0 | 0 | 26 | 26 |
| `crates/ariadne-graph/src/docgen.rs` | 23 | 0 | 1 | 24 |
| `crates/ariadne-e2e/src/domain/mod.rs` | 15 | 0 | 8 | 23 |
| `crates/ariadne-storage/tests/changeset.rs` | 18 | 0 | 4 | 22 |
| `crates/ariadne-graph/src/refactor.rs` | 18 | 0 | 3 | 21 |
| `crates/ariadne-mcp/src/server.rs` | 4 | 0 | 17 | 21 |
| `crates/ariadne-watcher/src/adapters/sink.rs` | 10 | 0 | 11 | 21 |

## Coupling

| Module | Ca | Ce | I | A | Distance |
| --- | --- | --- | --- | --- | --- |
| `crates/ariadne-cli/src/commands/index.rs` | 0 | 2 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-cli/src/commands/init.rs` | 0 | 6 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-cli/src/commands/mem.rs` | 0 | 4 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-cli/src/commands/mod.rs` | 5 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-cli/src/commands/query.rs` | 0 | 5 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-cli/src/commands/serve.rs` | 0 | 5 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-cli/src/commands/status.rs` | 0 | 6 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-cli/src/commands/watch.rs` | 0 | 4 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-cli/src/config.rs` | 22 | 10 | 0.31 | 0.00 | 0.69 |
| `crates/ariadne-cli/src/domain/mod.rs` | 4 | 28 | 0.88 | 0.00 | 0.12 |
| `crates/ariadne-cli/src/errors.rs` | 0 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-cli/src/main.rs` | 0 | 4 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-cli/tests/smoke.rs` | 0 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-core/benches/ids.rs` | 0 | 4 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-core/src/domain/changeset.rs` | 10 | 2 | 0.17 | 0.00 | 0.83 |
| `crates/ariadne-core/src/domain/mod.rs` | 0 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-core/src/domain/ports.rs` | 0 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-core/src/domain/records.rs` | 0 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-core/src/domain/types/ids.rs` | 81 | 1 | 0.01 | 0.00 | 0.99 |
| `crates/ariadne-core/src/domain/types/lang.rs` | 4 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-core/src/domain/types/mod.rs` | 0 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-core/src/domain/types/span.rs` | 0 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-core/src/domain/watcher.rs` | 0 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-core/src/errors.rs` | 0 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-core/src/lib.rs` | 0 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-core/tests/ids.rs` | 0 | 1 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-e2e/src/domain/mod.rs` | 33 | 15 | 0.31 | 0.00 | 0.69 |
| `crates/ariadne-e2e/src/errors.rs` | 0 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-e2e/src/lib.rs` | 0 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-e2e/tests/mcp_session.rs` | 0 | 11 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-e2e/tests/repos/csharp.rs` | 0 | 2 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-e2e/tests/repos/go.rs` | 0 | 2 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-e2e/tests/repos/java.rs` | 0 | 2 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-e2e/tests/repos/python.rs` | 0 | 2 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-e2e/tests/repos/rust.rs` | 0 | 2 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-e2e/tests/repos/typescript.rs` | 0 | 2 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-e2e/tests/slo.rs` | 0 | 19 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-e2e/tests/smoke.rs` | 0 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-graph/benches/blast.rs` | 0 | 9 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-graph/src/blast.rs` | 3 | 10 | 0.77 | 0.00 | 0.23 |
| `crates/ariadne-graph/src/build.rs` | 19 | 11 | 0.37 | 0.00 | 0.63 |
| `crates/ariadne-graph/src/coupling.rs` | 6 | 9 | 0.60 | 0.00 | 0.40 |
| `crates/ariadne-graph/src/cycles.rs` | 6 | 3 | 0.33 | 0.00 | 0.67 |
| `crates/ariadne-graph/src/dead.rs` | 3 | 5 | 0.62 | 0.00 | 0.38 |
| `crates/ariadne-graph/src/docgen.rs` | 5 | 23 | 0.82 | 0.00 | 0.18 |
| `crates/ariadne-graph/src/errors.rs` | 0 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-graph/src/heuristics.rs` | 46 | 9 | 0.16 | 0.00 | 0.84 |
| `crates/ariadne-graph/src/lib.rs` | 24 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-graph/src/plan_assist.rs` | 0 | 13 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-graph/src/refactor.rs` | 4 | 18 | 0.82 | 0.00 | 0.18 |
| `crates/ariadne-graph/tests/docgen_fixture.rs` | 0 | 6 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-graph/tests/golden_repo.rs` | 0 | 9 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-graph/tests/refactor_cases.rs` | 0 | 9 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-graph/tests/support.rs` | 17 | 10 | 0.37 | 0.00 | 0.63 |
| `crates/ariadne-graph/tests/synthetic.rs` | 0 | 5 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-mcp/benches/cold_start.rs` | 0 | 12 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-mcp/benches/concurrent.rs` | 0 | 15 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-mcp/src/bin/ariadne-mcp.rs` | 0 | 9 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-mcp/src/catalog.rs` | 11 | 12 | 0.52 | 0.00 | 0.48 |
| `crates/ariadne-mcp/src/errors.rs` | 0 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-mcp/src/lib.rs` | 0 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-mcp/src/serve.rs` | 2 | 3 | 0.60 | 0.00 | 0.40 |
| `crates/ariadne-mcp/src/server.rs` | 1 | 4 | 0.80 | 0.00 | 0.20 |
| `crates/ariadne-mcp/src/tools/blast_radius.rs` | 0 | 6 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-mcp/src/tools/coupling_report.rs` | 4 | 8 | 0.67 | 0.00 | 0.33 |
| `crates/ariadne-mcp/src/tools/doc_for.rs` | 0 | 7 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-mcp/src/tools/doc_module.rs` | 0 | 4 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-mcp/src/tools/doc_project.rs` | 0 | 3 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-mcp/src/tools/file_summary.rs` | 0 | 13 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-mcp/src/tools/find_definition.rs` | 0 | 2 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-mcp/src/tools/find_references.rs` | 0 | 10 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-mcp/src/tools/list_symbols.rs` | 0 | 4 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-mcp/src/tools/mod.rs` | 0 | 6 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-mcp/src/tools/plan_assist.rs` | 0 | 11 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-mcp/src/tools/project_status.rs` | 0 | 3 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-mcp/src/tools/refactor.rs` | 0 | 11 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-mcp/src/tools/weak_spots.rs` | 0 | 10 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-mcp/src/types.rs` | 0 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-mcp/tests/handshake.rs` | 0 | 4 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-mcp/tests/shutdown.rs` | 0 | 5 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-mcp/tests/smoke.rs` | 0 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-mcp/tests/support.rs` | 16 | 14 | 0.47 | 0.00 | 0.53 |
| `crates/ariadne-mcp/tests/tools_blast_radius.rs` | 0 | 6 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-mcp/tests/tools_coupling_report.rs` | 0 | 7 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-mcp/tests/tools_doc.rs` | 0 | 6 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-mcp/tests/tools_doc_for.rs` | 0 | 6 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-mcp/tests/tools_file_summary.rs` | 0 | 6 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-mcp/tests/tools_find_definition.rs` | 0 | 5 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-mcp/tests/tools_find_references.rs` | 0 | 6 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-mcp/tests/tools_list_symbols.rs` | 0 | 6 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-mcp/tests/tools_plan_assist.rs` | 0 | 6 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-mcp/tests/tools_project_status.rs` | 0 | 5 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-mcp/tests/tools_refactor.rs` | 0 | 6 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-mcp/tests/tools_weak_spots.rs` | 0 | 6 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-parser/benches/parse.rs` | 0 | 8 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-parser/fixtures/csharp/Sample.cs` | 0 | 1 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-parser/fixtures/go/sample.go` | 0 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-parser/fixtures/java/Sample.java` | 6 | 2 | 0.25 | 0.00 | 0.75 |
| `crates/ariadne-parser/fixtures/javascript/jquery.js` | 250 | 5 | 0.02 | 0.00 | 0.98 |
| `crates/ariadne-parser/fixtures/javascript/sample.js` | 0 | 1 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-parser/fixtures/kotlin/sample.kt` | 0 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-parser/fixtures/python/sample.py` | 0 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-parser/fixtures/rust/sample.rs` | 0 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-parser/fixtures/typescript/sample.ts` | 2 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-parser/src/adapters/mod.rs` | 0 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-parser/src/adapters/treesitter/cache.rs` | 13 | 5 | 0.28 | 0.00 | 0.72 |
| `crates/ariadne-parser/src/adapters/treesitter/facts.rs` | 3 | 6 | 0.67 | 0.00 | 0.33 |
| `crates/ariadne-parser/src/adapters/treesitter/incremental.rs` | 6 | 4 | 0.40 | 0.00 | 0.60 |
| `crates/ariadne-parser/src/adapters/treesitter/mod.rs` | 0 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-parser/src/adapters/treesitter/registry.rs` | 2 | 4 | 0.67 | 0.00 | 0.33 |
| `crates/ariadne-parser/src/errors.rs` | 0 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-parser/src/lib.rs` | 0 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-parser/tests/common/mod.rs` | 8 | 5 | 0.38 | 0.00 | 0.62 |
| `crates/ariadne-parser/tests/facts_csharp.rs` | 0 | 1 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-parser/tests/facts_go.rs` | 0 | 1 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-parser/tests/facts_java.rs` | 0 | 1 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-parser/tests/facts_javascript.rs` | 0 | 1 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-parser/tests/facts_kotlin.rs` | 0 | 1 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-parser/tests/facts_python.rs` | 0 | 1 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-parser/tests/facts_rust.rs` | 0 | 1 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-parser/tests/facts_typescript.rs` | 0 | 1 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-parser/tests/incremental.rs` | 0 | 2 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-parser/tests/real_world.rs` | 0 | 5 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-parser/tests/smoke.rs` | 0 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-salsa/benches/edit.rs` | 0 | 6 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-salsa/src/db.rs` | 154 | 6 | 0.04 | 0.00 | 0.96 |
| `crates/ariadne-salsa/src/derived.rs` | 11 | 9 | 0.45 | 0.00 | 0.55 |
| `crates/ariadne-salsa/src/errors.rs` | 0 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-salsa/src/inputs.rs` | 2 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-salsa/src/lib.rs` | 0 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-salsa/src/memory.rs` | 1 | 4 | 0.80 | 0.00 | 0.20 |
| `crates/ariadne-salsa/tests/durability.rs` | 0 | 4 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-salsa/tests/equivalence.rs` | 0 | 7 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-salsa/tests/smoke.rs` | 0 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-scip/build.rs` | 0 | 1 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-scip/src/errors.rs` | 0 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-scip/src/indexer/lsif_go.rs` | 0 | 4 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-scip/src/indexer/mod.rs` | 1 | 2 | 0.67 | 0.00 | 0.33 |
| `crates/ariadne-scip/src/indexer/plan.rs` | 5 | 15 | 0.75 | 0.00 | 0.25 |
| `crates/ariadne-scip/src/indexer/rust_analyzer.rs` | 0 | 4 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-scip/src/indexer/scip_clang.rs` | 0 | 4 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-scip/src/indexer/scip_dotnet.rs` | 0 | 5 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-scip/src/indexer/scip_java.rs` | 0 | 4 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-scip/src/indexer/scip_python.rs` | 0 | 4 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-scip/src/indexer/scip_typescript.rs` | 0 | 4 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-scip/src/indexer/subprocess.rs` | 7 | 6 | 0.46 | 0.00 | 0.54 |
| `crates/ariadne-scip/src/lib.rs` | 0 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-scip/src/normalize/grammar.rs` | 0 | 3 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-scip/src/normalize/mod.rs` | 5 | 3 | 0.38 | 0.00 | 0.62 |
| `crates/ariadne-scip/tests/common/mod.rs` | 14 | 12 | 0.46 | 0.00 | 0.54 |
| `crates/ariadne-scip/tests/fixtures/sample-rust/src/lib.rs` | 2 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-scip/tests/ingest_clang.rs` | 0 | 2 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-scip/tests/ingest_csharp.rs` | 0 | 2 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-scip/tests/ingest_go.rs` | 0 | 2 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-scip/tests/ingest_java.rs` | 0 | 2 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-scip/tests/ingest_plan.rs` | 11 | 9 | 0.45 | 0.00 | 0.55 |
| `crates/ariadne-scip/tests/ingest_python.rs` | 0 | 2 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-scip/tests/ingest_rust.rs` | 0 | 2 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-scip/tests/ingest_typescript.rs` | 0 | 2 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-scip/tests/normalize.rs` | 0 | 4 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-scip/tests/roundtrip.rs` | 0 | 4 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-storage/benches/apply.rs` | 0 | 10 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-storage/src/adapters/codec.rs` | 9 | 2 | 0.18 | 0.00 | 0.82 |
| `crates/ariadne-storage/src/adapters/mod.rs` | 0 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-storage/src/adapters/redb/apply.rs` | 1 | 15 | 0.94 | 0.00 | 0.06 |
| `crates/ariadne-storage/src/adapters/redb/mod.rs` | 35 | 13 | 0.27 | 0.00 | 0.73 |
| `crates/ariadne-storage/src/adapters/redb/scan.rs` | 0 | 11 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-storage/src/adapters/redb/snapshot.rs` | 4 | 11 | 0.73 | 0.00 | 0.27 |
| `crates/ariadne-storage/src/errors.rs` | 26 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-storage/src/lib.rs` | 0 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-storage/tests/changeset.rs` | 0 | 18 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-storage/tests/mvcc.rs` | 0 | 13 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-storage/tests/roundtrip.rs` | 0 | 1 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-storage/tests/smoke.rs` | 0 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-storage/tests/support.rs` | 1 | 3 | 0.75 | 0.00 | 0.25 |
| `crates/ariadne-watcher/benches/sink.rs` | 0 | 6 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-watcher/src/adapters/ignore.rs` | 5 | 6 | 0.55 | 0.00 | 0.45 |
| `crates/ariadne-watcher/src/adapters/mod.rs` | 0 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-watcher/src/adapters/notify.rs` | 0 | 10 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-watcher/src/adapters/reconcile.rs` | 4 | 11 | 0.73 | 0.00 | 0.27 |
| `crates/ariadne-watcher/src/adapters/sink.rs` | 4 | 10 | 0.71 | 0.00 | 0.29 |
| `crates/ariadne-watcher/src/errors.rs` | 0 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-watcher/src/lib.rs` | 0 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-watcher/tests/events.rs` | 3 | 14 | 0.82 | 0.00 | 0.18 |
| `crates/ariadne-watcher/tests/ignore.rs` | 0 | 4 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-watcher/tests/reconcile.rs` | 0 | 6 | 1.00 | 0.00 | 0.00 |
| `tests/architecture.rs` | 0 | 4 | 1.00 | 0.00 | 0.00 |

## Glossary

- `new` (function) — `crates/ariadne-salsa/src/db.rs`
- `map` (variable) — `crates/ariadne-parser/fixtures/javascript/jquery.js`
- `push` (variable) — `crates/ariadne-parser/fixtures/javascript/jquery.js`
- `get` (function) — `crates/ariadne-core/src/domain/types/ids.rs`
- `clone` (variable) — `crates/ariadne-parser/fixtures/javascript/jquery.js`
- `len` (variable) — `crates/ariadne-parser/fixtures/javascript/jquery.js`
- `path` (function) — `crates/ariadne-graph/src/heuristics.rs`
- `insert` (variable) — `crates/ariadne-parser/fixtures/javascript/jquery.js`
- `from` (function) — `crates/ariadne-storage/src/errors.rs`
- `build` (module) — `crates/ariadne-graph/src/lib.rs`

