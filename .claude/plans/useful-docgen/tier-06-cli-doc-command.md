---
tier_id: tier-06
title: CLI `doc` command — write .md + sidecar .svg
deps: [tier-03, tier-02]
exit_criteria:
  - "ariadne doc writes docs/codebase-overview.md AND docs/codebase-overview.svg (configurable paths); re-running is byte-identical"
  - "--exclude <glob> populates DocScope.extra_excludes and is honoured by the rendered output"
  - "regenerated docs/codebase-overview.svg opens and visually renders in a bare IDE Markdown preview (no extension); jquery.js absent, big SCC named"
  - "cli doc-command test green; cargo clippy/fmt/deny/architecture green"
status: pending
---

<context>
The driving adapter that produces the committed human-facing artefact. Only this layer touches
the filesystem: it runs the graph use case cold (like `query`), gets Markdown from `for_project`
and SVG bytes from `architecture_svg`, and writes both files — the read-only MCP tool returns a
string and must not do IO [src: plan.md D4; crates/ariadne-cli/src/commands/query.rs:255-275].
This tier also closes the loop: it regenerates this repo's own overview as end-to-end validation.
Full context: plan.md.
</context>

<files>
- crates/ariadne-cli/src/commands/doc.rs — NEW. `ariadne doc [--out PATH] [--svg PATH]
  [--exclude GLOB]...`; builds the cold graph, calls `docgen::for_project` + `architecture_svg`
  with a `DocScope` built from `--exclude`, writes `.md` (relative `![](…svg)` link) + `.svg`.
- crates/ariadne-cli/src/commands/mod.rs — register the subcommand.
- crates/ariadne-cli/src/main.rs — add the `Doc` variant to the CLI arg enum.
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
2. Implement `commands/doc.rs`: reuse the cold graph-build path used by `query`/`index`
   [src: crates/ariadne-cli/src/commands/query.rs]; construct `DocScope { extra_excludes }` from
   `--exclude`; call `for_project(&graph, &snap, &modules, &scope)` and
   `architecture_svg(&graph, &modules, &scope)`.
3. Write the `.svg` first, then the `.md` whose image link is the `.svg`'s relative basename, so the
   committed pair is self-contained and renders in-place.
4. Register the subcommand (`mod.rs`, `main.rs`); thread config defaults from `config.rs`.
5. Regenerate `docs/codebase-overview.md` + `docs/codebase-overview.svg` for this repo; **open the
   SVG in the IDE Markdown preview and confirm it draws** (golden-path validation, per project rule)
   [src: CLAUDE.md `<rules>` "Validate by execution"].
6. Confirm the regenerated overview shows the six insight sections, omits `jquery.js`, and names the
   ~100-file SCC — by reading the committed file, not by assumption.
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
