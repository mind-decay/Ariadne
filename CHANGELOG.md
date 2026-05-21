# Changelog

All notable changes to Ariadne are documented here. Format follows
[Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/);
sections generated from commit history via `cog changelog`.

## 1.0.0 - 2026-05-22

First stable release. Covers the full `ariadne-core` build (tiers 00–16) and
the `js-framework-support` plan (tiers 01–09).

### Features

- (**ci**) land tier-00 foundations - (21ab3c8) - mind-decay
- (**core**) land tier-01 workspace skeleton - (b77bbbe) - mind-decay
- (**storage**) land tier-02 redb-backed Storage adapter - (edb257f) - mind-decay
- (**parser**) land tier-03 tree-sitter parser pipeline - (865a489) - mind-decay
- (**salsa**) land tier-04 incremental query layer - (83b54a8) - mind-decay
- (**scip**) land tier-05 SCIP ingestion pipeline - (472a6bd) - mind-decay
- (**watcher**) land tier-06 file watcher + invalidation pipeline - (949d73c) - mind-decay
- (**graph**) land tier-07 in-RAM graph analytics - (97cde2e) - mind-decay
- (**mcp**) land tier-08 rmcp 1.7.0 stdio server - (aa1fb4d) - mind-decay
- (**graph**) land tier-09 static doc-gen + refactor engine - (2de7c0b) - mind-decay
- (**cli**) land tiers 10-13 closing v1 SLO release gate - (2b1a0d3) - mind-decay
- (**graph**) land tier-14 analytics-quality fixes - (c79f6ce) - mind-decay
- (**cli**) land tiers 15-16 mcp discoverability + setup command - (8d91f70) - mind-decay
- (**parser**) land js-framework tiers 01-02 jsx/tsx parsing - (6277768) - mind-decay
- (**parser**) land js-framework tiers 03-04 vue/svelte/astro injection - (d44f683) - mind-decay
- (**cli**) land js-framework tier-05 framework detection + edges - (71dcd5f) - mind-decay
- (**scip**) land js-framework tiers 06-07 jsx/tsx scip + vue sfc bridge - (1acd838) - mind-decay
- (**scip**) land js-framework tiers 08-09 svelte scip + component graph e2e - (db601f0) - mind-decay

### Documentation

- (**docs**) tighten spec-audit INFO bar; forbid nitpick findings - (9536b42) - mind-decay
