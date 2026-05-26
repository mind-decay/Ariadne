---
tier_id: tier-16
title: LSP skeleton — ariadne-lsp crate on async-lsp, initialize/shutdown, daemon client
deps: [tier-07]
exit_criteria:
  - A new `ariadne-lsp` crate runs an `async-lsp` server over stdio answering `initialize`/`shutdown`.
  - `ariadne lsp` launches the server; it connects to the daemon as a thin client.
  - An offset-encoding layer converts between LSP UTF-16 positions and Ariadne byte offsets, property-tested.
  - ADR-0017 records the LSP adapter; `tests/architecture.rs` classifies `ariadne-lsp` as a driving adapter.
  - `cargo nextest run -p ariadne-lsp` + architecture + clippy + fmt all green.
status: pending
---

<context>
v1 reaches only Claude, via MCP. Block D adds a Language Server so any LSP editor benefits from the same graph. This tier ships the skeleton: the crate, an `async-lsp` server handshake, and the byte-offset ↔ LSP-position conversion every later LSP feature depends on (plan RD9). Navigation (tier-17) and hierarchy (tier-18) build on it. Full context: plan.md.
</context>

<files>
- crates/ariadne-lsp/Cargo.toml — new: deps `ariadne-core`, `async-lsp = "=0.2.4"` (tokio feature), `interprocess = "=2.4.2"`, `tokio`, `thiserror`.
- crates/ariadne-lsp/src/lib.rs — new: façade.
- crates/ariadne-lsp/src/domain/position.rs — new: UTF-16 position ↔ byte-offset conversion.
- crates/ariadne-lsp/src/adapters/server.rs — new: `async-lsp` `LspService` + router (one file, one tech).
- crates/ariadne-lsp/src/adapters/daemon_client.rs — new: thin IPC client (ADR-0015 per-adapter module).
- crates/ariadne-lsp/src/errors.rs — new: `thiserror` `LspError`.
- crates/ariadne-cli — modify: add an `ariadne lsp` subcommand (composition root, ADR-0007).
- tests/architecture.rs — modify: add `ariadne-lsp` to `DRIVING_ADAPTERS`.
- docs/adr/0017-lsp-adapter.md — new.
</files>

<steps>
1. Failing test first (`ariadne-lsp` tests): drive the server with a spawned LSP client — send `initialize`, assert server capabilities, send `shutdown`/`exit`, assert clean exit. Red — the crate does not exist.
2. Scaffold `ariadne-lsp` per `docs/folder-layout.md`.
3. Implement `adapters/server.rs`: an `async-lsp` `LspService` over a tower router on stdio; `async-lsp` is tower-`Layer`-based and its immutable-request / `&mut self`-notification split lets a request snapshot graph state then serve concurrently [src: https://lib.rs/crates/async-lsp]. Advertise only the capabilities later tiers implement.
4. Implement `domain/position.rs`: convert LSP positions (UTF-16 code units, default encoding) to/from byte offsets; negotiate `positionEncoding` if the client offers UTF-8 [src: https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/]. Property-test round-trips over multi-byte text (risk R-D1).
5. Implement `adapters/daemon_client.rs` reusing the tier-09 connect/auto-spawn/cold-fallback policy.
6. Wire `ariadne lsp` in `ariadne-cli`; classify `ariadne-lsp` in `tests/architecture.rs`.
7. Write ADR-0017: decision = `async-lsp`; rejected = `tower-lsp` (unmaintained), `lsp-server` (low-level sync), `tower-lsp-server`.
</steps>

<verification>
- `cargo nextest run -p ariadne-lsp` — initialize/shutdown + position round-trip property test green.
- Manual: point a VS Code generic LSP client (or `helix`) at `ariadne lsp`; confirm the server initializes without error.
- `cargo test --test architecture`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check` — green.
</verification>

<rollback>
`git checkout -- .` and `rm -rf crates/ariadne-lsp docs/adr/0017-lsp-adapter.md`. No tier depends on the LSP crate yet.
</rollback>
