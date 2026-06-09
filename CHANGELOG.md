## Unreleased (21ab3c8..9b515b1)
### Package updates
### Global changes
#### Features
- (**ci**) land tier-00 foundations - (21ab3c8) - mind-decay
- (**cli**) steer symbol Grep/Read to search_code/read_symbol - (21dcae7) - mind-decay
- (**cli**) install PreToolUse grep advisory hook via setup - (8c7b759) - mind-decay
- (**cli**) inject digest at SessionStart via setup-installed hook - (59c4d9a) - mind-decay
- (**cli**) add digest command and always-load tool visibility - (e273c6d) - mind-decay
- (**cli**) land js-framework tier-05 framework detection + edges - (71dcd5f) - mind-decay
- (**cli**) land tiers 15-16 mcp discoverability + setup command - (8d91f70) - mind-decay
- (**cli**) land tiers 10-13 closing v1 SLO release gate - (2b1a0d3) - mind-decay
- (**core**) land post-v1 tier-02 schema migration + tier-04 symbol metadata enrichment - (3c649f2) - mind-decay
- (**core**) land tier-01 workspace skeleton - (b77bbbe) - mind-decay
- (**daemon**) project git-history analytics into warm catalog (tier-15a) - (86b59a5) - mind-decay
- (**daemon**) land post-v1 tier-06 skeleton + tier-07 warm graph - (1505f42) - mind-decay
- (**git**) add incremental git-history ingestion (tier-11a) - (a6a131f) - mind-decay
- (**git**) add git-history ingest adapter (tier-11) - (ac386d2) - mind-decay
- (**graph**) roll out response economy to diff-aware tools - (4f5855d) - mind-decay
- (**graph**) roll out cursor pagination and verbosity to list queries - (904a996) - mind-decay
- (**graph**) add response economy with cursor pagination and verbosity controls - (99e0f3a) - mind-decay
- (**graph**) add read_outline and fitness_report capabilities - (fd46d84) - mind-decay
- (**graph**) classify api-surface semver delta via mcp+cli api-diff - (0af641e) - mind-decay
- (**graph**) land affected-tests and api-surface plumbing - (d6daae8) - mind-decay
- (**graph**) honest project overview with uniform DocScope - (63adc4b) - mind-decay
- (**graph**) useful docgen redesign and god-module member suggestions - (c54557c) - mind-decay
- (**graph**) redesign module doc as crate-aware insight - (710f3c8) - mind-decay
- (**graph**) redesign project overview with insight sections - (92ddb19) - mind-decay
- (**graph**) add doc-scope model and deterministic SVG emitter - (f4339f0) - mind-decay
- (**graph**) diff-aware blast radius from a diff spec (tier-14) - (c83902a) - mind-decay
- (**graph**) add hotspot and change-coupling metrics (tier-13) - (5bc5136) - mind-decay
- (**graph**) attribute symbol churn to functions (tier-11b) - (8d7c3ec) - mind-decay
- (**graph**) land post-v1 tier-05 dead-code root classifier - (623b4b2) - mind-decay
- (**graph**) land tier-14 analytics-quality fixes - (c79f6ce) - mind-decay
- (**graph**) land tier-09 static doc-gen + refactor engine - (2de7c0b) - mind-decay
- (**graph**) land tier-07 in-RAM graph analytics - (97cde2e) - mind-decay
- (**mcp**) add search_code and read_symbol source-retrieval tools - (7aaf9f8) - mind-decay
- (**mcp**) expose diff_blast_radius tool (tier-15c) - (0909c2e) - mind-decay
- (**mcp**) expose hotspots, complexity, and co_change analytics tools (tier-15b) - (58f5e3d) - mind-decay
- (**mcp**) land tier-08 rmcp 1.7.0 stdio server - (aa1fb4d) - mind-decay
- (**parser**) add cyclomatic complexity facts (tier-12) - (0052cef) - mind-decay
- (**parser**) land js-framework tiers 03-04 vue/svelte/astro injection - (d44f683) - mind-decay
- (**parser**) land js-framework tiers 01-02 jsx/tsx parsing - (6277768) - mind-decay
- (**parser**) land tier-03 tree-sitter parser pipeline - (865a489) - mind-decay
- (**salsa**) land tier-04 incremental query layer - (83b54a8) - mind-decay
- (**scip**) drive impl/type edges and run SCIP default-on out-of-band - (ad59c2f) - mind-decay
- (**scip**) activate SCIP layer to drive precise graph edges - (bc82fbd) - mind-decay
- (**scip**) land post-v1 tier-03 astro semantic indexer - (03eb942) - mind-decay
- (**scip**) land post-v1 tier-01 native go scip indexer - (d363c2a) - mind-decay
- (**scip**) land js-framework tiers 08-09 svelte scip + component graph e2e - (db601f0) - mind-decay
- (**scip**) land js-framework tiers 06-07 jsx/tsx scip + vue sfc bridge - (1acd838) - mind-decay
- (**scip**) land tier-05 SCIP ingestion pipeline - (472a6bd) - mind-decay
- (**storage**) land tier-02 redb-backed Storage adapter - (edb257f) - mind-decay
- (**watcher**) land tier-06 file watcher + invalidation pipeline - (949d73c) - mind-decay
#### Bug Fixes
- (**ci**) scope audit-gate to real git commit/push, gate only on FAIL - (93b2ed8) - mind-decay
- (**salsa**) abstain method/path callees without a same-file definition - (97f122a) - mind-decay, *Claude Opus 4.8 (1M context)*
- (**salsa**) gate cross-crate fallback to free call shapes - (985116d) - mind-decay, *Claude Opus 4.8 (1M context)*
- (**salsa**) scope index-time call resolution to caller crate - (a2f6b45) - mind-decay, *Claude Opus 4.8 (1M context)*
#### Documentation
- (**ci**) reconcile tier-03 plan prose with windows decision - (9b515b1) - mind-decay
- (**ci**) mark tier-03 windows CI completed - (07432fc) - mind-decay
- (**daemon**) add tier-07a..10 plans, ADRs 0016/0017, and audit reports - (432d078) - mind-decay
- (**daemon**) add ADR-0015 and tier-06/07 plans + audit reports - (6b98334) - mind-decay
- (**docs**) tighten spec-audit INFO bar; forbid nitpick findings - (9536b42) - mind-decay
- (**graph**) land project overview on reliable resolver edges - (6011ea2) - mind-decay, *Claude Opus 4.8 (1M context)*
- (**mcp**) add ariadne-mcp-adoption plan and tiers - (5ba1fbc) - mind-decay
- (**mcp**) add mcp-startup-latency plan, tiers, and audit reports - (7a1d738) - mind-decay
- (**parser**) revise tier-12 cyclomatic-complexity plan - (aab8474) - mind-decay
- add r1-resolver and scip-driven-edges plans with audit records - (20eeaa5) - mind-decay
- split git-history ingestion into tiers 11/11a/11b - (5f94a9f) - mind-decay
#### Tests
- (**e2e**) add adoption wiring gate and behavioral harness - (b215999) - mind-decay
#### Build
- (**deps**) lock socket2 for tokio net feature - (aece779) - mind-decay
- (**deps**) sync lockfile for tier-07a..10 daemon/mcp/cli/salsa deps - (6f3f77f) - mind-decay
#### CI
- (**ci**) drop windows from nextest and release; keep clippy guard - (02e18e3) - mind-decay
- (**ci**) install cog 7.0.0 directly for v7 cog.toml - (fc96c4b) - mind-decay
- (**ci**) repair commits job with cocogitto-action - (b387a1c) - mind-decay
- register daemon crate as a commit scope - (3a28d62) - mind-decay


