---
tier_id: tier-06
title: CLI `doc` command — write .md + sidecar .svg
deps: [tier-03, tier-02]
exit_criteria:
  - "ariadne doc writes docs/codebase-overview.md AND docs/codebase-overview.svg (configurable paths); re-running is byte-identical"
  - "--exclude <glob> populates DocScope.extra_excludes and is honoured by the rendered output"
  - "regenerated docs/codebase-overview.svg opens and visually renders in a bare IDE Markdown preview (no extension); jquery.js absent, largest SCC named"
  - "cli doc-command test green; cargo clippy/fmt/deny/architecture green"
status: pending
---

<context>
The driving adapter that produces the committed human-facing artefact. Only this layer touches
the filesystem: it runs the graph use case cold (like `query`'s in-process `dispatch`), gets
Markdown from `for_project` and SVG bytes from `architecture_svg`, and writes both files with
`std::fs::write` — the read-only MCP tool returns a string and does no IO [src: plan.md D4;
crates/ariadne-cli/src/commands/query.rs:246-290; std::fs::write at commands/index.rs:303].
This tier also closes the loop: it regenerates this repo's own overview as end-to-end validation.
Full context: plan.md.
</context>

<files>
- crates/ariadne-cli/src/commands/doc.rs — NEW. `ariadne doc [--out PATH] [--svg PATH]
  [--exclude GLOB]...`; builds the cold catalog (`ariadne_mcp::Catalog::build` over `RedbStorage`,
  as `query` does), reads `cat.churn`/`cat.co_change`/`snapshot`, calls `docgen::for_project` +
  `architecture_svg` with a `DocScope` from `--exclude`, writes `.md` (relative `![](…svg)` link) + `.svg`.
- crates/ariadne-cli/src/commands/mod.rs — `pub mod doc;` and dispatch the new variant.
- crates/ariadne-cli/src/main.rs — add a `Doc { root, out, svg, exclude }` variant to the `Cmd` enum
  (main.rs:31) + its match arm.
- crates/ariadne-cli/src/config.rs — OPTIONAL default exclude globs + default out/svg paths.
- crates/ariadne-cli/tests/doc_command.rs — NEW. runs the command on a fixture repo, asserts both
  files written, byte-identical on re-run, and `--exclude` changes the output.
- docs/codebase-overview.md + docs/codebase-overview.svg — REGENERATED artefacts (committed).
</files>

<steps>
1. Write failing `tests/doc_command.rs`: invoke `ariadne doc --out <tmp>/o.md --svg <tmp>/o.svg`
   on a small fixture repo; assert both files exist, the `.md` contains `![architecture](o.svg)`
   (relative basename), re-running yields identical bytes, and adding `--exclude '**/fixtures/**'`
   removes a fixture row.
2. Implement `commands/doc.rs`: reuse the cold catalog-build path used by `query`
   [src: crates/ariadne-cli/src/commands/query.rs]; construct `DocScope { extra_excludes }` from
   `--exclude`; call `for_project(&cat.graph, &snap, &modules, &cat.churn, &cat.co_change, &scope)`
   and `architecture_svg(&cat.graph, &modules, &scope)`.
3. Write the `.svg` first, then the `.md` whose image link is the `.svg`'s relative basename, so the
   committed pair is self-contained and renders in-place. Use `std::fs::write` (std, no new dep).
4. Register the subcommand (`mod.rs`, `main.rs` `Cmd::Doc` arm); thread config defaults from `config.rs`.
5. Regenerate `docs/codebase-overview.md` + `docs/codebase-overview.svg` for this repo; **open the
   SVG in the IDE Markdown preview and confirm it draws** (golden-path validation, per project rule)
   [src: CLAUDE.md `<rules>` "Validate by execution"].
6. Confirm the regenerated overview shows the insight sections, omits `jquery.js`, and names the
   largest SCC — by reading the committed file, not by assumption.
</steps>

<verification>
- `cargo nextest run -p ariadne-cli` → doc_command green (both files, byte-identical re-run, exclude).
- End-to-end: `cargo run -p ariadne-cli -- doc` regenerates the repo overview; `git diff --stat`
  shows the `.md`+`.svg`; the `.svg` renders in a bare IDE preview.
- Determinism: run `ariadne doc` twice → `diff` empty for both outputs.
- `cargo clippy … -D warnings`; `cargo fmt --all --check`; `cargo deny check`; `cargo test --test architecture`.
</verification>

<rollback>
`git checkout -- crates/ariadne-cli docs/codebase-overview.md`; delete `commands/doc.rs`,
`tests/doc_command.rs`, and `docs/codebase-overview.svg`. The graph use cases (tier-01..05) are
unaffected — only the driving adapter and committed artefacts revert.
</rollback>
