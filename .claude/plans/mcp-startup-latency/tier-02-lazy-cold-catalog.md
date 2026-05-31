---
tier_id: tier-02
title: Lazy cold catalog — stop building the in-RAM graph on every warm session
deps: []
exit_criteria:
  - "build_server no longer calls Catalog::build; serve_stdio answers initialize without reading the full index."
  - "AriadneServer holds a lazily-built catalog (OnceCell) + a cheap startup-read revision; the catalog is built only on a cold-fallback miss."
  - "A test proves: with a reachable daemon, no tool call builds the catalog; with no daemon, the first cold-fallback tool call builds it once and answers correctly."
  - "cargo nextest run -p ariadne-mcp, clippy -D warnings, fmt --check, cargo test --test architecture all green."
status: completed
completed: 2026-05-31
---

<context>
`build_server` eagerly runs `Catalog::build` (full redb read → petgraph) on every
session, before the MCP handshake, even though tools route to the warm daemon and
the catalog is only the cold fallback [src: serve.rs:78-82; server.rs:130-134].
Defer the build so session-open is O(1) when the daemon answers (the common
case), building the catalog once on the first true cold miss. Full context:
plan.md.
</context>

<files>
- crates/ariadne-mcp/src/serve.rs — `build_server` stops building the catalog;
  reads `revision` via a transient storage handle, then drops it.
- crates/ariadne-mcp/src/server.rs — `AriadneServer`: replace `catalog: Arc<Catalog>`
  with `catalog: Arc<OnceCell<Arc<Catalog>>>`, add `revision: u64` and `root: PathBuf`;
  `new(db_path, root, revision)`; a `catalog().await` helper that builds on first
  miss via `spawn_blocking`; cold-fallback arms call it.
- crates/ariadne-storage/src/adapters/redb/mod.rs — confirm a cheap `revision()`
  read exists (single `KEY_REVISION`); expose if not already public to the port.
- crates/ariadne-mcp/tests/lazy_catalog.rs — new: failing-first behaviour test.
</files>

<steps>
1. Failing test first (`tests/lazy_catalog.rs`): build a server over a temp index
   with `ARIADNE_MCP_AUTOSPAWN=0` and no daemon; assert the catalog cell is empty
   after `build_server`; call a cold-fallback tool (e.g. `list_symbols`); assert it
   answers correctly and the cell is now populated. Red — `build_server` builds
   eagerly today. [src: server.rs:121-135]
2. Add a cheap revision read on the storage port: a single `KEY_REVISION` lookup,
   no graph build. Reuse the transient-open pattern already in `build_server`
   [src: serve.rs:78-81; adapters/redb/tables.rs KEY_REVISION].
3. Change `AriadneServer::new` to take `(db_path, root, revision)`; store
   `catalog: Arc<OnceCell<Arc<Catalog>>>` (tokio). `revision()` returns the stored
   `u64`; `DaemonClient::new(root)` no longer reads `catalog.root`
   [src: server.rs:87-95,112-113].
4. Add `async fn catalog(&self) -> Result<Arc<Catalog>, ErrorData>` using
   `OnceCell::get_or_try_init`, building inside `tokio::task::spawn_blocking`
   (`Catalog::build` is sync/CPU). Cold-fallback arms replace `&self.catalog`
   with `&*self.catalog().await?` [src: server.rs:133, all `tools::*::handle` calls;
   tokio 1.52 OnceCell].
5. `build_server`: open storage transiently, read `revision`, drop the handle,
   return `AriadneServer::new(storage_path, root, revision)` — no `Catalog::build`.
   `serve_stdio` unchanged otherwise [src: serve.rs:70-83].
6. Keep `catalog_arc()` (tests/benches) working by forcing a build, or gate it
   behind the lazy accessor [src: server.rs:99-101].
7. Run the full gate set; record the probe delta.
</steps>

<verification>
- `cargo nextest run -p ariadne-mcp` — lazy-catalog behaviour test green; existing
  cold-fallback tests still pass.
- `cargo clippy -p ariadne-mcp --all-targets -- -D warnings`; `cargo fmt --all --check`.
- `cargo test --test architecture` — `AriadneServer::new` signature change stays
  inside the adapter; no port/domain leak.
- End-to-end probe (real run): with a warm daemon already running, time
  `serve <root>` spawn→`initialize`; expected near-constant regardless of index
  symbol count (no `Catalog::build`). With no daemon + autospawn off, first tool
  call still answers from the cold catalog (correctness preserved). Use the
  session harness (/tmp/probe4.py warm/cold arms). Compare to the ~29 ms baseline;
  a build observed on the warm path is a fail.
- Memory: catalog now absent until cold-miss → on warm sessions the in-RAM graph
  table is not allocated; report the delta as "0 on warm path" (R1).
</verification>

<rollback>
Restore `AriadneServer { catalog: Arc<Catalog> }`, the eager `Catalog::build` in
`build_server`, and the `new(db_path, catalog)` signature. No persisted state
changes, so rollback is a pure code revert.
</rollback>
