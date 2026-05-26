---
tier_id: tier-17
title: LSP navigation — definition, references, hover, document/workspace symbols
deps: [tier-16]
exit_criteria:
  - `textDocument/definition`, `references`, `hover`, `documentSymbol`, and `workspace/symbol` are answered.
  - Each request maps a daemon query result back to LSP locations via the tier-16 position layer.
  - An LSP integration test drives all five requests against a spawned server with golden responses.
  - `cargo nextest run -p ariadne-lsp` + architecture + clippy + fmt all green.
status: pending
---

<context>
tier-16 gave `ariadne-lsp` a handshake and a daemon client. This tier delivers the core navigation features — the LSP equivalents of the v1 MCP tools `find_definition`, `find_references`, `doc_for`, and `list_symbols` (plan RD9, Block D). Full context: plan.md.
</context>

<files>
- crates/ariadne-lsp/src/adapters/server.rs — modify: register the five request handlers + advertise their capabilities.
- crates/ariadne-lsp/src/domain/ — modify: map daemon results to `lsp-types` `Location`/`Hover`/`SymbolInformation`.
- crates/ariadne-lsp/tests/ — new: navigation integration goldens.
- crates/ariadne-lsp/fixtures/ — new/modify: a small multi-file fixture project with known definitions/references.
</files>

<steps>
1. Failing test first (`ariadne-lsp` tests): a spawned LSP client opens the fixture, requests `definition` at a known reference, asserts the returned `Location`; repeat for `references`, `hover`, `documentSymbol`, `workspace/symbol`. Red — handlers are unregistered.
2. `textDocument/definition` → daemon `find_definition`; map the result symbol's file + byte span to an LSP `Location` via the tier-16 position layer.
3. `textDocument/references` → daemon `find_references`; map each occurrence to a `Location`; honor the `includeDeclaration` flag.
4. `textDocument/hover` → daemon `doc_for`; render the doc string as `Hover` markup.
5. `textDocument/documentSymbol` → daemon `list_symbols` scoped to the file; `workspace/symbol` → project-wide `list_symbols` filtered by the query string [src: https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/].
6. Advertise exactly these capabilities in the `initialize` result; no capability is announced without a handler.
7. Integration goldens for all five requests via the spawned-client harness.
</steps>

<verification>
- `cargo nextest run -p ariadne-lsp` — five navigation goldens green.
- Manual: open the ariadne_v2 repo in VS Code with `ariadne lsp`; "Go to Definition" and "Find All References" on a symbol resolve correctly; hover shows the doc.
- `cargo test --test architecture`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check` — green.
</verification>

<rollback>
`git checkout -- crates/ariadne-lsp`. The server reverts to the tier-16 initialize-only skeleton.
</rollback>
