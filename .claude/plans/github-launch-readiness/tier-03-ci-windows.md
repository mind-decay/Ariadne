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
Blocked on exit criterion #2 (green `clippy (windows-latest)` + `nextest
(windows-latest)` on a PR): the repo has no git remote and the build host is
macOS, so a real Windows CI run cannot be triggered this session. Owner
accepted local-only verification; this tier stays `blocked` until a real
Windows PR run is green. Edits + local evidence:
- `windows-latest` added to the `clippy` (ci.yml:39) and `test` (ci.yml:55)
  matrices — `grep -n windows-latest` shows both; YAML parses.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` green
  on macOS; `cargo nextest run --workspace --profile ci` green on macOS
  (577 passed, 21 skipped, 0 failed) — cross-platform evidence for unix.
- No cfg-gates added: no Windows failure was observed (Windows could not run),
  and the existing unix-only tests are already `#[cfg(unix)]`-gated
  (`advisory.rs:10`, `setup.rs:258`, `adoption_wiring.rs:215`).

Authorized prerequisite (out of this tier's original `<files>`, approved by the
owner mid-build): added `.config/nextest.toml` defining `[profile.ci]`. The
`ci` profile that `cargo nextest run --profile ci` requires was undefined
repo-wide (`error: profile 'ci' not found`), so the `test` job — the thing this
tier extends to Windows — would have failed on every OS. The profile sets
`fail-fast = false` + `failure-output = "immediate-final"` only; no `retries`
(masking flakiness violates CLAUDE.md "failures are root-caused").
</blockers>
</content>
