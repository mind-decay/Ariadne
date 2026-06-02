# Project Architecture Overview

## Overview

349 modules · 3308 symbols · 4523 edges · 2 dependency cycle(s).

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
    g14["crates/ariadne-daemon/tests/live_update.rs"]
    g15["crates/ariadne-daemon/tests/warm_analytics.rs"]
    g16["crates/ariadne-daemon/tests/warm_graph.rs"]
    g17["crates/ariadne-cli/src/commands/status.rs"]
    g18["crates/ariadne-cli/src/commands/init.rs"]
    g19["crates/ariadne-cli/src/commands/setup.rs"]
    g20["crates/ariadne-cli/tests/index_parity.rs"]
    g21["crates/ariadne-cli/src/commands/mem.rs"]
    g22["crates/ariadne-core/benches/ids.rs"]
    g23["crates/ariadne-core/tests/ids.rs"]
    g24["crates/ariadne-core/tests/tags.rs"]
    g25["crates/ariadne-daemon/src/domain/facts.rs"]
    g26["crates/ariadne-graph/tests/docgen_fixture.rs"]
    g27["crates/ariadne-mcp/src/tools/doc_module.rs"]
    g28["crates/ariadne-mcp/src/tools/doc_project.rs"]
    g29["crates/ariadne-graph/tests/refactor_cases.rs"]
    g30["crates/ariadne-mcp/src/tools/refactor.rs"]
    g31["crates/ariadne-mcp/tests/catalog_projection.rs"]
    g32["crates/ariadne-mcp/tests/daemon_client.rs"]
    g33["crates/ariadne-mcp/tests/handshake.rs"]
    g34["crates/ariadne-mcp/tests/lazy_catalog.rs"]
    g35["crates/ariadne-mcp/tests/shutdown.rs"]
    g36["crates/ariadne-mcp/tests/tools_blast_radius.rs"]
    g37["crates/ariadne-mcp/tests/tools_co_change.rs"]
    g38["crates/ariadne-mcp/tests/tools_complexity.rs"]
    g39["crates/ariadne-mcp/tests/tools_component_graph.rs"]
    g40["crates/ariadne-mcp/tests/tools_coupling_report.rs"]
    g41["crates/ariadne-mcp/tests/tools_doc.rs"]
    g42["crates/ariadne-mcp/tests/tools_doc_for.rs"]
    g43["crates/ariadne-mcp/tests/tools_file_summary.rs"]
    g44["crates/ariadne-mcp/tests/tools_find_definition.rs"]
    g45["crates/ariadne-mcp/tests/tools_find_references.rs"]
    g46["crates/ariadne-mcp/tests/tools_hotspots.rs"]
    g47["crates/ariadne-mcp/tests/tools_list_symbols.rs"]
    g48["crates/ariadne-mcp/tests/tools_plan_assist.rs"]
    g49["crates/ariadne-mcp/tests/tools_project_status.rs"]
    g50["crates/ariadne-mcp/tests/tools_refactor.rs"]
    g51["crates/ariadne-mcp/tests/tools_weak_spots.rs"]
    g52["crates/ariadne-mcp/tests/support.rs"]
    g53["crates/ariadne-parser/benches/parse.rs"]
    g54["crates/ariadne-parser/tests/facts_astro.rs"]
    g55["crates/ariadne-parser/tests/facts_c.rs"]
    g56["crates/ariadne-parser/tests/facts_cpp.rs"]
    g57["crates/ariadne-parser/tests/facts_csharp.rs"]
    g58["crates/ariadne-parser/tests/facts_go.rs"]
    g59["crates/ariadne-parser/tests/facts_java.rs"]
    g60["crates/ariadne-parser/tests/facts_javascript.rs"]
    g61["crates/ariadne-parser/tests/facts_jsx.rs"]
    g62["crates/ariadne-parser/tests/facts_kotlin.rs"]
    g63["crates/ariadne-parser/tests/facts_python.rs"]
    g64["crates/ariadne-parser/tests/facts_rust.rs"]
    g65["crates/ariadne-parser/tests/facts_svelte.rs"]
    g66["crates/ariadne-parser/tests/facts_tsx.rs"]
    g67["crates/ariadne-parser/tests/facts_typescript.rs"]
    g68["crates/ariadne-parser/tests/facts_vue.rs"]
    g69["crates/ariadne-parser/tests/common/mod.rs"]
    g70["crates/ariadne-parser/tests/complexity.rs"]
    g71["crates/ariadne-parser/tests/real_world.rs"]
    g72["crates/ariadne-salsa/benches/edit.rs"]
    g73["crates/ariadne-salsa/tests/derivation.rs"]
    g74["crates/ariadne-salsa/tests/durability.rs"]
    g75["crates/ariadne-salsa/tests/equivalence.rs"]
    g76["crates/ariadne-salsa/tests/incremental.rs"]
    g77["crates/ariadne-watcher/tests/events.rs"]
    g78["crates/ariadne-scip/tests/ingest_go.rs"]
    g79["crates/ariadne-scip/src/indexer/rust_analyzer.rs"]
    g80["crates/ariadne-scip/src/indexer/scip_astro.rs"]
    g81["crates/ariadne-scip/src/indexer/scip_clang.rs"]
    g82["crates/ariadne-scip/src/indexer/scip_dotnet.rs"]
    g83["crates/ariadne-scip/src/indexer/scip_go.rs"]
    g84["crates/ariadne-scip/src/indexer/scip_java.rs"]
    g85["crates/ariadne-scip/src/indexer/scip_python.rs"]
    g86["crates/ariadne-scip/src/indexer/scip_svelte.rs"]
    g87["crates/ariadne-scip/src/indexer/scip_typescript.rs"]
    g88["crates/ariadne-scip/src/indexer/scip_vue.rs"]
    g89["crates/ariadne-scip/src/indexer/subprocess.rs"]
    g90["crates/ariadne-mcp/benches/concurrent.rs"]
    g91["crates/ariadne-mcp/src/catalog.rs"]
    g92["crates/ariadne-mcp/src/bin/ariadne-mcp.rs"]
    g93["crates/ariadne-mcp/src/serve.rs"]
    g94["crates/ariadne-storage/tests/changeset.rs"]
    g95["crates/ariadne-storage/tests/mvcc.rs"]
    g96["crates/ariadne-mcp/benches/cold_start.rs"]
    g97["crates/ariadne-scip/tests/ingest_plan.rs"]
    g98["crates/ariadne-parser/fixtures/csharp/Sample.cs"]
    g99["crates/ariadne-storage/benches/apply.rs"]
    g100["crates/ariadne-storage/tests/history_merge.rs"]
    g101["crates/ariadne-storage/tests/history.rs"]
    g102["crates/ariadne-storage/tests/migration.rs"]
    g103["crates/ariadne-storage/tests/symbol_churn.rs"]
    g104["crates/ariadne-scip/tests/ingest_clang.rs"]
    g105["crates/ariadne-scip/tests/ingest_csharp.rs"]
    g106["crates/ariadne-scip/tests/ingest_java.rs"]
    g107["crates/ariadne-scip/tests/ingest_python.rs"]
    g108["crates/ariadne-scip/tests/ingest_rust.rs"]
    g109["crates/ariadne-scip/tests/ingest_typescript.rs"]
    g110["crates/ariadne-scip/tests/common/mod.rs"]
    g111["crates/ariadne-scip/tests/roundtrip.rs"]
    g112["crates/ariadne-storage/src/adapters/redb/scan.rs"]
    g113["crates/ariadne-storage/tests/roundtrip.rs"]
    g114["crates/ariadne-graph/benches/blast.rs"]
    g115["crates/ariadne-graph/tests/component_graph.rs"]
    g116["crates/ariadne-graph/tests/dead_code_roots.rs"]
    g117["crates/ariadne-graph/tests/diff_blast.rs"]
    g118["crates/ariadne-graph/tests/golden_repo.rs"]
    g119["crates/ariadne-graph/tests/support.rs"]
    g120["crates/ariadne-graph/tests/synthetic.rs"]
    g121["crates/ariadne-mcp/src/tools/project_status.rs"]
    g122["crates/ariadne-mcp/src/tools/file_summary.rs"]
    g123["crates/ariadne-mcp/src/tools/find_references.rs"]
    g124["crates/ariadne-e2e/tests/cli_daemon_parity.rs"]
    g125["crates/ariadne-e2e/tests/mcp_session.rs"]
    g126["crates/ariadne-e2e/tests/slo.rs"]
    g127["crates/ariadne-mcp/src/tools/co_change.rs"]
    g128["crates/ariadne-mcp/src/tools/weak_spots.rs"]
    g129["crates/ariadne-graph/tests/hotspot.rs"]
    g130["crates/ariadne-mcp/src/tools/hotspots.rs"]
    g131["crates/ariadne-graph/src/diff_blast.rs"]
    g132["crates/ariadne-graph/tests/symbol_churn.rs"]
    g133["crates/ariadne-mcp/src/tools/mod.rs"]
    g134["crates/ariadne-git/tests/diff.rs"]
    g135["crates/ariadne-scip/tests/ingest_astro.rs"]
    g136["crates/ariadne-git/tests/line_hunks.rs"]
    g137["crates/ariadne-scip/src/normalize/grammar.rs"]
    g138["crates/ariadne-scip/tests/ingest_react.rs"]
    g139["crates/ariadne-scip/tests/ingest_svelte.rs"]
    g140["crates/ariadne-scip/tests/ingest_vue.rs"]
    g141["crates/ariadne-watcher/tests/file_id_cache.rs"]
    g142["crates/ariadne-watcher/src/adapters/file_id_cache.rs"]
    g143["crates/ariadne-watcher/src/adapters/sink.rs"]
    g144["tests/architecture.rs"]
    g145["crates/ariadne-daemon/tests/daemon.rs"]
    g146["crates/ariadne-git/tests/history.rs"]
    g147["crates/ariadne-git/tests/incremental.rs"]
    g148["crates/ariadne-mcp/src/tools/complexity.rs"]
    g149["crates/ariadne-mcp/src/tools/coupling_report.rs"]
    g150["crates/ariadne-mcp/src/tools/doc_for.rs"]
    g151["crates/ariadne-mcp/src/tools/list_symbols.rs"]
    g152["crates/ariadne-mcp/src/tools/plan_assist.rs"]
    g153["crates/ariadne-parser/tests/incremental.rs"]
    g154["crates/ariadne-parser/tests/incremental_svelte.rs"]
    g155["crates/ariadne-parser/tests/incremental_vue.rs"]
    g156["crates/ariadne-scip/src/normalize/mod.rs"]
    g157["crates/ariadne-scip/tests/normalize.rs"]
    g158["crates/ariadne-watcher/tests/ignore.rs"]
    g159["crates/ariadne-watcher/src/adapters/ignore.rs"]
    g160["crates/ariadne-scip/build.rs"]
    g161["crates/ariadne-storage/tests/support.rs"]
    g162["crates/ariadne-e2e/tests/repos/astro.rs"]
    g163["crates/ariadne-e2e/tests/repos/c.rs"]
    g164["crates/ariadne-e2e/tests/repos/cpp.rs"]
    g165["crates/ariadne-e2e/tests/repos/csharp.rs"]
    g166["crates/ariadne-e2e/tests/repos/go.rs"]
    g167["crates/ariadne-e2e/tests/repos/java.rs"]
    g168["crates/ariadne-e2e/tests/repos/python.rs"]
    g169["crates/ariadne-e2e/tests/repos/react.rs"]
    g170["crates/ariadne-e2e/tests/repos/rust.rs"]
    g171["crates/ariadne-e2e/tests/repos/svelte.rs"]
    g172["crates/ariadne-e2e/tests/repos/typescript.rs"]
    g173["crates/ariadne-e2e/tests/repos/vue.rs"]
    g174["crates/ariadne-mcp/src/tools/find_definition.rs"]
    g175["crates/ariadne-cli/src/adapters/daemon_client.rs ⇄ crates/ariadne-cli/src/commands/daemon.rs ⇄ crates/ariadne-cli/src/commands/index.rs ⇄ crates/ariadne-cli/src/commands/query.rs ⇄ crates/ariadne-cli/src/config.rs ⇄ crates/ariadne-cli/src/domain/mod.rs ⇄ crates/ariadne-cli/tests/incremental_history.rs ⇄ crates/ariadne-cli/tests/setup.rs ⇄ crates/ariadne-core/src/domain/changeset.rs ⇄ crates/ariadne-core/src/domain/records.rs ⇄ crates/ariadne-core/src/domain/types/ids.rs ⇄ crates/ariadne-daemon/benches/warm_query.rs ⇄ crates/ariadne-daemon/src/adapters/codec.rs ⇄ crates/ariadne-daemon/src/adapters/ipc.rs ⇄ crates/ariadne-daemon/src/domain/catalog.rs ⇄ crates/ariadne-daemon/src/domain/dispatch.rs ⇄ crates/ariadne-daemon/src/domain/dump.rs ⇄ crates/ariadne-daemon/src/domain/index_lock.rs ⇄ crates/ariadne-daemon/src/domain/lifecycle.rs ⇄ crates/ariadne-daemon/src/domain/live.rs ⇄ crates/ariadne-daemon/src/domain/queries/analytics.rs ⇄ crates/ariadne-daemon/src/domain/queries/docs.rs ⇄ crates/ariadne-daemon/src/domain/queries/health.rs ⇄ crates/ariadne-daemon/src/domain/queries/impact.rs ⇄ crates/ariadne-daemon/src/domain/queries/meta.rs ⇄ crates/ariadne-daemon/src/domain/queries/navigate.rs ⇄ crates/ariadne-daemon/src/domain/queries/refactor.rs ⇄ crates/ariadne-daemon/src/domain/snapshot.rs ⇄ crates/ariadne-daemon/tests/memory_probe.rs ⇄ crates/ariadne-daemon/tests/support.rs ⇄ crates/ariadne-e2e/src/domain/mod.rs ⇄ crates/ariadne-git/src/adapters/gix/diff.rs ⇄ crates/ariadne-git/src/adapters/gix/incremental.rs ⇄ crates/ariadne-git/src/adapters/gix/line_hunks.rs ⇄ crates/ariadne-git/src/adapters/gix/mod.rs ⇄ crates/ariadne-graph/src/blast.rs ⇄ crates/ariadne-graph/src/build.rs ⇄ crates/ariadne-graph/src/co_change.rs ⇄ crates/ariadne-graph/src/coupling.rs ⇄ crates/ariadne-graph/src/cycles.rs ⇄ crates/ariadne-graph/src/dead.rs ⇄ crates/ariadne-graph/src/docgen.rs ⇄ crates/ariadne-graph/src/heuristics.rs ⇄ crates/ariadne-graph/src/hotspot.rs ⇄ crates/ariadne-graph/src/plan_assist.rs ⇄ crates/ariadne-graph/src/refactor.rs ⇄ crates/ariadne-graph/src/roots.rs ⇄ crates/ariadne-graph/src/span_lines.rs ⇄ crates/ariadne-graph/src/symbol_churn.rs ⇄ crates/ariadne-graph/tests/co_change.rs ⇄ crates/ariadne-mcp/src/adapters/daemon_client.rs ⇄ crates/ariadne-mcp/src/server.rs ⇄ crates/ariadne-mcp/src/tools/blast_radius.rs ⇄ crates/ariadne-parser/fixtures/java/Sample.java ⇄ crates/ariadne-parser/fixtures/javascript/jquery.js ⇄ crates/ariadne-parser/fixtures/javascript/sample.js ⇄ crates/ariadne-parser/src/adapters/treesitter/cache.rs ⇄ crates/ariadne-parser/src/adapters/treesitter/complexity.rs ⇄ crates/ariadne-parser/src/adapters/treesitter/facts.rs ⇄ crates/ariadne-parser/src/adapters/treesitter/incremental.rs ⇄ crates/ariadne-parser/src/adapters/treesitter/injection.rs ⇄ crates/ariadne-parser/src/adapters/treesitter/registry.rs ⇄ crates/ariadne-salsa/src/db.rs ⇄ crates/ariadne-salsa/src/derive.rs ⇄ crates/ariadne-salsa/src/derived.rs ⇄ crates/ariadne-salsa/src/memory.rs ⇄ crates/ariadne-scip/src/indexer/mod.rs ⇄ crates/ariadne-scip/src/indexer/plan.rs ⇄ crates/ariadne-storage/src/adapters/codec.rs ⇄ crates/ariadne-storage/src/adapters/redb/apply.rs ⇄ crates/ariadne-storage/src/adapters/redb/history.rs ⇄ crates/ariadne-storage/src/adapters/redb/mod.rs ⇄ crates/ariadne-storage/src/adapters/redb/snapshot.rs ⇄ crates/ariadne-storage/src/domain/migration.rs ⇄ tools/ariadne-sfc-scip/src/index.ts ⇄ tools/ariadne-sfc-scip/src/scip.ts"]
    g176["crates/ariadne-cli/fixtures/c/caller.c"]
    g177["crates/ariadne-cli/fixtures/java/Caller.java"]
    g178["crates/ariadne-cli/fixtures/python/caller.py"]
    g179["crates/ariadne-cli/fixtures/rust/caller.rs"]
    g180["crates/ariadne-cli/fixtures/typescript/caller.ts"]
    g181["crates/ariadne-cli/fixtures/c/callee.c"]
    g182["crates/ariadne-cli/fixtures/csharp/Caller.cs"]
    g183["crates/ariadne-cli/fixtures/go/caller.go"]
    g184["crates/ariadne-cli/fixtures/csharp/Callee.cs"]
    g185["crates/ariadne-cli/fixtures/go/callee.go"]
    g186["crates/ariadne-cli/fixtures/java/Callee.java"]
    g187["crates/ariadne-cli/fixtures/python/callee.py"]
    g188["crates/ariadne-cli/fixtures/react/App.tsx"]
    g189["crates/ariadne-cli/fixtures/rust/callee.rs"]
    g190["crates/ariadne-cli/fixtures/svelte/Child.svelte"]
    g191["crates/ariadne-cli/fixtures/typescript/callee.ts"]
    g192["crates/ariadne-cli/fixtures/vue/Child.vue"]
    g193["crates/ariadne-cli/src/adapters/mod.rs"]
    g194["crates/ariadne-cli/src/commands/mod.rs"]
    g195["crates/ariadne-cli/src/errors.rs"]
    g196["crates/ariadne-cli/tests/smoke.rs"]
    g197["crates/ariadne-core/src/domain/daemon/mod.rs"]
    g198["crates/ariadne-core/src/domain/daemon/query.rs"]
    g199["crates/ariadne-core/src/domain/daemon/response.rs"]
    g200["crates/ariadne-core/src/domain/daemon/rows.rs"]
    g201["crates/ariadne-core/src/domain/mod.rs"]
    g202["crates/ariadne-daemon/src/errors.rs"]
    g203["crates/ariadne-core/src/domain/ports.rs"]
    g204["crates/ariadne-core/src/domain/types/lang.rs"]
    g205["crates/ariadne-core/src/domain/types/mod.rs"]
    g206["crates/ariadne-core/src/domain/types/span.rs"]
    g207["crates/ariadne-core/src/domain/types/visibility.rs"]
    g208["crates/ariadne-core/src/domain/watcher.rs"]
    g209["crates/ariadne-core/src/errors.rs"]
    g210["crates/ariadne-core/src/lib.rs"]
    g211["crates/ariadne-daemon/src/adapters/mod.rs"]
    g212["crates/ariadne-daemon/src/domain/mod.rs"]
    g213["crates/ariadne-daemon/src/domain/queries/mod.rs"]
    g214["crates/ariadne-daemon/src/lib.rs"]
    g215["crates/ariadne-daemon/tests/incremental_warm.rs"]
    g216["crates/ariadne-e2e/src/errors.rs"]
    g217["crates/ariadne-e2e/src/lib.rs"]
    g218["crates/ariadne-e2e/tests/smoke.rs"]
    g219["crates/ariadne-git/src/adapters/mod.rs"]
    g220["crates/ariadne-git/src/errors.rs"]
    g221["crates/ariadne-git/src/lib.rs"]
    g222["crates/ariadne-graph/src/errors.rs"]
    g223["crates/ariadne-graph/src/lib.rs"]
    g224["crates/ariadne-mcp/src/adapters/mod.rs"]
    g225["crates/ariadne-mcp/src/errors.rs"]
    g226["crates/ariadne-mcp/src/lib.rs"]
    g227["crates/ariadne-mcp/src/types.rs"]
    g228["crates/ariadne-mcp/tests/smoke.rs"]
    g229["crates/ariadne-parser/fixtures/astro/sample.astro"]
    g230["crates/ariadne-parser/fixtures/c/sample.c"]
    g231["crates/ariadne-parser/fixtures/cpp/sample.cpp"]
    g232["crates/ariadne-parser/fixtures/go/sample.go"]
    g233["crates/ariadne-parser/fixtures/kotlin/sample.kt"]
    g234["crates/ariadne-parser/fixtures/python/sample.py"]
    g235["crates/ariadne-parser/fixtures/react/sample.jsx"]
    g236["crates/ariadne-parser/fixtures/vue/two-scripts.vue"]
    g237["crates/ariadne-parser/fixtures/react/sample.tsx"]
    g238["crates/ariadne-parser/fixtures/rust/sample.rs"]
    g239["crates/ariadne-parser/fixtures/solid/sample.tsx"]
    g240["crates/ariadne-parser/fixtures/typescript/sample.ts"]
    g241["crates/ariadne-parser/fixtures/vue/script-tsx.vue"]
    g242["crates/ariadne-parser/src/adapters/mod.rs"]
    g243["crates/ariadne-parser/src/adapters/treesitter/mod.rs"]
    g244["crates/ariadne-parser/src/errors.rs"]
    g245["crates/ariadne-parser/src/lib.rs"]
    g246["crates/ariadne-parser/tests/smoke.rs"]
    g247["crates/ariadne-salsa/src/errors.rs"]
    g248["crates/ariadne-salsa/src/inputs.rs"]
    g249["crates/ariadne-salsa/src/lib.rs"]
    g250["crates/ariadne-salsa/tests/smoke.rs"]
    g251["crates/ariadne-scip/fixtures/astro/src/Page.astro"]
    g252["crates/ariadne-scip/fixtures/astro/src/util.ts"]
    g253["crates/ariadne-scip/fixtures/go/demo.go"]
    g254["crates/ariadne-scip/src/errors.rs"]
    g255["crates/ariadne-scip/src/lib.rs"]
    g256["crates/ariadne-scip/tests/fixtures/sample-react/src/App.tsx"]
    g257["crates/ariadne-scip/tests/fixtures/sample-svelte/src/App.svelte"]
    g258["crates/ariadne-scip/tests/fixtures/sample-vue/src/App.vue"]
    g259["crates/ariadne-scip/tests/fixtures/sample-svelte/src/Card.svelte"]
    g260["crates/ariadne-scip/tests/fixtures/sample-vue/src/Card.vue"]
    g261["crates/ariadne-scip/tests/fixtures/sample-react/src/Button.tsx"]
    g262["crates/ariadne-scip/tests/fixtures/sample-react/src/legacy.jsx"]
    g263["crates/ariadne-scip/tests/fixtures/sample-rust/src/lib.rs"]
    g264["crates/ariadne-scip/tests/fixtures/sample-svelte/src/Button.svelte"]
    g265["crates/ariadne-scip/tests/fixtures/sample-vue/src/Button.vue"]
    g266["crates/ariadne-storage/src/adapters/mod.rs"]
    g267["crates/ariadne-storage/src/domain/mod.rs"]
    g268["crates/ariadne-storage/src/errors.rs"]
    g269["crates/ariadne-storage/src/lib.rs"]
    g270["crates/ariadne-storage/tests/smoke.rs"]
    g271["crates/ariadne-watcher/src/adapters/mod.rs"]
    g272["crates/ariadne-watcher/src/errors.rs"]
    g273["crates/ariadne-watcher/src/lib.rs"]
    g0 --> g5
    g1 --> g5
    g2 --> g5
    g3 --> g5
    g4 --> g5
    g6 --> g93
    g6 --> g175
    g7 --> g11
    g7 --> g175
    g8 --> g10
    g8 --> g11
    g8 --> g159
    g8 --> g175
    g8 --> g194
    g9 --> g10
    g9 --> g175
    g10 --> g11
    g10 --> g159
    g10 --> g175
    g11 --> g175
    g12 --> g175
    g13 --> g175
    g14 --> g175
    g14 --> g194
    g15 --> g175
    g16 --> g175
    g17 --> g175
    g18 --> g175
    g19 --> g175
    g20 --> g175
    g20 --> g202
    g21 --> g175
    g22 --> g175
    g23 --> g175
    g24 --> g175
    g25 --> g175
    g25 --> g204
    g26 --> g119
    g26 --> g175
    g27 --> g175
    g28 --> g175
    g29 --> g119
    g29 --> g175
    g30 --> g175
    g31 --> g52
    g31 --> g175
    g32 --> g52
    g32 --> g175
    g33 --> g52
    g33 --> g175
    g34 --> g52
    g34 --> g93
    g34 --> g175
    g34 --> g194
    g35 --> g52
    g35 --> g175
    g36 --> g52
    g36 --> g175
    g37 --> g52
    g37 --> g175
    g38 --> g52
    g38 --> g175
    g39 --> g52
    g39 --> g175
    g40 --> g52
    g40 --> g175
    g41 --> g52
    g41 --> g175
    g42 --> g52
    g42 --> g175
    g43 --> g52
    g43 --> g175
    g44 --> g52
    g44 --> g175
    g45 --> g52
    g45 --> g175
    g46 --> g52
    g46 --> g175
    g47 --> g52
    g47 --> g175
    g48 --> g52
    g48 --> g175
    g49 --> g52
    g49 --> g175
    g50 --> g52
    g50 --> g175
    g51 --> g52
    g51 --> g175
    g52 --> g175
    g52 --> g194
    g52 --> g202
    g53 --> g175
    g53 --> g202
    g54 --> g69
    g54 --> g115
    g54 --> g175
    g55 --> g69
    g56 --> g69
    g57 --> g69
    g58 --> g69
    g59 --> g69
    g60 --> g69
    g61 --> g69
    g62 --> g69
    g63 --> g69
    g64 --> g69
    g65 --> g69
    g65 --> g115
    g65 --> g175
    g66 --> g69
    g67 --> g69
    g68 --> g69
    g68 --> g115
    g68 --> g175
    g69 --> g175
    g69 --> g202
    g70 --> g175
    g71 --> g175
    g72 --> g175
    g73 --> g175
    g74 --> g175
    g75 --> g175
    g75 --> g176
    g76 --> g175
    g76 --> g176
    g76 --> g202
    g77 --> g143
    g77 --> g175
    g78 --> g79
    g78 --> g83
    g78 --> g110
    g78 --> g175
    g78 --> g176
    g78 --> g202
    g79 --> g89
    g79 --> g175
    g80 --> g89
    g80 --> g175
    g81 --> g89
    g81 --> g175
    g82 --> g89
    g82 --> g175
    g83 --> g89
    g83 --> g175
    g84 --> g89
    g84 --> g175
    g85 --> g89
    g85 --> g175
    g86 --> g89
    g86 --> g175
    g87 --> g89
    g87 --> g175
    g88 --> g89
    g88 --> g175
    g89 --> g175
    g90 --> g175
    g91 --> g175
    g92 --> g93
    g92 --> g175
    g92 --> g202
    g93 --> g175
    g93 --> g194
    g94 --> g161
    g94 --> g175
    g94 --> g202
    g95 --> g175
    g95 --> g202
    g96 --> g175
    g96 --> g202
    g97 --> g110
    g97 --> g175
    g98 --> g175
    g99 --> g175
    g100 --> g175
    g101 --> g175
    g102 --> g175
    g102 --> g202
    g102 --> g231
    g103 --> g175
    g104 --> g110
    g104 --> g175
    g105 --> g110
    g105 --> g175
    g106 --> g110
    g106 --> g175
    g107 --> g110
    g107 --> g175
    g108 --> g110
    g108 --> g175
    g109 --> g110
    g109 --> g175
    g110 --> g156
    g110 --> g175
    g111 --> g175
    g111 --> g202
    g112 --> g175
    g112 --> g202
    g112 --> g231
    g113 --> g175
    g114 --> g175
    g115 --> g175
    g116 --> g175
    g117 --> g131
    g117 --> g175
    g118 --> g175
    g118 --> g202
    g119 --> g175
    g120 --> g175
    g121 --> g175
    g122 --> g175
    g122 --> g202
    g123 --> g175
    g124 --> g175
    g125 --> g175
    g126 --> g175
    g127 --> g175
    g128 --> g175
    g129 --> g175
    g129 --> g202
    g130 --> g175
    g131 --> g175
    g132 --> g175
    g133 --> g175
    g133 --> g202
    g134 --> g175
    g135 --> g175
    g135 --> g202
    g136 --> g175
    g137 --> g175
    g138 --> g156
    g138 --> g175
    g138 --> g202
    g138 --> g204
    g139 --> g175
    g139 --> g202
    g139 --> g204
    g140 --> g175
    g140 --> g202
    g140 --> g204
    g141 --> g142
    g141 --> g175
    g142 --> g159
    g142 --> g175
    g143 --> g175
    g143 --> g202
    g143 --> g248
    g144 --> g175
    g145 --> g175
    g145 --> g194
    g146 --> g175
    g147 --> g175
    g148 --> g175
    g149 --> g175
    g150 --> g175
    g151 --> g175
    g152 --> g175
    g153 --> g175
    g153 --> g202
    g154 --> g175
    g154 --> g202
    g155 --> g175
    g155 --> g202
    g156 --> g175
    g157 --> g175
    g158 --> g159
    g158 --> g175
    g159 --> g175
    g160 --> g175
    g161 --> g175
    g162 --> g175
    g163 --> g175
    g164 --> g175
    g165 --> g175
    g166 --> g175
    g167 --> g175
    g168 --> g175
    g169 --> g175
    g170 --> g175
    g171 --> g175
    g172 --> g175
    g173 --> g175
    g174 --> g175
    g175 --> g176
    g175 --> g194
    g175 --> g202
    g175 --> g203
    g175 --> g204
    g175 --> g205
    g175 --> g207
    g175 --> g212
    g175 --> g225
    g175 --> g231
    g175 --> g248
    g175 --> g262
    g176 --> g181
    g177 --> g181
    g178 --> g181
    g179 --> g181
    g180 --> g181
    g182 --> g184
    g183 --> g184
    g202 --> g203
    g229 --> g259
    g236 --> g237
    g239 --> g240
    g256 --> g261
    g257 --> g259
    g258 --> g259
    g259 --> g261
    g260 --> g261
```

## Hot-Spots

| Module | Ce | Cycles | Dead | Score |
| --- | --- | --- | --- | --- |
| `crates/ariadne-parser/fixtures/javascript/jquery.js` | 7 | 1 | 855 | 863 |
| `tools/ariadne-sfc-scip/src/index.ts` | 16 | 0 | 172 | 188 |
| `crates/ariadne-mcp/src/server.rs` | 14 | 0 | 39 | 53 |
| `crates/ariadne-storage/src/adapters/redb/mod.rs` | 20 | 0 | 26 | 46 |
| `crates/ariadne-cli/src/domain/mod.rs` | 34 | 1 | 6 | 41 |
| `crates/ariadne-daemon/src/domain/catalog.rs` | 32 | 1 | 5 | 38 |
| `crates/ariadne-storage/src/domain/migration.rs` | 9 | 0 | 28 | 37 |
| `crates/ariadne-mcp/src/types.rs` | 0 | 0 | 36 | 36 |
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
| `crates/ariadne-cli/src/adapters/daemon_client.rs` | 322 | 2 | 0.01 | 0.00 | 0.99 |
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
| `crates/ariadne-cli/src/config.rs` | 217 | 12 | 0.05 | 0.00 | 0.95 |
| `crates/ariadne-cli/src/domain/mod.rs` | 43 | 34 | 0.44 | 0.00 | 0.56 |
| `crates/ariadne-cli/src/errors.rs` | 0 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-cli/src/main.rs` | 0 | 7 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-cli/tests/incremental_history.rs` | 64 | 10 | 0.14 | 0.00 | 0.86 |
| `crates/ariadne-cli/tests/index_frameworks.rs` | 0 | 10 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-cli/tests/index_parity.rs` | 0 | 15 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-cli/tests/setup.rs` | 5 | 7 | 0.58 | 0.00 | 0.42 |
| `crates/ariadne-cli/tests/smoke.rs` | 0 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-core/benches/ids.rs` | 0 | 4 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-core/src/domain/changeset.rs` | 31 | 2 | 0.06 | 0.00 | 0.94 |
| `crates/ariadne-core/src/domain/daemon/mod.rs` | 0 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-core/src/domain/daemon/query.rs` | 0 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-core/src/domain/daemon/response.rs` | 0 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-core/src/domain/daemon/rows.rs` | 0 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-core/src/domain/mod.rs` | 0 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-core/src/domain/ports.rs` | 2 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-core/src/domain/records.rs` | 21 | 1 | 0.05 | 0.00 | 0.95 |
| `crates/ariadne-core/src/domain/types/ids.rs` | 119 | 1 | 0.01 | 0.00 | 0.99 |
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
| `crates/ariadne-daemon/src/domain/dispatch.rs` | 26 | 22 | 0.46 | 0.00 | 0.54 |
| `crates/ariadne-daemon/src/domain/dump.rs` | 148 | 12 | 0.08 | 0.00 | 0.93 |
| `crates/ariadne-daemon/src/domain/facts.rs` | 0 | 12 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-daemon/src/domain/index_lock.rs` | 2 | 1 | 0.33 | 0.00 | 0.67 |
| `crates/ariadne-daemon/src/domain/lifecycle.rs` | 2 | 2 | 0.50 | 0.00 | 0.50 |
| `crates/ariadne-daemon/src/domain/live.rs` | 40 | 26 | 0.39 | 0.00 | 0.61 |
| `crates/ariadne-daemon/src/domain/mod.rs` | 1 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-daemon/src/domain/queries/analytics.rs` | 1 | 21 | 0.95 | 0.00 | 0.05 |
| `crates/ariadne-daemon/src/domain/queries/docs.rs` | 1 | 12 | 0.92 | 0.00 | 0.08 |
| `crates/ariadne-daemon/src/domain/queries/health.rs` | 13 | 16 | 0.55 | 0.00 | 0.45 |
| `crates/ariadne-daemon/src/domain/queries/impact.rs` | 17 | 23 | 0.57 | 0.00 | 0.43 |
| `crates/ariadne-daemon/src/domain/queries/meta.rs` | 1 | 3 | 0.75 | 0.00 | 0.25 |
| `crates/ariadne-daemon/src/domain/queries/mod.rs` | 0 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-daemon/src/domain/queries/navigate.rs` | 1 | 14 | 0.93 | 0.00 | 0.07 |
| `crates/ariadne-daemon/src/domain/queries/refactor.rs` | 2 | 11 | 0.85 | 0.00 | 0.15 |
| `crates/ariadne-daemon/src/domain/snapshot.rs` | 30 | 10 | 0.25 | 0.00 | 0.75 |
| `crates/ariadne-daemon/src/errors.rs` | 60 | 1 | 0.02 | 0.00 | 0.98 |
| `crates/ariadne-daemon/src/lib.rs` | 0 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-daemon/tests/daemon.rs` | 0 | 7 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-daemon/tests/incremental_warm.rs` | 0 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-daemon/tests/live_update.rs` | 0 | 11 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-daemon/tests/memory_probe.rs` | 7 | 10 | 0.59 | 0.00 | 0.41 |
| `crates/ariadne-daemon/tests/support.rs` | 18 | 27 | 0.60 | 0.00 | 0.40 |
| `crates/ariadne-daemon/tests/warm_analytics.rs` | 0 | 24 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-daemon/tests/warm_graph.rs` | 0 | 12 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-e2e/src/domain/mod.rs` | 76 | 21 | 0.22 | 0.00 | 0.78 |
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
| `crates/ariadne-git/src/adapters/gix/diff.rs` | 11 | 13 | 0.54 | 0.00 | 0.46 |
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
| `crates/ariadne-graph/src/diff_blast.rs` | 1 | 8 | 0.89 | 0.00 | 0.11 |
| `crates/ariadne-graph/src/docgen.rs` | 9 | 25 | 0.74 | 0.00 | 0.26 |
| `crates/ariadne-graph/src/errors.rs` | 0 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-graph/src/heuristics.rs` | 20 | 11 | 0.35 | 0.00 | 0.65 |
| `crates/ariadne-graph/src/hotspot.rs` | 4 | 6 | 0.60 | 0.00 | 0.40 |
| `crates/ariadne-graph/src/lib.rs` | 0 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-graph/src/plan_assist.rs` | 2 | 15 | 0.88 | 0.00 | 0.12 |
| `crates/ariadne-graph/src/refactor.rs` | 6 | 20 | 0.77 | 0.00 | 0.23 |
| `crates/ariadne-graph/src/roots.rs` | 4 | 1 | 0.20 | 0.00 | 0.80 |
| `crates/ariadne-graph/src/span_lines.rs` | 2 | 5 | 0.71 | 0.00 | 0.29 |
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
| `crates/ariadne-mcp/src/adapters/daemon_client.rs` | 30 | 5 | 0.14 | 0.00 | 0.86 |
| `crates/ariadne-mcp/src/adapters/mod.rs` | 0 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-mcp/src/bin/ariadne-mcp.rs` | 0 | 10 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-mcp/src/catalog.rs` | 0 | 15 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-mcp/src/errors.rs` | 2 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-mcp/src/lib.rs` | 0 | 0 | 0.00 | 0.00 | 1.00 |
| `crates/ariadne-mcp/src/serve.rs` | 5 | 4 | 0.44 | 0.00 | 0.56 |
| `crates/ariadne-mcp/src/server.rs` | 6 | 14 | 0.70 | 0.00 | 0.30 |
| `crates/ariadne-mcp/src/tools/blast_radius.rs` | 19 | 7 | 0.27 | 0.00 | 0.73 |
| `crates/ariadne-mcp/src/tools/co_change.rs` | 0 | 4 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-mcp/src/tools/complexity.rs` | 0 | 6 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-mcp/src/tools/coupling_report.rs` | 0 | 9 | 1.00 | 0.00 | 0.00 |
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
| `crates/ariadne-mcp/tests/support.rs` | 36 | 19 | 0.35 | 0.00 | 0.65 |
| `crates/ariadne-mcp/tests/tools_blast_radius.rs` | 0 | 7 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-mcp/tests/tools_co_change.rs` | 0 | 5 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-mcp/tests/tools_complexity.rs` | 0 | 5 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-mcp/tests/tools_component_graph.rs` | 0 | 5 | 1.00 | 0.00 | 0.00 |
| `crates/ariadne-mcp/tests/tools_coupling_report.rs` | 0 | 8 | 1.00 | 0.00 | 0.00 |
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
| `crates/ariadne-parser/fixtures/javascript/jquery.js` | 515 | 7 | 0.01 | 0.00 | 0.99 |
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
| `crates/ariadne-storage/src/adapters/redb/mod.rs` | 39 | 20 | 0.34 | 0.00 | 0.66 |
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

