---
tier_id: tier-03
title: Test the Windows release target in CI (clippy + nextest on windows-latest)
deps: []
exit_criteria:
  - ci.yml clippy and test matrices include windows-latest
  - clippy (windows-latest) and nextest (windows-latest) jobs pass on a PR
  - any genuinely unix-only test is cfg-gated with a one-line justification
status: blocked
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
Owner added the `origin` remote mid-build; the matrix was exercised by a real
push CI run (commit `acbca87`). `windows-latest` is in both matrices (ci.yml:39,
55). Stays `blocked` until a re-run shows green Windows jobs.

Tier-03 finding (the genuine Windows break this tier exists to catch):
- `clippy (windows-latest)` + `nextest (windows-latest)` failed with
  `error[E0599]: no method named enable_io ... tokio::runtime::Builder`
  (ariadne-cli serve.rs:29, watch.rs:56). `enable_io()` needs the I/O driver,
  which on unix the `signal` feature pulls in but on Windows it does not
  [src: docs.rs/tokio Builder::enable_io — "feature `net`, or Unix + `signal`…"].
  Fix: added the `net` feature to ariadne-cli's tokio dep — not a cfg-gate;
  the code is meant to run on Windows. `cargo check -p ariadne-cli` green on
  unix; Windows confirmation pends the re-run.

Authorized prerequisite: `.config/nextest.toml` `[profile.ci]` (the profile was
undefined repo-wide; `error: profile 'ci' not found`).

Unrelated pre-existing failures the first-ever CI run exposed (no remote before,
so CI never ran), fixed with owner approval — outside this tier's scope:
- `docs`: rustdoc broken-intra-doc-links from the `[src: … <…> …]` citation
  pattern (mcp co_change/hotspots/read_outline, graph/fitness, salsa/derive,
  cli/outline) — escaped the brackets; `cargo doc --workspace -D warnings` green.
- `commits`: dead action `oknozor/setup-cocogitto@v1` (404) → `cocogitto/
  cocogitto-action@v4`; push uses full-history `cog check` (no release tags yet).
- `nextest (unix)`: wall-clock SLO `read_outline_p95_under_100ms` exceeds 100ms
  on shared CI runners → `#[ignore]` (matching slo.rs), assertion intact.
  `cargo nextest run --workspace --profile ci` green on macOS (576 passed).
</blockers>
</content>
