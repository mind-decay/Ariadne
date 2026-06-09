---
tier_id: tier-03
audited: 2026-06-09
verdict: PASS
commit: 07432fc660ea9119eb37a043da83ff5919f08e3c
---

<scope>
Tier-03 ("Test the Windows release target in CI"). Diff range `4f5855d..HEAD`,
scoped to the tier's `<files>` (`.github/workflows/ci.yml`, `crates/**`) plus
the `<blockers>`-authorized expansion (`.config/nextest.toml`,
`dist-workspace.toml`, regenerated `.github/workflows/release.yml`, rustdoc
bracket-escape fixes, `commits`-job cog install, wall-clock SLO `#[ignore]`,
tokio `net` feature). The owner revised the original "windows on both matrices"
goal mid-build: adding `windows-latest` proved Ariadne is not Windows-ready
(daemon IPC is unix-socket-only; non-portable paths → 102 nextest failures), so
Windows is now clippy/build-guarded only and is **not shipped**. The revised
`exit_criteria` + `<blockers>` are the binding contract.

The commit range also carries unrelated data-fidelity-arc economy work swept in
by the bundle commit `acbca87` (ADR-0029, `economy_token_delta.rs`, server.rs
economy doc + handshake snapshot); that work is out of tier-03 scope and is
covered by its own audit trail (data-fidelity-arc block-1). Reviewed for
interference only.
</scope>

<checks_run>
- `grep -n windows-latest .github/workflows/ci.yml` → line 39 (clippy matrix);
  line 55 is a comment in the test job, not a matrix entry. Matches the revised
  exit_criteria ("clippy includes windows-latest; nextest does not").
- `dist-workspace.toml`: `x86_64-pc-windows-msvc` removed from `targets`;
  `powershell` dropped from `installers`. Comments cite the tier-03 finding.
- `dist generate --check` → exit 0 (release.yml in sync, no Windows build job;
  `grep -ni windows release.yml` finds only the benign `longpaths`/comment lines
  from the unrelated homebrew block).
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` → exit 0.
- `cargo fmt --all --check` → exit 0.
- `RUSTDOCFLAGS=-D warnings cargo doc --workspace --no-deps --document-private-items`
  → exit 0 (the `\[src: …\]` bracket-escape fix in co_change/hotspots/read_outline/
  fitness/derive/outline clears the broken-intra-doc-link errors).
- `cargo nextest run --workspace --profile ci` → 576 passed, 0 failed, 22 skipped
  in 85s; the `[profile.ci]` in `.config/nextest.toml` resolves (no
  "profile 'ci' not found").
- `cog check` (full history) → "No errored commits".
- tokio `net` fix traced to real `Builder::enable_io()` call sites
  (serve.rs:29, watch.rs:56); E0599-on-Windows reasoning is sound.
- `set_executable` `#[cfg(not(unix))]` stub carries `#[allow(clippy::unnecessary_wraps)]`
  + signature-parity justification (setup.rs:421-425).
- No test deleted or weakened. Sole test change is the wall-clock SLO `#[ignore]`
  (tools_read_outline.rs:425), assertion intact, matching the precedent in
  `crates/ariadne-e2e/tests/slo.rs:57`.
- Working tree clean (`git status --short` empty); tier fully committed.
</checks_run>

<findings>
| id | category | severity | location | problem | fix |
|---|---|---|---|---|---|
| F1 | docs | INFO | tier-03-ci-windows.md:34-39,52-57 | `<steps>` 1-2 and `<verification>` 1-2 still say "add windows-latest to the test matrix" and "PR shows green nextest (windows-latest)", contradicting the owner-revised `exit_criteria`/`<blockers>` that deliberately exclude Windows from nextest. | Update the steps/verification prose to match the revised decision (clippy-only Windows; nextest unix-only). |
| F2 | plan_adherence | INFO | commit acbca87 | The tier's enabling commit bundles unrelated data-fidelity-arc economy work (ADR-0029, economy_token_delta.rs +464, server.rs economy doc, handshake snapshot) into the tier-03 range, muddying blast-radius review. | Cosmetic only; the economy work is green and separately audited. Prefer one concern per commit in future bundles. |
</findings>

<verdict>
PASS. Zero FAIL findings. Every runnable verification is green on the unix host:
clippy, fmt, rustdoc (-D warnings), nextest `--profile ci` (576/0), cog check,
and `dist generate --check`. The static config satisfies all three revised
exit_criteria: clippy matrix gains `windows-latest`, the nextest matrix does
not, and `dist-workspace.toml` no longer targets `x86_64-pc-windows-msvc`. The
Windows-not-ready pivot was handled honestly — Windows dropped wholesale from
nextest/release rather than tests silenced or assertions weakened; the lone
`#[ignore]` is a precedented wall-clock SLO with its assertion intact.

Verification gap (not a defect): exit_criterion 2's "clippy (windows-latest)
passes on a PR" cannot be reproduced on this darwin host (no Windows runner; `gh`
absent). It rests on the owner's reported real push-CI runs (`<blockers>`). The
unix half of that criterion ("unix nextest/clippy stay green") is fully verified
here.
</verdict>

<next_steps>
None blocking. Optional cleanup: reconcile F1 (stale steps/verification prose)
on the next edit of the tier file so a future reader is not misled into
expecting a Windows nextest check. F2 needs no action.
</next_steps>

<sources>
- [tier-03-ci-windows.md](../tier-03-ci-windows.md) — revised exit_criteria + blockers
- [nextest configuration](https://nexte.st/docs/configuration/) — `[profile.ci]` inheritance
- [cargo-dist config reference](https://raw.githubusercontent.com/axodotdev/cargo-dist/main/book/src/reference/config.md) — targets/installers, `dist generate --check`
- crates/ariadne-cli/src/commands/{serve,watch}.rs — `enable_io()` call sites (tokio `net`)
- crates/ariadne-e2e/tests/slo.rs:57 — `#[ignore]` SLO precedent
</sources>
