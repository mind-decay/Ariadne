# Project Architecture Overview

## Overview

351 modules · 3331 symbols · 4597 edges · 2 dependency cycle(s).

## Layers

```mermaid
flowchart TD
    g0["crates/ariadne-cli/fixtures/astro/App.astro"]
    g1["crates/ariadne-cli/fixtures/svelte/App.svelte"]
    g2["crates/ariadne-cli/fixtures/vue/App.vue"]
    g3["crates/ariadne-parser/fixtures/svelte/sample.svelte"]
    g4["crates/ariadne-parser/fixtures/vue/sample.vue"]
    g5["crates/ariadne-cli/fixtures/astro/Child.astro"]
    g6["crates/ariadne-cli/src/commands/serve.rs"]
    g7["crates/ariadne-watcher/benches/sink.rs"]
    g8["crates/ariadne-watcher/src/adapters/notify.rs"]
    g9["crates/ariadne-watcher/tests/reconcile.rs"]
    g10["crates/ariadne-watcher/src/adapters/reconcile.rs"]
    g11["crates/ariadne-cli/src/commands/watch.rs"]
    g12["crates/ariadne-cli/src/main.rs"]
    g13["crates/ariadne-cli/tests/index_frameworks.rs"]
    g14["crates/ariadne-mcp/tests/tools_diff_blast.rs"]
    g15["crates/ariadne-daemon/tests/live_update.rs"]
    g16["crates/ariadne-daemon/tests/warm_analytics.rs"]
    g17["crates/ariadne-daemon/tests/warm_graph.rs"]
    g18["crates/ariadne-cli/src/commands/status.rs"]
    g19["crates/ariadne-cli/src/commands/init.rs"]
    g20["crates/ariadne-cli/src/commands/setup.rs"]
    g21["crates/ariadne-cli/tests/index_parity.rs"]
    g22["crates/ariadne-cli/src/commands/mem.rs"]
    g23["crates/ariadne-core/benches/ids.rs"]
    g24["crates/ariadne-core/tests/ids.rs"]
    g25["crates/ariadne-core/tests/tags.rs"]
    g26["crates/ariadne-daemon/src/domain/facts.rs"]
    g27["crates/ariadne-graph/tests/docgen_fixture.rs"]
    g28["crates/ariadne-mcp/src/tools/doc_module.rs"]
    g29["crates/ariadne-mcp/src/tools/doc_project.rs"]
    g30["crates/ariadne-graph/tests/refactor_cases.rs"]
    g31["crates/ariadne-mcp/src/tools/refactor.rs"]
    g32["crates/ariadne-mcp/tests/catalog_projection.rs"]
    g33["crates/ariadne-mcp/tests/daemon_client.rs"]
    g34["crates/ariadne-mcp/tests/handshake.rs"]
    g35["crates/ariadne-mcp/tests/lazy_catalog.rs"]
    g36["crates/ariadne-mcp/tests/shutdown.rs"]
    g37["crates/ariadne-mcp/tests/tools_blast_radius.rs"]
    g38["crates/ariadne-mcp/tests/tools_co_change.rs"]
    g39["crates/ariadne-mcp/tests/tools_complexity.rs"]
    g40["crates/ariadne-mcp/tests/tools_component_graph.rs"]
    g41["crates/ariadne-mcp/tests/tools_coupling_report.rs"]
    g42["crates/ariadne-mcp/tests/tools_doc.rs"]
    g43["crates/ariadne-mcp/tests/tools_doc_for.rs"]
    g44["crates/ariadne-mcp/tests/tools_file_summary.rs"]
    g45["crates/ariadne-mcp/tests/tools_find_definition.rs"]
    g46["crates/ariadne-mcp/tests/tools_find_references.rs"]
    g47["crates/ariadne-mcp/tests/tools_hotspots.rs"]
    g48["crates/ariadne-mcp/tests/tools_list_symbols.rs"]
    g49["crates/ariadne-mcp/tests/tools_plan_assist.rs"]
    g50["crates/ariadne-mcp/tests/tools_project_status.rs"]
    g51["crates/ariadne-mcp/tests/tools_refactor.rs"]
    g52["crates/ariadne-mcp/tests/tools_weak_spots.rs"]
    g53["crates/ariadne-mcp/tests/support.rs"]
    g54["crates/ariadne-parser/benches/parse.rs"]
    g55["crates/ariadne-parser/tests/facts_astro.rs"]
    g56["crates/ariadne-parser/tests/facts_c.rs"]
    g57["crates/ariadne-parser/tests/facts_cpp.rs"]
    g58["crates/ariadne-parser/tests/facts_csharp.rs"]
    g59["crates/ariadne-parser/tests/facts_go.rs"]
    g60["crates/ariadne-parser/tests/facts_java.rs"]
    g61["crates/ariadne-parser/tests/facts_javascript.rs"]
    g62["crates/ariadne-parser/tests/facts_jsx.rs"]
    g63["crates/ariadne-parser/tests/facts_kotlin.rs"]
    g64["crates/ariadne-parser/tests/facts_python.rs"]
    g65["crates/ariadne-parser/tests/facts_rust.rs"]
    g66["crates/ariadne-parser/tests/facts_svelte.rs"]
    g67["crates/ariadne-parser/tests/facts_tsx.rs"]
    g68["crates/ariadne-parser/tests/facts_typescript.rs"]
    g69["crates/ariadne-parser/tests/facts_vue.rs"]
    g70["crates/ariadne-parser/tests/common/mod.rs"]
    g71["crates/ariadne-parser/tests/complexity.rs"]
    g72["crates/ariadne-parser/tests/real_world.rs"]
    g73["crates/ariadne-salsa/benches/edit.rs"]
    g74["crates/ariadne-salsa/tests/derivation.rs"]
    g75["crates/ariadne-salsa/tests/durability.rs"]
    g76["crates/ariadne-salsa/tests/equivalence.rs"]
    g77["crates/ariadne-salsa/tests/incremental.rs"]
    g78["crates/ariadne-watcher/tests/events.rs"]
    g79["crates/ariadne-scip/tests/ingest_go.rs"]
    g80["crates/ariadne-scip/src/indexer/rust_analyzer.rs"]
    g81["crates/ariadne-scip/src/indexer/scip_astro.rs"]
    g82["crates/ariadne-scip/src/indexer/scip_clang.rs"]
    g83["crates/ariadne-scip/src/indexer/scip_dotnet.rs"]
    g84["crates/ariadne-scip/src/indexer/scip_go.rs"]
    g85["crates/ariadne-scip/src/indexer/scip_java.rs"]
    g86["crates/ariadne-scip/src/indexer/scip_python.rs"]
    g87["crates/ariadne-scip/src/indexer/scip_svelte.rs"]
    g88["crates/ariadne-scip/src/indexer/scip_typescript.rs"]
    g89["crates/ariadne-scip/src/indexer/scip_vue.rs"]
    g90["crates/ariadne-scip/src/indexer/subprocess.rs"]
    g91["crates/ariadne-mcp/benches/concurrent.rs"]
    g92["crates/ariadne-mcp/src/catalog.rs"]
    g93["crates/ariadne-mcp/src/bin/ariadne-mcp.rs"]
    g94["crates/ariadne-mcp/src/serve.rs"]
    g95["crates/ariadne-storage/tests/changeset.rs"]
    g96["crates/ariadne-storage/tests/mvcc.rs"]
    g97["crates/ariadne-mcp/benches/cold_start.rs"]
    g98["crates/ariadne-scip/tests/ingest_plan.rs"]
    g99["crates/ariadne-parser/fixtures/csharp/Sample.cs"]
    g100["crates/ariadne-storage/benches/apply.rs"]
    g101["crates/ariadne-storage/tests/history_merge.rs"]
    g102["crates/ariadne-storage/tests/history.rs"]
    g103["crates/ariadne-storage/tests/migration.rs"]
    g104["crates/ariadne-storage/tests/symbol_churn.rs"]
    g105["crates/ariadne-scip/tests/ingest_clang.rs"]
    g106["crates/ariadne-scip/tests/ingest_csharp.rs"]
    g107["crates/ariadne-scip/tests/ingest_java.rs"]
    g108["crates/ariadne-scip/tests/ingest_python.rs"]
    g109["crates/ariadne-scip/tests/ingest_rust.rs"]
    g110["crates/ariadne-scip/tests/ingest_typescript.rs"]
    g111["crates/ariadne-scip/tests/common/mod.rs"]
    g112["crates/ariadne-scip/tests/roundtrip.rs"]
    g113["crates/ariadne-storage/src/adapters/redb/scan.rs"]
    g114["crates/ariadne-storage/tests/roundtrip.rs"]
    g115["crates/ariadne-graph/benches/blast.rs"]
    g116["crates/ariadne-graph/tests/component_graph.rs"]
    g117["crates/ariadne-graph/tests/dead_code_roots.rs"]
    g118["crates/ariadne-graph/tests/diff_blast.rs"]
    g119["crates/ariadne-graph/tests/golden_repo.rs"]
    g120["crates/ariadne-graph/tests/support.rs"]
    g121["crates/ariadne-graph/tests/synthetic.rs"]
    g122["crates/ariadne-mcp/src/tools/project_status.rs"]
    g123["crates/ariadne-mcp/src/tools/file_summary.rs"]
    g124["crates/ariadne-mcp/src/tools/find_references.rs"]
    g125["crates/ariadne-mcp/src/tools/diff_blast.rs"]
    g126["crates/ariadne-e2e/tests/cli_daemon_parity.rs"]
    g127["crates/ariadne-e2e/tests/mcp_session.rs"]
    g128["crates/ariadne-e2e/tests/slo.rs"]
    g129["crates/ariadne-mcp/src/tools/co_change.rs"]
    g130["crates/ariadne-mcp/src/tools/weak_spots.rs"]
    g131["crates/ariadne-graph/tests/hotspot.rs"]
    g132["crates/ariadne-mcp/src/tools/hotspots.rs"]
    g133["crates/ariadne-graph/src/diff_blast.rs"]
    g134["crates/ariadne-graph/tests/symbol_churn.rs"]
    g135["crates/ariadne-mcp/src/tools/mod.rs"]
    g136["crates/ariadne-git/tests/diff.rs"]
    g137["crates/ariadne-scip/tests/ingest_astro.rs"]
    g138["crates/ariadne-git/tests/line_hunks.rs"]
    g139["crates/ariadne-scip/src/normalize/grammar.rs"]
    g140["crates/ariadne-scip/tests/ingest_react.rs"]
    g141["crates/ariadne-scip/tests/ingest_svelte.rs"]
    g142["crates/ariadne-scip/tests/ingest_vue.rs"]
    g143["crates/ariadne-watcher/tests/file_id_cache.rs"]
    g144["crates/ariadne-watcher/src/adapters/file_id_cache.rs"]
    g145["crates/ariadne-watcher/src/adapters/sink.rs"]
    g146["tests/architecture.rs"]
    g147["crates/ariadne-daemon/tests/daemon.rs"]
    g148["crates/ariadne-git/tests/history.rs"]
    g149["crates/ariadne-git/tests/incremental.rs"]
    g150["crates/ariadne-mcp/src/tools/complexity.rs"]
    g151["crates/ariadne-mcp/src/tools/coupling_report.rs"]
    g152["crates/ariadne-mcp/src/tools/doc_for.rs"]
    g153["crates/ariadne-mcp/src/tools/list_symbols.rs"]
    g154["crates/ariadne-mcp/src/tools/plan_assist.rs"]
    g155["crates/ariadne-parser/tests/incremental.rs"]
    g156["crates/ariadne-parser/tests/incremental_svelte.rs"]
    g157["crates/ariadne-parser/tests/incremental_vue.rs"]
    g158["crates/ariadne-scip/src/normalize/mod.rs"]
    g159["crates/ariadne-scip/tests/normalize.rs"]
    g160["crates/ariadne-watcher/tests/ignore.rs"]
    g161["crates/ariadne-watcher/src/adapters/ignore.rs"]
    g162["crates/ariadne-scip/build.rs"]
    g163["crates/ariadne-storage/tests/support.rs"]
    g164["crates/ariadne-e2e/tests/repos/astro.rs"]
    g165["crates/ariadne-e2e/tests/repos/c.rs"]
    g166["crates/ariadne-e2e/tests/repos/cpp.rs"]
    g167["crates/ariadne-e2e/tests/repos/csharp.rs"]
    g168["crates/ariadne-e2e/tests/repos/go.rs"]
    g169["crates/ariadne-e2e/tests/repos/java.rs"]
    g170["crates/ariadne-e2e/tests/repos/python.rs"]
    g171["crates/ariadne-e2e/tests/repos/react.rs"]
    g172["crates/ariadne-e2e/tests/repos/rust.rs"]
    g173["crates/ariadne-e2e/tests/repos/svelte.rs"]
    g174["crates/ariadne-e2e/tests/repos/typescript.rs"]
    g175["crates/ariadne-e2e/tests/repos/vue.rs"]
    g176["crates/ariadne-mcp/src/tools/find_definition.rs"]
    g177["crates/ariadne-cli/src/adapters/daemon_client.rs ⇄ crates/ariadne-cli/src/commands/daemon.rs ⇄ crates/ariadne-cli/src/commands/index.rs ⇄ crates/ariadne-cli/src/commands/query.rs ⇄ crates/ariadne-cli/src/config.rs ⇄ crates/ariadne-cli/src/domain/mod.rs ⇄ crates/ariadne-cli/tests/incremental_history.rs ⇄ crates/ariadne-cli/tests/setup.rs ⇄ crates/ariadne-core/src/domain/changeset.rs ⇄ crates/ariadne-core/src/domain/records.rs ⇄ crates/ariadne-core/src/domain/types/ids.rs ⇄ crates/ariadne-daemon/benches/warm_query.rs ⇄ crates/ariadne-daemon/src/adapters/codec.rs ⇄ crates/ariadne-daemon/src/adapters/ipc.rs ⇄ crates/ariadne-daemon/src/domain/catalog.rs ⇄ crates/ariadne-daemon/src/domain/dispatch.rs ⇄ crates/ariadne-daemon/src/domain/dump.rs ⇄ crates/ariadne-daemon/src/domain/index_lock.rs ⇄ crates/ariadne-daemon/src/domain/lifecycle.rs ⇄ crates/ariadne-daemon/src/domain/live.rs ⇄ crates/ariadne-daemon/src/domain/queries/analytics.rs ⇄ crates/ariadne-daemon/src/domain/queries/docs.rs ⇄ crates/ariadne-daemon/src/domain/queries/health.rs ⇄ crates/ariadne-daemon/src/domain/queries/impact.rs ⇄ crates/ariadne-daemon/src/domain/queries/meta.rs ⇄ crates/ariadne-daemon/src/domain/queries/navigate.rs ⇄ crates/ariadne-daemon/src/domain/queries/refactor.rs ⇄ crates/ariadne-daemon/src/domain/snapshot.rs ⇄ crates/ariadne-daemon/tests/memory_probe.rs ⇄ crates/ariadne-daemon/tests/support.rs ⇄ crates/ariadne-e2e/src/domain/mod.rs ⇄ crates/ariadne-git/src/adapters/gix/diff.rs ⇄ crates/ariadne-git/src/adapters/gix/incremental.rs ⇄ crates/ariadne-git/src/adapters/gix/line_hunks.rs ⇄ crates/ariadne-git/src/adapters/gix/mod.rs ⇄ crates/ariadne-graph/src/blast.rs ⇄ crates/ariadne-graph/src/build.rs ⇄ crates/ariadne-graph/src/co_change.rs ⇄ crates/ariadne-graph/src/coupling.rs ⇄ crates/ariadne-graph/src/cycles.rs ⇄ crates/ariadne-graph/src/dead.rs ⇄ crates/ariadne-graph/src/docgen.rs ⇄ crates/ariadne-graph/src/heuristics.rs ⇄ crates/ariadne-graph/src/hotspot.rs ⇄ crates/ariadne-graph/src/plan_assist.rs ⇄ crates/ariadne-graph/src/refactor.rs ⇄ crates/ariadne-graph/src/roots.rs ⇄ crates/ariadne-graph/src/span_lines.rs ⇄ crates/ariadne-graph/src/symbol_churn.rs ⇄ crates/ariadne-graph/tests/co_change.rs ⇄ crates/ariadne-mcp/src/adapters/daemon_client.rs ⇄ crates/ariadne-mcp/src/server.rs ⇄ crates/ariadne-mcp/src/tools/blast_radius.rs ⇄ crates/ariadne-parser/fixtures/java/Sample.java ⇄ crates/ariadne-parser/fixtures/javascript/jquery.js ⇄ crates/ariadne-parser/fixtures/javascript/sample.js ⇄ crates/ariadne-parser/src/adapters/treesitter/cache.rs ⇄ crates/ariadne-parser/src/adapters/treesitter/complexity.rs ⇄ crates/ariadne-parser/src/adapters/treesitter/facts.rs ⇄ crates/ariadne-parser/src/adapters/treesitter/incremental.rs ⇄ crates/ariadne-parser/src/adapters/treesitter/injection.rs ⇄ crates/ariadne-parser/src/adapters/treesitter/registry.rs ⇄ crates/ariadne-salsa/src/db.rs ⇄ crates/ariadne-salsa/src/derive.rs ⇄ crates/ariadne-salsa/src/derived.rs ⇄ crates/ariadne-salsa/src/memory.rs ⇄ crates/ariadne-scip/src/indexer/mod.rs ⇄ crates/ariadne-scip/src/indexer/plan.rs ⇄ crates/ariadne-storage/src/adapters/codec.rs ⇄ crates/ariadne-storage/src/adapters/redb/apply.rs ⇄ crates/ariadne-storage/src/adapters/redb/history.rs ⇄ crates/ariadne-storage/src/adapters/redb/mod.rs ⇄ crates/ariadne-storage/src/adapters/redb/snapshot.rs ⇄ crates/ariadne-storage/src/domain/migration.rs ⇄ tools/ariadne-sfc-scip/src/index.ts ⇄ tools/ariadne-sfc-scip/src/scip.ts"]
    g178["crates/ariadne-cli/fixtures/c/caller.c"]
    g179["crates/ariadne-cli/fixtures/java/Caller.java"]
    g180["crates/ariadne-cli/fixtures/python/caller.py"]
    g181["crates/ariadne-cli/fixtures/rust/caller.rs"]
    g182["crates/ariadne-cli/fixtures/typescript/caller.ts"]
    g183["crates/ariadne-cli/fixtures/c/callee.c"]
    g184["crates/ariadne-cli/fixtures/csharp/Caller.cs"]
    g185["crates/ariadne-cli/fixtures/go/caller.go"]
    g186["crates/ariadne-cli/fixtures/csharp/Callee.cs"]
    g187["crates/ariadne-cli/fixtures/go/callee.go"]
    g188["crates/ariadne-cli/fixtures/java/Callee.java"]
    g189["crates/ariadne-cli/fixtures/python/callee.py"]
    g190["crates/ariadne-cli/fixtures/react/App.tsx"]
    g191["crates/ariadne-cli/fixtures/rust/callee.rs"]
    g192["crates/ariadne-cli/fixtures/svelte/Child.svelte"]
    g193["crates/ariadne-cli/fixtures/typescript/callee.ts"]
    g194["crates/ariadne-cli/fixtures/vue/Child.vue"]
    g195["crates/ariadne-cli/src/adapters/mod.rs"]
    g196["crates/ariadne-cli/src/commands/mod.rs"]
    g197["crates/ariadne-cli/src/errors.rs"]
    g198["crates/ariadne-cli/tests/smoke.rs"]
    g199["crates/ariadne-core/src/domain/daemon/mod.rs"]
    g200["crates/ariadne-core/src/domain/daemon/query.rs"]
    g201["crates/ariadne-core/src/domain/daemon/response.rs"]
    g202["crates/ariadne-core/src/domain/daemon/rows.rs"]
    g203["crates/ariadne-core/src/domain/mod.rs"]
    g204["crates/ariadne-daemon/src/errors.rs"]
    g205["crates/ariadne-core/src/domain/ports.rs"]
    g206["crates/ariadne-core/src/domain/types/lang.rs"]
    g207["crates/ariadne-core/src/domain/types/mod.rs"]
    g208["crates/ariadne-core/src/domain/types/span.rs"]
    g209["crates/ariadne-core/src/domain/types/visibility.rs"]
    g210["crates/ariadne-core/src/domain/watcher.rs"]
    g211["crates/ariadne-core/src/errors.rs"]
    g212["crates/ariadne-core/src/lib.rs"]
    g213["crates/ariadne-daemon/src/adapters/mod.rs"]
    g214["crates/ariadne-daemon/src/domain/mod.rs"]
    g215["crates/ariadne-daemon/src/domain/queries/mod.rs"]
    g216["crates/ariadne-daemon/src/lib.rs"]
    g217["crates/ariadne-daemon/tests/incremental_warm.rs"]
    g218["crates/ariadne-e2e/src/errors.rs"]
    g219["crates/ariadne-e2e/src/lib.rs"]
    g220["crates/ariadne-e2e/tests/smoke.rs"]
    g221["crates/ariadne-git/src/adapters/mod.rs"]
    g222["crates/ariadne-git/src/errors.rs"]
    g223["crates/ariadne-git/src/lib.rs"]
    g224["crates/ariadne-graph/src/errors.rs"]
    g225["crates/ariadne-graph/src/lib.rs"]
    g226["crates/ariadne-mcp/src/adapters/mod.rs"]
    g227["crates/ariadne-mcp/src/errors.rs"]
    g228["crates/ariadne-mcp/src/lib.rs"]
    g229["crates/ariadne-mcp/src/types.rs"]
    g230["crates/ariadne-mcp/tests/smoke.rs"]
    g231["crates/ariadne-parser/fixtures/astro/sample.astro"]
    g232["crates/ariadne-parser/fixtures/c/sample.c"]
    g233["crates/ariadne-parser/fixtures/cpp/sample.cpp"]
    g234["crates/ariadne-parser/fixtures/go/sample.go"]
    g235["crates/ariadne-parser/fixtures/kotlin/sample.kt"]
    g236["crates/ariadne-parser/fixtures/python/sample.py"]
    g237["crates/ariadne-parser/fixtures/react/sample.jsx"]
    g238["crates/ariadne-parser/fixtures/vue/two-scripts.vue"]
    g239["crates/ariadne-parser/fixtures/react/sample.tsx"]
    g240["crates/ariadne-parser/fixtures/rust/sample.rs"]
    g241["crates/ariadne-parser/fixtures/solid/sample.tsx"]
    g242["crates/ariadne-parser/fixtures/typescript/sample.ts"]
    g243["crates/ariadne-parser/fixtures/vue/script-tsx.vue"]
    g244["crates/ariadne-parser/src/adapters/mod.rs"]
    g245["crates/ariadne-parser/src/adapters/treesitter/mod.rs"]
    g246["crates/ariadne-parser/src/errors.rs"]
    g247["crates/ariadne-parser/src/lib.rs"]
    g248["crates/ariadne-parser/tests/smoke.rs"]
    g249["crates/ariadne-salsa/src/errors.rs"]
    g250["crates/ariadne-salsa/src/inputs.rs"]
    g251["crates/ariadne-salsa/src/lib.rs"]
    g252["crates/ariadne-salsa/tests/smoke.rs"]
    g253["crates/ariadne-scip/fixtures/astro/src/Page.astro"]
    g254["crates/ariadne-scip/fixtures/astro/src/util.ts"]
    g255["crates/ariadne-scip/fixtures/go/demo.go"]
    g256["crates/ariadne-scip/src/errors.rs"]
    g257["crates/ariadne-scip/src/lib.rs"]
    g258["crates/ariadne-scip/tests/fixtures/sample-react/src/App.tsx"]
    g259["crates/ariadne-scip/tests/fixtures/sample-svelte/src/App.svelte"]
    g260["crates/ariadne-scip/tests/fixtures/sample-vue/src/App.vue"]
    g261["crates/ariadne-scip/tests/fixtures/sample-svelte/src/Card.svelte"]
    g262["crates/ariadne-scip/tests/fixtures/sample-vue/src/Card.vue"]
    g263["crates/ariadne-scip/tests/fixtures/sample-react/src/Button.tsx"]
    g264["crates/ariadne-scip/tests/fixtures/sample-react/src/legacy.jsx"]
    g265["crates/ariadne-scip/tests/fixtures/sample-rust/src/lib.rs"]
    g266["crates/ariadne-scip/tests/fixtures/sample-svelte/src/Button.svelte"]
    g267["crates/ariadne-scip/tests/fixtures/sample-vue/src/Button.vue"]
    g268["crates/ariadne-storage/src/adapters/mod.rs"]
    g269["crates/ariadne-storage/src/domain/mod.rs"]
    g270["crates/ariadne-storage/src/errors.rs"]
    g271["crates/ariadne-storage/src/lib.rs"]
    g272["crates/ariadne-storage/tests/smoke.rs"]
    g273["crates/ariadne-watcher/src/adapters/mod.rs"]
    g274["crates/ariadne-watcher/src/errors.rs"]
    g275["crates/ariadne-watcher/src/lib.rs"]
    g0 --> g5
    g1 --> g5
    g2 --> g5
    g3 --> g5
    g4 --> g5
    g6 --> g94
    g6 --> g177
    g7 --> g11
    g7 --> g177
    g8 --> g10
    g8 --> g11
    g8 --> g161
    g8 --> g177
    g8 --> g196
    g9 --> g10
    g9 --> g177
    g10 --> g11
    g10 --> g161
    g10 --> g177
    g11 --> g177
    g12 --> g177
    g13 --> g177
    g14 --> g53
    g14 --> g177
    g15 --> g177
    g15 --> g196
    g16 --> g177
    g17 --> g177
    g18 --> g177
    g19 --> g177
    g20 --> g177
    g21 --> g177
    g21 --> g204
    g22 --> g177
    g23 --> g177
    g24 --> g177
    g25 --> g177
    g26 --> g177
    g26 --> g206
    g27 --> g120
    g27 --> g177
    g28 --> g177
    g29 --> g177
    g30 --> g120
    g30 --> g177
    g31 --> g177
    g32 --> g53
    g32 --> g177
    g33 --> g53
    g33 --> g177
    g34 --> g53
    g34 --> g177
    g35 --> g53
    g35 --> g94
    g35 --> g177
    g35 --> g196
    g36 --> g53
    g36 --> g177
    g37 --> g53
    g37 --> g177
    g38 --> g53
    g38 --> g177
    g39 --> g53
    g39 --> g177
    g40 --> g53
    g40 --> g177
    g41 --> g53
    g41 --> g177
    g42 --> g53
    g42 --> g177
    g43 --> g53
    g43 --> g177
    g44 --> g53
    g44 --> g177
    g45 --> g53
    g45 --> g177
    g46 --> g53
    g46 --> g177
    g47 --> g53
    g47 --> g177
    g48 --> g53
    g48 --> g177
    g49 --> g53
    g49 --> g177
    g50 --> g53
    g50 --> g177
    g51 --> g53
    g51 --> g177
    g52 --> g53
    g52 --> g177
    g53 --> g177
    g53 --> g196
    g53 --> g204
    g54 --> g177
    g54 --> g204
    g55 --> g70
    g55 --> g116
    g55 --> g177
    g56 --> g70
    g57 --> g70
    g58 --> g70
    g59 --> g70
    g60 --> g70
    g61 --> g70
    g62 --> g70
    g63 --> g70
    g64 --> g70
    g65 --> g70
    g66 --> g70
    g66 --> g116
    g66 --> g177
    g67 --> g70
    g68 --> g70
    g69 --> g70
    g69 --> g116
    g69 --> g177
    g70 --> g177
    g70 --> g204
    g71 --> g177
    g72 --> g177
    g73 --> g177
    g74 --> g177
    g75 --> g177
    g76 --> g177
    g76 --> g178
    g77 --> g177
    g77 --> g178
    g77 --> g204
    g78 --> g145
    g78 --> g177
    g79 --> g80
    g79 --> g84
    g79 --> g111
    g79 --> g177
    g79 --> g178
    g79 --> g204
    g80 --> g90
    g80 --> g177
    g81 --> g90
    g81 --> g177
    g82 --> g90
    g82 --> g177
    g83 --> g90
    g83 --> g177
    g84 --> g90
    g84 --> g177
    g85 --> g90
    g85 --> g177
    g86 --> g90
    g86 --> g177
    g87 --> g90
    g87 --> g177
    g88 --> g90
    g88 --> g177
    g89 --> g90
    g89 --> g177
    g90 --> g177
    g91 --> g177
    g92 --> g177
    g93 --> g94
    g93 --> g177
    g93 --> g204
    g94 --> g177
    g94 --> g196
    g95 --> g163
    g95 --> g177
    g95 --> g204
    g96 --> g177
    g96 --> g204
    g97 --> g177
    g97 --> g204
    g98 --> g111
    g98 --> g177
    g99 --> g177
    g100 --> g177
    g101 --> g177
    g102 --> g177
    g103 --> g177
    g103 --> g204
    g103 --> g233
    g104 --> g177
    g105 --> g111
    g105 --> g177
    g106 --> g111
    g106 --> g177
    g107 --> g111
    g107 --> g177
    g108 --> g111
    g108 --> g177
    g109 --> g111
    g109 --> g177
    g110 --> g111
    g110 --> g177
    g111 --> g158
    g111 --> g177
    g112 --> g177
    g112 --> g204
    g113 --> g177
    g113 --> g204
    g113 --> g233
    g114 --> g177
    g115 --> g177
    g116 --> g177
    g117 --> g177
    g118 --> g177
    g119 --> g177
    g119 --> g204
    g120 --> g177
    g121 --> g177
    g122 --> g177
    g123 --> g177
    g123 --> g204
    g124 --> g177
    g125 --> g177
    g126 --> g177
    g127 --> g177
    g128 --> g177
    g129 --> g177
    g130 --> g177
    g131 --> g177
    g131 --> g204
    g132 --> g177
    g133 --> g177
    g134 --> g177
    g135 --> g177
    g135 --> g204
    g136 --> g177
    g137 --> g177
    g137 --> g204
    g138 --> g177
    g139 --> g177
    g140 --> g158
    g140 --> g177
    g140 --> g204
    g140 --> g206
    g141 --> g177
    g141 --> g204
    g141 --> g206
    g142 --> g177
    g142 --> g204
    g142 --> g206
    g143 --> g144
    g143 --> g177
    g144 --> g161
    g144 --> g177
    g145 --> g177
    g145 --> g204
    g145 --> g250
    g146 --> g177
    g147 --> g177
    g147 --> g196
    g148 --> g177
    g149 --> g177
    g150 --> g177
    g151 --> g177
    g152 --> g177
    g153 --> g177
    g154 --> g177
    g155 --> g177
    g155 --> g204
    g156 --> g177
    g156 --> g204
    g157 --> g177
    g157 --> g204
    g158 --> g177
    g159 --> g177
    g160 --> g161
    g160 --> g177
    g161 --> g177
    g162 --> g177
    g163 --> g177
    g164 --> g177
    g165 --> g177
    g166 --> g177
    g167 --> g177
    g168 --> g177
    g169 --> g177
    g170 --> g177
    g171 --> g177
    g172 --> g177
    g173 --> g177
    g174 --> g177
    g175 --> g177
    g176 --> g177
    g177 --> g178
    g177 --> g196
    g177 --> g204
    g177 --> g205
    g177 --> g206
    g177 --> g207
    g177 --> g209
    g177 --> g214
    g177 --> g227
    g177 --> g233
    g177 --> g250
    g177 --> g264
    g178 --> g183
    g179 --> g183
    g180 --> g183
    g181 --> g183
    g182 --> g183
    g184 --> g186
    g185 --> g186
    g204 --> g205
    g231 --> g261
    g238 --> g239
    g241 --> g242
    g258 --> g263
    g259 --> g261
    g260 --> g261
    g261 --> g263
    g262 --> g263
```

## Hot-Spots

| Module | Ce | Cycles | Dead | Score |
| --- | --- | --- | --- | --- |
| `crates/ariadne-parser/fixtures/javascript/jquery.js` | 7 | 1 | 855 | 863 |
| `tools/ariadne-sfc-scip/src/index.ts` | 16 | 0 | 172 | 188 |
| `crates/ariadne-mcp/src/server.rs` | 15 | 0 | 41 | 56 |
| `crates/ariadne-storage/src/adapters/redb/mod.rs` | 20 | 0 | 26 | 46 |
| `crates/ariadne-cli/src/domain/mod.rs` | 34 | 1 | 6 | 41 |
| `crates/ariadne-mcp/src/types.rs` | 0 | 0 | 40 | 40 |
| `crates/ariadne-daemon/src/domain/catalog.rs` | 32 | 1 | 5 | 38 |
| `crates/ariadne-storage/src/domain/migration.rs` | 9 | 0 | 28 | 37 |
| `crates/ariadne-daemon/src/adapters/ipc.rs` | 31 | 0 | 4 | 35 |
| `crates/ariadne-storage/tests/migration.rs` | 23 | 0 | 12 | 35 |

## Coupling

| Module | Ca | Ce | I | A | Distance |
| --- | --- | --- | --- | --- | --- |
| `crates/ariadne-cli/fixtures/astro/App.astro` | 0 | 1 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-cli/fixtures/astro/Child.astro` | 5 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-cli/fixtures/c/callee.c` | 5 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-cli/fixtures/c/caller.c` | 6 | 1 | 0.14 | 0.00 | 0.86 |
| `crates/ariadne-cli/fixtures/csharp/Callee.cs` | 2 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-cli/fixtures/csharp/Caller.cs` | 0 | 1 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-cli/fixtures/go/callee.go` | 0 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-cli/fixtures/go/caller.go` | 0 | 1 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-cli/fixtures/java/Callee.java` | 0 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-cli/fixtures/java/Caller.java` | 0 | 1 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-cli/fixtures/python/callee.py` | 0 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-cli/fixtures/python/caller.py` | 0 | 1 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-cli/fixtures/react/App.tsx` | 0 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-cli/fixtures/rust/callee.rs` | 0 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-cli/fixtures/rust/caller.rs` | 0 | 1 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-cli/fixtures/svelte/App.svelte` | 0 | 1 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-cli/fixtures/svelte/Child.svelte` | 0 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-cli/fixtures/typescript/callee.ts` | 0 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-cli/fixtures/typescript/caller.ts` | 0 | 1 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-cli/fixtures/vue/App.vue` | 0 | 1 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-cli/fixtures/vue/Child.vue` | 0 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-cli/src/adapters/daemon_client.rs` | 328 | 2 | 0.01 | 0.00 | 0.99 |
| `crates/ariadne-cli/src/adapters/mod.rs` | 0 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-cli/src/commands/daemon.rs` | 20 | 10 | 0.33 | 0.00 | 0.67 |
| `crates/ariadne-cli/src/commands/index.rs` | 14 | 29 | 0.67 | 0.00 | 0.33 |
| `crates/ariadne-cli/src/commands/init.rs` | 0 | 7 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-cli/src/commands/mem.rs` | 0 | 5 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-cli/src/commands/mod.rs` | 12 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-cli/src/commands/query.rs` | 23 | 9 | 0.28 | 0.00 | 0.72 |
| `crates/ariadne-cli/src/commands/serve.rs` | 0 | 5 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-cli/src/commands/setup.rs` | 0 | 7 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-cli/src/commands/status.rs` | 0 | 6 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-cli/src/commands/watch.rs` | 3 | 4 | 0.57 | 0.00 | 0.43 |
| `crates/ariadne-cli/src/config.rs` | 218 | 12 | 0.05 | 0.00 | 0.95 |
| `crates/ariadne-cli/src/domain/mod.rs` | 44 | 34 | 0.44 | 0.00 | 0.56 |
| `crates/ariadne-cli/src/errors.rs` | 0 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-cli/src/main.rs` | 0 | 7 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-cli/tests/incremental_history.rs` | 65 | 10 | 0.13 | 0.00 | 0.87 |
| `crates/ariadne-cli/tests/index_frameworks.rs` | 0 | 10 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-cli/tests/index_parity.rs` | 0 | 15 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-cli/tests/setup.rs` | 5 | 7 | 0.58 | 0.00 | 0.42 |
| `crates/ariadne-cli/tests/smoke.rs` | 0 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-core/benches/ids.rs` | 0 | 4 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-core/src/domain/changeset.rs` | 32 | 2 | 0.06 | 0.00 | 0.94 |
| `crates/ariadne-core/src/domain/daemon/mod.rs` | 0 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-core/src/domain/daemon/query.rs` | 0 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-core/src/domain/daemon/response.rs` | 0 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-core/src/domain/daemon/rows.rs` | 0 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-core/src/domain/mod.rs` | 0 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-core/src/domain/ports.rs` | 2 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-core/src/domain/records.rs` | 21 | 1 | 0.05 | 0.00 | 0.95 |
| `crates/ariadne-core/src/domain/types/ids.rs` | 121 | 1 | 0.01 | 0.00 | 0.99 |
| `crates/ariadne-core/src/domain/types/lang.rs` | 12 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-core/src/domain/types/mod.rs` | 2 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-core/src/domain/types/span.rs` | 0 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-core/src/domain/types/visibility.rs` | 1 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-core/src/domain/watcher.rs` | 0 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-core/src/errors.rs` | 0 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-core/src/lib.rs` | 0 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-core/tests/ids.rs` | 0 | 1 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-core/tests/tags.rs` | 0 | 1 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-daemon/benches/warm_query.rs` | 34 | 14 | 0.29 | 0.00 | 0.71 |
| `crates/ariadne-daemon/src/adapters/codec.rs` | 2 | 2 | 0.50 | 0.00 | 0.50 |
| `crates/ariadne-daemon/src/adapters/ipc.rs` | 2 | 31 | 0.94 | 0.00 | 0.06 |
| `crates/ariadne-daemon/src/adapters/mod.rs` | 0 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-daemon/src/domain/catalog.rs` | 59 | 32 | 0.35 | 0.00 | 0.65 |
| `crates/ariadne-daemon/src/domain/dispatch.rs` | 28 | 23 | 0.45 | 0.00 | 0.55 |
| `crates/ariadne-daemon/src/domain/dump.rs` | 151 | 12 | 0.07 | 0.00 | 0.93 |
| `crates/ariadne-daemon/src/domain/facts.rs` | 0 | 12 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-daemon/src/domain/index_lock.rs` | 2 | 1 | 0.33 | 0.00 | 0.67 |
| `crates/ariadne-daemon/src/domain/lifecycle.rs` | 2 | 2 | 0.50 | 0.00 | 0.50 |
| `crates/ariadne-daemon/src/domain/live.rs` | 41 | 26 | 0.39 | 0.00 | 0.61 |
| `crates/ariadne-daemon/src/domain/mod.rs` | 1 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-daemon/src/domain/queries/analytics.rs` | 2 | 21 | 0.91 | 0.00 | 0.09 |
| `crates/ariadne-daemon/src/domain/queries/docs.rs` | 1 | 12 | 0.92 | 0.00 | 0.08 |
| `crates/ariadne-daemon/src/domain/queries/health.rs` | 13 | 16 | 0.55 | 0.00 | 0.45 |
| `crates/ariadne-daemon/src/domain/queries/impact.rs` | 18 | 27 | 0.60 | 0.00 | 0.40 |
| `crates/ariadne-daemon/src/domain/queries/meta.rs` | 1 | 3 | 0.75 | 0.00 | 0.25 |
| `crates/ariadne-daemon/src/domain/queries/mod.rs` | 0 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-daemon/src/domain/queries/navigate.rs` | 1 | 14 | 0.93 | 0.00 | 0.07 |
| `crates/ariadne-daemon/src/domain/queries/refactor.rs` | 2 | 11 | 0.85 | 0.00 | 0.15 |
| `crates/ariadne-daemon/src/domain/snapshot.rs` | 32 | 10 | 0.24 | 0.00 | 0.76 |
| `crates/ariadne-daemon/src/errors.rs` | 60 | 1 | 0.02 | 0.00 | 0.98 |
| `crates/ariadne-daemon/src/lib.rs` | 0 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-daemon/tests/daemon.rs` | 0 | 7 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-daemon/tests/incremental_warm.rs` | 0 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-daemon/tests/live_update.rs` | 0 | 11 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-daemon/tests/memory_probe.rs` | 7 | 10 | 0.59 | 0.00 | 0.41 |
| `crates/ariadne-daemon/tests/support.rs` | 18 | 27 | 0.60 | 0.00 | 0.40 |
| `crates/ariadne-daemon/tests/warm_analytics.rs` | 0 | 24 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-daemon/tests/warm_graph.rs` | 0 | 12 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-e2e/src/domain/mod.rs` | 78 | 21 | 0.21 | 0.00 | 0.79 |
| `crates/ariadne-e2e/src/errors.rs` | 0 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-e2e/src/lib.rs` | 0 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-e2e/tests/cli_daemon_parity.rs` | 0 | 11 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-e2e/tests/mcp_session.rs` | 0 | 14 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-e2e/tests/repos/astro.rs` | 0 | 2 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-e2e/tests/repos/c.rs` | 0 | 2 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-e2e/tests/repos/cpp.rs` | 0 | 2 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-e2e/tests/repos/csharp.rs` | 0 | 2 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-e2e/tests/repos/go.rs` | 0 | 2 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-e2e/tests/repos/java.rs` | 0 | 2 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-e2e/tests/repos/python.rs` | 0 | 2 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-e2e/tests/repos/react.rs` | 0 | 2 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-e2e/tests/repos/rust.rs` | 0 | 2 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-e2e/tests/repos/svelte.rs` | 0 | 2 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-e2e/tests/repos/typescript.rs` | 0 | 2 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-e2e/tests/repos/vue.rs` | 0 | 2 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-e2e/tests/slo.rs` | 0 | 24 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-e2e/tests/smoke.rs` | 0 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-git/src/adapters/gix/diff.rs` | 12 | 13 | 0.52 | 0.00 | 0.48 |
| `crates/ariadne-git/src/adapters/gix/incremental.rs` | 6 | 4 | 0.40 | 0.00 | 0.60 |
| `crates/ariadne-git/src/adapters/gix/line_hunks.rs` | 7 | 7 | 0.50 | 0.00 | 0.50 |
| `crates/ariadne-git/src/adapters/gix/mod.rs` | 8 | 15 | 0.65 | 0.00 | 0.35 |
| `crates/ariadne-git/src/adapters/mod.rs` | 0 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-git/src/errors.rs` | 0 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-git/src/lib.rs` | 0 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-git/tests/diff.rs` | 0 | 6 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-git/tests/history.rs` | 0 | 7 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-git/tests/incremental.rs` | 0 | 8 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-git/tests/line_hunks.rs` | 0 | 5 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-graph/benches/blast.rs` | 0 | 10 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-graph/src/blast.rs` | 6 | 11 | 0.65 | 0.00 | 0.35 |
| `crates/ariadne-graph/src/build.rs` | 21 | 12 | 0.36 | 0.00 | 0.64 |
| `crates/ariadne-graph/src/co_change.rs` | 3 | 7 | 0.70 | 0.00 | 0.30 |
| `crates/ariadne-graph/src/coupling.rs` | 2 | 10 | 0.83 | 0.00 | 0.17 |
| `crates/ariadne-graph/src/cycles.rs` | 9 | 4 | 0.31 | 0.00 | 0.69 |
| `crates/ariadne-graph/src/dead.rs` | 6 | 5 | 0.45 | 0.00 | 0.55 |
| `crates/ariadne-graph/src/diff_blast.rs` | 0 | 8 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-graph/src/docgen.rs` | 9 | 25 | 0.74 | 0.00 | 0.26 |
| `crates/ariadne-graph/src/errors.rs` | 0 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-graph/src/heuristics.rs` | 20 | 11 | 0.35 | 0.00 | 0.65 |
| `crates/ariadne-graph/src/hotspot.rs` | 4 | 6 | 0.60 | 0.00 | 0.40 |
| `crates/ariadne-graph/src/lib.rs` | 0 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-graph/src/plan_assist.rs` | 2 | 15 | 0.88 | 0.00 | 0.12 |
| `crates/ariadne-graph/src/refactor.rs` | 6 | 20 | 0.77 | 0.00 | 0.23 |
| `crates/ariadne-graph/src/roots.rs` | 4 | 1 | 0.20 | 0.00 | 0.80 |
| `crates/ariadne-graph/src/span_lines.rs` | 5 | 8 | 0.62 | 0.00 | 0.38 |
| `crates/ariadne-graph/src/symbol_churn.rs` | 4 | 5 | 0.56 | 0.00 | 0.44 |
| `crates/ariadne-graph/tests/co_change.rs` | 4 | 5 | 0.56 | 0.00 | 0.44 |
| `crates/ariadne-graph/tests/component_graph.rs` | 5 | 9 | 0.64 | 0.00 | 0.36 |
| `crates/ariadne-graph/tests/dead_code_roots.rs` | 0 | 10 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-graph/tests/diff_blast.rs` | 0 | 8 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-graph/tests/docgen_fixture.rs` | 0 | 6 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-graph/tests/golden_repo.rs` | 0 | 10 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-graph/tests/hotspot.rs` | 0 | 5 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-graph/tests/refactor_cases.rs` | 0 | 10 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-graph/tests/support.rs` | 6 | 11 | 0.65 | 0.00 | 0.35 |
| `crates/ariadne-graph/tests/symbol_churn.rs` | 0 | 2 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-graph/tests/synthetic.rs` | 0 | 6 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-mcp/benches/cold_start.rs` | 0 | 13 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-mcp/benches/concurrent.rs` | 0 | 18 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-mcp/src/adapters/daemon_client.rs` | 31 | 5 | 0.14 | 0.00 | 0.86 |
| `crates/ariadne-mcp/src/adapters/mod.rs` | 0 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-mcp/src/bin/ariadne-mcp.rs` | 0 | 10 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-mcp/src/catalog.rs` | 0 | 15 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-mcp/src/errors.rs` | 3 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-mcp/src/lib.rs` | 0 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-mcp/src/serve.rs` | 5 | 4 | 0.44 | 0.00 | 0.56 |
| `crates/ariadne-mcp/src/server.rs` | 6 | 15 | 0.71 | 0.00 | 0.29 |
| `crates/ariadne-mcp/src/tools/blast_radius.rs` | 20 | 7 | 0.26 | 0.00 | 0.74 |
| `crates/ariadne-mcp/src/tools/co_change.rs` | 0 | 4 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-mcp/src/tools/complexity.rs` | 0 | 6 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-mcp/src/tools/coupling_report.rs` | 0 | 9 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-mcp/src/tools/diff_blast.rs` | 0 | 15 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-mcp/src/tools/doc_for.rs` | 0 | 8 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-mcp/src/tools/doc_module.rs` | 0 | 4 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-mcp/src/tools/doc_project.rs` | 0 | 3 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-mcp/src/tools/file_summary.rs` | 0 | 17 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-mcp/src/tools/find_definition.rs` | 0 | 2 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-mcp/src/tools/find_references.rs` | 0 | 11 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-mcp/src/tools/hotspots.rs` | 0 | 8 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-mcp/src/tools/list_symbols.rs` | 0 | 4 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-mcp/src/tools/mod.rs` | 0 | 6 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-mcp/src/tools/plan_assist.rs` | 0 | 12 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-mcp/src/tools/project_status.rs` | 0 | 3 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-mcp/src/tools/refactor.rs` | 0 | 12 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-mcp/src/tools/weak_spots.rs` | 0 | 15 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-mcp/src/types.rs` | 0 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-mcp/tests/catalog_projection.rs` | 0 | 4 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-mcp/tests/daemon_client.rs` | 0 | 8 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-mcp/tests/handshake.rs` | 0 | 6 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-mcp/tests/lazy_catalog.rs` | 0 | 15 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-mcp/tests/shutdown.rs` | 0 | 7 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-mcp/tests/smoke.rs` | 0 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-mcp/tests/support.rs` | 37 | 19 | 0.34 | 0.00 | 0.66 |
| `crates/ariadne-mcp/tests/tools_blast_radius.rs` | 0 | 7 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-mcp/tests/tools_co_change.rs` | 0 | 5 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-mcp/tests/tools_complexity.rs` | 0 | 5 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-mcp/tests/tools_component_graph.rs` | 0 | 5 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-mcp/tests/tools_coupling_report.rs` | 0 | 8 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-mcp/tests/tools_diff_blast.rs` | 0 | 19 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-mcp/tests/tools_doc.rs` | 0 | 6 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-mcp/tests/tools_doc_for.rs` | 0 | 7 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-mcp/tests/tools_file_summary.rs` | 0 | 7 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-mcp/tests/tools_find_definition.rs` | 0 | 5 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-mcp/tests/tools_find_references.rs` | 0 | 7 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-mcp/tests/tools_hotspots.rs` | 0 | 5 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-mcp/tests/tools_list_symbols.rs` | 0 | 7 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-mcp/tests/tools_plan_assist.rs` | 0 | 7 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-mcp/tests/tools_project_status.rs` | 0 | 5 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-mcp/tests/tools_refactor.rs` | 0 | 7 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-mcp/tests/tools_weak_spots.rs` | 0 | 8 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-parser/benches/parse.rs` | 0 | 9 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-parser/fixtures/astro/sample.astro` | 0 | 1 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-parser/fixtures/c/sample.c` | 0 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-parser/fixtures/cpp/sample.cpp` | 16 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-parser/fixtures/csharp/Sample.cs` | 0 | 1 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-parser/fixtures/go/sample.go` | 0 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-parser/fixtures/java/Sample.java` | 16 | 2 | 0.11 | 0.00 | 0.89 |
| `crates/ariadne-parser/fixtures/javascript/jquery.js` | 526 | 7 | 0.01 | 0.00 | 0.99 |
| `crates/ariadne-parser/fixtures/javascript/sample.js` | 2 | 1 | 0.33 | 0.00 | 0.67 |
| `crates/ariadne-parser/fixtures/kotlin/sample.kt` | 0 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-parser/fixtures/python/sample.py` | 0 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-parser/fixtures/react/sample.jsx` | 0 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-parser/fixtures/react/sample.tsx` | 1 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-parser/fixtures/rust/sample.rs` | 0 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-parser/fixtures/solid/sample.tsx` | 0 | 1 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-parser/fixtures/svelte/sample.svelte` | 0 | 1 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-parser/fixtures/typescript/sample.ts` | 1 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-parser/fixtures/vue/sample.vue` | 0 | 1 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-parser/fixtures/vue/script-tsx.vue` | 0 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-parser/fixtures/vue/two-scripts.vue` | 0 | 1 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-parser/src/adapters/mod.rs` | 0 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-parser/src/adapters/treesitter/cache.rs` | 3 | 4 | 0.57 | 0.00 | 0.43 |
| `crates/ariadne-parser/src/adapters/treesitter/complexity.rs` | 1 | 4 | 0.80 | 0.00 | 0.20 |
| `crates/ariadne-parser/src/adapters/treesitter/facts.rs` | 9 | 13 | 0.59 | 0.00 | 0.41 |
| `crates/ariadne-parser/src/adapters/treesitter/incremental.rs` | 13 | 5 | 0.28 | 0.00 | 0.72 |
| `crates/ariadne-parser/src/adapters/treesitter/injection.rs` | 1 | 9 | 0.90 | 0.00 | 0.10 |
| `crates/ariadne-parser/src/adapters/treesitter/mod.rs` | 0 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-parser/src/adapters/treesitter/registry.rs` | 4 | 4 | 0.50 | 0.00 | 0.50 |
| `crates/ariadne-parser/src/errors.rs` | 0 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-parser/src/lib.rs` | 0 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-parser/tests/common/mod.rs` | 17 | 4 | 0.19 | 0.00 | 0.81 |
| `crates/ariadne-parser/tests/complexity.rs` | 0 | 7 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-parser/tests/facts_astro.rs` | 0 | 4 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-parser/tests/facts_c.rs` | 0 | 1 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-parser/tests/facts_cpp.rs` | 0 | 1 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-parser/tests/facts_csharp.rs` | 0 | 1 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-parser/tests/facts_go.rs` | 0 | 1 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-parser/tests/facts_java.rs` | 0 | 1 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-parser/tests/facts_javascript.rs` | 0 | 1 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-parser/tests/facts_jsx.rs` | 0 | 1 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-parser/tests/facts_kotlin.rs` | 0 | 1 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-parser/tests/facts_python.rs` | 0 | 1 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-parser/tests/facts_rust.rs` | 0 | 1 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-parser/tests/facts_svelte.rs` | 0 | 4 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-parser/tests/facts_tsx.rs` | 0 | 1 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-parser/tests/facts_typescript.rs` | 0 | 1 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-parser/tests/facts_vue.rs` | 0 | 4 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-parser/tests/incremental.rs` | 0 | 2 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-parser/tests/incremental_svelte.rs` | 0 | 2 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-parser/tests/incremental_vue.rs` | 0 | 2 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-parser/tests/real_world.rs` | 0 | 5 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-parser/tests/smoke.rs` | 0 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-salsa/benches/edit.rs` | 0 | 8 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-salsa/src/db.rs` | 11 | 24 | 0.69 | 0.00 | 0.31 |
| `crates/ariadne-salsa/src/derive.rs` | 2 | 13 | 0.87 | 0.00 | 0.13 |
| `crates/ariadne-salsa/src/derived.rs` | 7 | 9 | 0.56 | 0.00 | 0.44 |
| `crates/ariadne-salsa/src/errors.rs` | 0 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-salsa/src/inputs.rs` | 5 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-salsa/src/lib.rs` | 0 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-salsa/src/memory.rs` | 3 | 8 | 0.73 | 0.00 | 0.27 |
| `crates/ariadne-salsa/tests/derivation.rs` | 0 | 12 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-salsa/tests/durability.rs` | 0 | 5 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-salsa/tests/equivalence.rs` | 0 | 8 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-salsa/tests/incremental.rs` | 0 | 22 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-salsa/tests/smoke.rs` | 0 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-scip/build.rs` | 0 | 1 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-scip/fixtures/astro/src/Page.astro` | 0 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-scip/fixtures/astro/src/util.ts` | 0 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-scip/fixtures/go/demo.go` | 0 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-scip/src/errors.rs` | 0 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-scip/src/indexer/mod.rs` | 2 | 2 | 0.50 | 0.00 | 0.50 |
| `crates/ariadne-scip/src/indexer/plan.rs` | 5 | 17 | 0.77 | 0.00 | 0.23 |
| `crates/ariadne-scip/src/indexer/rust_analyzer.rs` | 1 | 4 | 0.80 | 0.00 | 0.20 |
| `crates/ariadne-scip/src/indexer/scip_astro.rs` | 0 | 5 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-scip/src/indexer/scip_clang.rs` | 0 | 4 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-scip/src/indexer/scip_dotnet.rs` | 0 | 5 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-scip/src/indexer/scip_go.rs` | 1 | 4 | 0.80 | 0.00 | 0.20 |
| `crates/ariadne-scip/src/indexer/scip_java.rs` | 0 | 4 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-scip/src/indexer/scip_python.rs` | 0 | 4 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-scip/src/indexer/scip_svelte.rs` | 0 | 5 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-scip/src/indexer/scip_typescript.rs` | 0 | 6 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-scip/src/indexer/scip_vue.rs` | 0 | 5 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-scip/src/indexer/subprocess.rs` | 10 | 7 | 0.41 | 0.00 | 0.59 |
| `crates/ariadne-scip/src/lib.rs` | 0 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-scip/src/normalize/grammar.rs` | 0 | 3 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-scip/src/normalize/mod.rs` | 2 | 3 | 0.60 | 0.00 | 0.40 |
| `crates/ariadne-scip/tests/common/mod.rs` | 8 | 13 | 0.62 | 0.00 | 0.38 |
| `crates/ariadne-scip/tests/fixtures/sample-react/src/App.tsx` | 0 | 1 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-scip/tests/fixtures/sample-react/src/Button.tsx` | 3 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-scip/tests/fixtures/sample-react/src/legacy.jsx` | 1 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-scip/tests/fixtures/sample-rust/src/lib.rs` | 0 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-scip/tests/fixtures/sample-svelte/src/App.svelte` | 0 | 1 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-scip/tests/fixtures/sample-svelte/src/Button.svelte` | 0 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-scip/tests/fixtures/sample-svelte/src/Card.svelte` | 3 | 1 | 0.25 | 0.00 | 0.75 |
| `crates/ariadne-scip/tests/fixtures/sample-vue/src/App.vue` | 0 | 1 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-scip/tests/fixtures/sample-vue/src/Button.vue` | 0 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-scip/tests/fixtures/sample-vue/src/Card.vue` | 0 | 1 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-scip/tests/ingest_astro.rs` | 0 | 9 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-scip/tests/ingest_clang.rs` | 0 | 2 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-scip/tests/ingest_csharp.rs` | 0 | 2 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-scip/tests/ingest_go.rs` | 0 | 13 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-scip/tests/ingest_java.rs` | 0 | 2 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-scip/tests/ingest_plan.rs` | 0 | 9 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-scip/tests/ingest_python.rs` | 0 | 2 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-scip/tests/ingest_react.rs` | 0 | 11 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-scip/tests/ingest_rust.rs` | 0 | 2 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-scip/tests/ingest_svelte.rs` | 0 | 10 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-scip/tests/ingest_typescript.rs` | 0 | 2 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-scip/tests/ingest_vue.rs` | 0 | 10 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-scip/tests/normalize.rs` | 0 | 6 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-scip/tests/roundtrip.rs` | 0 | 4 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-storage/benches/apply.rs` | 0 | 12 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-storage/src/adapters/codec.rs` | 14 | 2 | 0.12 | 0.00 | 0.88 |
| `crates/ariadne-storage/src/adapters/mod.rs` | 0 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-storage/src/adapters/redb/apply.rs` | 1 | 15 | 0.94 | 0.00 | 0.06 |
| `crates/ariadne-storage/src/adapters/redb/history.rs` | 5 | 11 | 0.69 | 0.00 | 0.31 |
| `crates/ariadne-storage/src/adapters/redb/mod.rs` | 40 | 20 | 0.33 | 0.00 | 0.67 |
| `crates/ariadne-storage/src/adapters/redb/scan.rs` | 0 | 11 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-storage/src/adapters/redb/snapshot.rs` | 6 | 11 | 0.65 | 0.00 | 0.35 |
| `crates/ariadne-storage/src/domain/migration.rs` | 1 | 9 | 0.90 | 0.00 | 0.10 |
| `crates/ariadne-storage/src/domain/mod.rs` | 0 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-storage/src/errors.rs` | 0 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-storage/src/lib.rs` | 0 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-storage/tests/changeset.rs` | 0 | 19 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-storage/tests/history.rs` | 0 | 5 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-storage/tests/history_merge.rs` | 0 | 8 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-storage/tests/migration.rs` | 0 | 23 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-storage/tests/mvcc.rs` | 0 | 14 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-storage/tests/roundtrip.rs` | 0 | 1 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-storage/tests/smoke.rs` | 0 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-storage/tests/support.rs` | 1 | 3 | 0.75 | 0.00 | 0.25 |
| `crates/ariadne-storage/tests/symbol_churn.rs` | 0 | 5 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-watcher/benches/sink.rs` | 0 | 7 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-watcher/src/adapters/file_id_cache.rs` | 1 | 7 | 0.88 | 0.00 | 0.12 |
| `crates/ariadne-watcher/src/adapters/ignore.rs` | 6 | 6 | 0.50 | 0.00 | 0.50 |
| `crates/ariadne-watcher/src/adapters/mod.rs` | 0 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-watcher/src/adapters/notify.rs` | 0 | 13 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-watcher/src/adapters/reconcile.rs` | 4 | 12 | 0.75 | 0.00 | 0.25 |
| `crates/ariadne-watcher/src/adapters/sink.rs` | 1 | 10 | 0.91 | 0.00 | 0.09 |
| `crates/ariadne-watcher/src/errors.rs` | 0 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-watcher/src/lib.rs` | 0 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-watcher/tests/events.rs` | 0 | 16 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-watcher/tests/file_id_cache.rs` | 0 | 5 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-watcher/tests/ignore.rs` | 0 | 4 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-watcher/tests/reconcile.rs` | 0 | 6 | 1.00 | 0.00 | 0.00 |
| `tests/architecture.rs` | 0 | 5 | 1.00 | 0.00 | 0.00 |
| `tools/ariadne-sfc-scip/src/index.ts` | 24 | 16 | 0.40 | 0.00 | 0.60 |
| `tools/ariadne-sfc-scip/src/scip.ts` | 3 | 2 | 0.40 | 0.00 | 0.60 |

## Glossary

- `new` (function) — `crates/ariadne-cli/src/adapters/daemon_client.rs`
- `map` (variable) — `crates/ariadne-parser/fixtures/javascript/jquery.js`
- `path` (function) — `crates/ariadne-cli/src/config.rs`
- `collect` (function) — `crates/ariadne-daemon/src/domain/dump.rs`
- `clone` (variable) — `crates/ariadne-parser/fixtures/javascript/jquery.js`
- `push` (variable) — `crates/ariadne-parser/fixtures/javascript/jquery.js`
- `get` (function) — `crates/ariadne-core/src/domain/types/ids.rs`
- `len` (variable) — `crates/ariadne-parser/fixtures/javascript/jquery.js`
- `open` (function) — `crates/ariadne-cli/tests/incremental_history.rs`
- `insert` (variable) — `crates/ariadne-parser/fixtures/javascript/jquery.js`

