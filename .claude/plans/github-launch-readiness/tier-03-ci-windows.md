---
tier_id: tier-03
title: Test the Windows release target in CI (clippy + nextest on windows-latest)
deps: []
exit_criteria:
  # Revised by owner decision (see <blockers>): adding windows-latest revealed
  # Ariadne is not Windows-ready (daemon IPC unix-socket-only; non-portable
  # paths — 102 nextest failures). Windows is build/clippy-guarded only, not
  # shipped, until a dedicated port lands.
  - ci.yml clippy matrix includes windows-latest; nextest matrix does not
  - clippy (windows-latest) passes on a PR; unix nextest/clippy stay green
  - release no longer builds x86_64-pc-windows-msvc (dist-workspace.toml)
status: completed
completed: 2026-06-09
---

<context>
The release pipeline builds `x86_64-pc-windows-msvc` [src: dist-workspace.toml:20]
but CI only runs clippy and tests on `ubuntu-latest` + `macos-latest`
[src: .github/workflows/ci.yml:38-39, 54-55]. A Windows-only break (path
handling, a C-compiled tree-sitter grammar via `cc`, vendored `protoc`) would
ship untested. Add Windows to the two matrices so breakage is caught before a
tag [src: plan.md D5, R3]. Independent of all other tiers.
</context>

<files>
- `.github/workflows/ci.yml` — modify; add `windows-latest` to the `clippy` and
  `test` job `matrix.os` arrays.
- `crates/**` — modify only if a test fails on Windows for a genuinely
  platform-specific reason (cfg-gate, do not delete or weaken).
</files>

<steps>
1. In the `clippy` job, change `matrix.os` to
   `[ubuntu-latest, macos-latest, windows-latest]` [src: ci.yml:38-39]. The
   `dtolnay/rust-toolchain@stable` + `Swatinem/rust-cache` + clippy steps are
   already cross-platform.
2. In the `test` job, change `matrix.os` to
   `[ubuntu-latest, macos-latest, windows-latest]` [src: ci.yml:54-55].
   `taiki-e/install-action@nextest` provides nextest on Windows.
3. Leave `fmt`, `deny`, `audit`, `docs`, `bench-build`, `msrv`,
   `arch-invariants`, `commits`, `pr-title` unchanged — they are platform-agnostic
   or unix-only by design (e.g. msrv build) and need no Windows runner.
4. Run the workspace on Windows (CI PR run, or local `cargo nextest run
   --workspace` on a Windows host if available). Root-cause every failure. If a
   test asserts unix-only behavior (file modes, `/`-only paths), gate it with
   `#[cfg(unix)]` and a comment naming why; never silence with skips, `--no-fail-fast`
   masking, or weakened asserts [src: CLAUDE.md `<rules>` "Validate by execution"].
</steps>

<verification>
- `grep -n windows-latest .github/workflows/ci.yml` shows it in both `clippy` and
  `test` matrices.
- A pushed PR shows green `clippy (windows-latest)` and `nextest (windows-latest)`
  checks.
- `cargo nextest run --workspace --profile ci` is green on Windows (or the PR
  check stands in for a local Windows host).
- Any added `#[cfg(unix)]` carries a justifying comment; no test was deleted or
  its assertions weakened (diff review).
</verification>

<rollback>
`git checkout -- .github/workflows/ci.yml` and revert any cfg-gates. Removing the
Windows matrix entries restores the prior 2-OS CI with no other impact.
</rollback>

<blockers>
Owner added `origin` mid-build; real push CI runs exercised the matrix. The key
outcome: adding `windows-latest` proved **Ariadne is not Windows-ready**, so the
owner chose to guard Windows compilation only (clippy) and **not ship a Windows
release** until a dedicated port lands. Stays `blocked` until the next run shows
green clippy (windows) + green unix nextest/clippy + Windows-free release.

Windows breaks surfaced + handled:
- COMPILE (clippy + nextest): `E0599 enable_io` (tokio I/O driver; on unix the
  `signal` feature pulls it in, Windows does not) → added tokio `net` to
  ariadne-cli [src: docs.rs/tokio Builder::enable_io]. Then `clippy (windows)`:
  `unnecessary_wraps` on the `#[cfg(not(unix))]` `set_executable` stub →
  `#[allow]` with a signature-parity justification.
- RUNTIME (nextest windows): 102 failures in 3 buckets — daemon IPC is
  unix-domain-socket-only (`not a named pipe path`; ~30 daemon/warm/mcp/e2e
  tests), and cold-index/parser goldens diverge on `\` paths (index_parity ×12,
  parser facts ×16). These are real product gaps, not unix-only tests to gate.
  Owner decision: drop `windows-latest` from the nextest matrix; remove
  `x86_64-pc-windows-msvc` + the `powershell` installer from dist-workspace.toml
  (release.yml regenerated via `dist generate`, `--check` clean). A Windows port
  (named-pipe IPC, path/line-ending normalization) is its own future plan.

Authorized prerequisite: `.config/nextest.toml` `[profile.ci]` (was undefined;
`error: profile 'ci' not found`).

Pre-existing failures the first-ever CI run exposed (CI never ran without a
remote), fixed with owner approval — outside this tier's original scope:
- `docs`: rustdoc broken-intra-doc-links from the `[src: … <…> …]` citation
  pattern (mcp co_change/hotspots/read_outline, graph/fitness, salsa/derive,
  cli/outline) — escaped the brackets; `cargo doc --workspace -D warnings` green.
- `commits`: dead action `oknozor/setup-cocogitto@v1` (404). cocogitto-action
  installs cog 6.x, which can't parse this repo's v7 `cog.toml`, so install the
  cog 7.0.0 release binary directly + raw `cog check` (full history; no tags yet).
- `nextest (unix)`: wall-clock SLO `read_outline_p95_under_100ms` exceeds 100ms
  on shared CI runners → `#[ignore]` (matching slo.rs), assertion intact.
  `cargo nextest run --workspace --profile ci` green on macOS (576 passed).
</blockers>
</content>
