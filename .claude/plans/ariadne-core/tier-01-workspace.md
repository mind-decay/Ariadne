---
tier_id: tier-01
title: Cargo workspace skeleton, core crate (ports), per-crate hexagonal layout
deps: [tier-00]
exit_criteria:
  - Cargo workspace builds with all 10 crates declared (each compiles as empty lib).
  - Each crate follows the canonical layout from tier-00 `docs/folder-layout.md` (src/lib.rs façade, domain/, adapters/ where applicable, errors.rs).
  - `cargo nextest run --workspace` passes (each crate has ≥1 placeholder test).
  - `cargo bench --workspace --no-run` builds (criterion benches scaffolded).
  - tier-00 `tests/architecture.rs` (was failing) now passes — `ariadne-core` has zero in-workspace deps; adapter crates depend only on `ariadne-core`.
  - ariadne-core exports stable IDs (`FileId`, `SymbolId`, `EdgeId`, `Span`, `Lang`) with proptest round-trip invariants AND port traits (`Storage`, `Parser`, `Indexer`, `WatcherSink`) as empty marker traits filled in by adapter tiers.
status: pending
---

<context>
Foundation-of-code tier. No domain logic yet. Establishes workspace shape and the hexagonal scaffolding declared in tier-00. CI/lint/release/governance are owned by tier-00 — this tier consumes them, does not redefine them.
See plan.md `<architecture>` for crate roles; tier-00 `docs/folder-layout.md` for the per-crate internal layout this tier instantiates.
</context>

<files>
- `Cargo.toml` (workspace root) — declares workspace members + shared `[workspace.dependencies]` + `[workspace.lints]` referencing tier-00 clippy.toml.
- `rust-toolchain.toml` — pin stable channel + components rustfmt, clippy, rust-src.
- `.editorconfig` — UTF-8, LF, 4-space Rust indent.
- `.gitignore` — append `target/`, `.ariadne/`, `*.profraw` (tier-00 may already have base entries).
- `.cargo/config.toml` — `lto = "thin"` for release; `incremental = true` for dev.
- `crates/ariadne-core/Cargo.toml` + `src/lib.rs` + `src/domain/{mod,types,ports}.rs` + `src/errors.rs` — domain-only crate (no `adapters/`).
- `crates/ariadne-{storage,parser,scip,graph,salsa,watcher,mcp,cli,e2e}/Cargo.toml` + `src/lib.rs` (or `main.rs` for cli) + `src/domain/` + `src/adapters/` empty stubs (storage/parser/scip/watcher only) + `src/errors.rs`.
- `crates/ariadne-core/tests/ids.rs` — proptest round-trip for ID encode/decode.
- `crates/ariadne-core/benches/ids.rs` — criterion sanity bench.
- (NOT in this tier — owned by tier-00: `.github/workflows/*`, `rustfmt.toml`, `clippy.toml`, `deny.toml`, `lefthook.yml`, `docs/`, `tests/architecture.rs`, `CONTRIBUTING.md`, ADRs, CODEOWNERS, PR template, dependabot.)
- `CLAUDE.md` (project-root) — add `<commands>` entry with build/test/lint pinned (rules-writer enforced) — note: tier-00 ships the configs, tier-01 records the commands.
</files>

<steps>
1. Create root `Cargo.toml` with `[workspace] members = ["crates/*"]` and `resolver = "2"` [src: https://doc.rust-lang.org/cargo/reference/workspaces.html].
2. Pin Rust toolchain: `rust-toolchain.toml` `channel = "stable"` (capture exact version observed at build time in the file, e.g. `1.83.0`) + `components = ["rustfmt", "clippy", "rust-src"]` [src: https://rust-lang.github.io/rustup/overrides.html].
3. Add `[workspace.dependencies]` entries (versions resolved at first `cargo update`; freeze in `Cargo.lock`): `thiserror`, `anyhow`, `tracing`, `tracing-subscriber`, `serde`, `serde_json`, `bincode` (CST serialization), `dashmap`, `parking_lot`, `proptest`, `insta`, `rstest`, `criterion`. Dev-only: `tempfile`, `pretty_assertions`.
4. Add lints profile in root `Cargo.toml`: `[workspace.lints.rust] unsafe_code = "forbid"` (except in tier-03 where `unsafe = "allow"` for tree-sitter FFI) + `[workspace.lints.clippy] pedantic = "warn"`.
5. Scaffold each crate via `cargo new --lib crates/ariadne-<name>` (ariadne-cli via `cargo new --bin`). Add to workspace.
6. In `ariadne-core/src/domain/types.rs` define:
   - `pub struct FileId(NonZeroU32);` interned via `string_interner` (or custom flat-index store; pin at impl time).
   - `pub struct SymbolId(NonZeroU64);` `pub struct EdgeId(NonZeroU64);`
   - `pub enum Lang { TypeScript, JavaScript, Python, Rust, Go, Java, Kotlin, CSharp, Other(&'static str) }`
   - `pub struct Span { file: FileId, byte_start: u32, byte_end: u32 }`
   - `pub trait IdEncode { fn to_bytes(&self) -> [u8; 8]; fn from_bytes(bytes: [u8; 8]) -> Option<Self>; }` (used by tier-02 redb codec).
   In `ariadne-core/src/domain/ports.rs` declare empty marker traits (real signatures filled by adapter tiers): `Storage`, `Parser`, `Indexer`, `WatcherSink`. These pin the hexagonal contract so tier-00 `tests/architecture.rs` invariant can verify cross-crate boundaries.
   `lib.rs` is façade-only: `pub use domain::*; pub use errors::*;` — no logic.
7. Write **failing tests first** in `crates/ariadne-core/tests/ids.rs` (proptest):
   - For any non-zero u64, `SymbolId::from_bytes(s.to_bytes()) == Some(s)`.
   - Span ordering total + transitive.
   Then implement until green.
8. Scaffold criterion bench in `crates/ariadne-core/benches/ids.rs` measuring 1M `to_bytes/from_bytes` round-trips; baseline reported, no gate yet.
9. cargo-deny / rustfmt / clippy configs and CI workflows are owned by tier-00 — this tier consumes them. Verify tier-00 files exist before proceeding; if missing, halt and re-run tier-00.
10. Update `CLAUDE.md` `<commands>` section via `/rules-writer` invocation (out-of-band) listing the pinned commands from tier-00 + the per-crate hexagonal layout rule.
11. Self-test: clone fresh, run `cargo nextest run --workspace` + `cargo test --test architecture` (root-level); verify tier-00 invariant flips from failing to passing now that the workspace exists.
</steps>

<verification>
- `cargo build --workspace` exits 0.
- `cargo nextest run --workspace` reports ≥10 tests passing (≥1 per crate; ariadne-core has proptest cases).
- `cargo clippy --workspace --all-targets -- -D warnings` clean.
- `cargo fmt --all --check` clean.
- `cargo deny check` clean.
- `cargo bench --workspace --no-run` builds criterion harness without compilation error.
- Expected: total wall-clock for the above on a 16-core dev machine <2min.
Failure modes that must be loud (no silencing): missing toolchain components, unresolved workspace deps, MSRV mismatch.
</verification>

<rollback>
Tier writes only new files. Rollback = `git rm -r crates/ Cargo.toml rust-toolchain.toml .cargo/` + `git checkout -- .gitignore CLAUDE.md`. Tier-00 files stay (they are independent foundations).
No state outside the repo.
</rollback>
