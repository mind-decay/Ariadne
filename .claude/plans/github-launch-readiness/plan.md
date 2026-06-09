---
slug: github-launch-readiness
title: Public GitHub launch — licensing, cross-platform releases, install UX, README
created: 2026-06-09
owners: [user, claude]
review: [user, codex]
single_tier: false
tiers:
  - tier-01-licensing-polyform
  - tier-02-release-installers
  - tier-03-ci-windows
  - tier-04-community-health
  - tier-05-readme-redesign
  - tier-06-crates-io-publish
---

<context>
Ariadne is ready to go public on GitHub (`mind-decay/ariadne`, the URL already
pinned in `Cargo.toml repository` [src: Cargo.toml:24]). The code, CI, and a
cargo-dist release pipeline exist; the launch-readiness gaps are: (1) the
declared license (`MIT OR Apache-2.0`) is the opposite of the owner's intent —
the project must be source-available but not commercially usable without the
owner's paid consent; (2) releases ship raw archives only, no convenient
installer; (3) the Windows release target is never tested in CI; (4) no
LICENSE / CONTRIBUTING / SECURITY / issue-form files exist; (5) the README has
complete content but plain design (no badges, no table of contents, no hero).

Goal (measurable): a tagged `vX.Y.Z` push produces GitHub-hosted macOS
(arm64+x64) and Windows (x64) artifacts installable via one command (`brew
install`, `curl|sh`, or `irm|iex`); the repo carries a PolyForm Noncommercial
license + standard community-health files; and the README renders with a logo,
badge row, anchored table of contents, and a commercial-licensing section.

Out of scope: changing analytics/behavior; a marketing site; Linux packaging
beyond the existing archive/installer; GitHub Sponsors/FUNDING; scoop/winget
(owner deselected). crates.io publishing is included but isolated to the final
optional tier because of its cost (see D6, R2).
</context>

<constraints>
- License intent overrides the current manifest: source-available, no
  commercial use without the owner's consent and benefit. OSI "open source"
  (MIT/Apache) cannot forbid commercial use, so an OSI license is disqualified
  [src: https://opensource.org/osd — clause 6, "No Discrimination Against
  Fields of Endeavor"].
- Release tooling is fixed at the installed `dist` (cargo-dist) v0.31.0; extend
  its config, do not replace the pipeline [src: dist-workspace.toml:6;
  .github/workflows/release.yml:1-3].
- Single shipped binary: `ariadne` from `ariadne-cli` (the only package with
  `[package.metadata.dist] dist = true`); `ariadne-mcp` builds a bin but is not
  dist-able [src: crates/ariadne-cli/Cargo.toml:15-23].
- Commit + PR rules unchanged: Conventional Commits, scope required, enforced by
  `cog` + `pr-title` [src: .github/workflows/ci.yml:133-192; cog.toml].
- ≤200 lines per authored file; each tier independently buildable in a fresh
  `spec-build` session [src: CLAUDE.md `<rules>`].
- No new runtime tech on the critical path; this work touches packaging,
  metadata, CI, and docs only.
</constraints>

<decisions>
- D1 — License = PolyForm Noncommercial 1.0.0 (SPDX `PolyForm-Noncommercial-1.0.0`).
  Any noncommercial purpose (personal, research, education, nonprofits,
  government) is free; every commercial use requires a separate commercial
  license from the owner — the exact "open but my benefit" model
  [src: https://polyformproject.org/licenses/noncommercial/1.0.0;
  https://spdx.org/licenses/PolyForm-Noncommercial-1.0.0]. Rejected: Business
  Source License 1.1 — auto-converts to OSS after ≤4y and needs a production
  carve-out, weaker "my benefit" [src: https://mariadb.com/bsl11/]; Functional
  Source License — allows free internal commercial use, blocks only competitors
  [src: https://fsl.software/]; MIT/Apache — permit unrestricted commercial use,
  contradicting intent.
- D2 — Dual-license posture: PolyForm NC is the public license; commercial use
  is offered under a separate paid grant documented in `LICENSE-COMMERCIAL.md` +
  a README "Commercial licensing" section. PolyForm explicitly contemplates this
  split [src: D1 PolyForm text, "noncommercial ... permitted purpose"].
- D3 — Installers = `shell` + `powershell` + `homebrew` (owner's pick), plus the
  existing GitHub archives. `dist` generates curl|sh, irm|iex, and a Homebrew
  formula pushed to a tap [src:
  https://raw.githubusercontent.com/axodotdev/cargo-dist/main/book/src/reference/config.md].
  Rejected scoop/winget (deselected).
- D4 — Homebrew tap = a new `mind-decay/homebrew-tap` repo; `dist` publishes the
  formula there via a `repo`-scoped PAT stored as secret `HOMEBREW_TAP_TOKEN`
  [src: https://raw.githubusercontent.com/axodotdev/cargo-dist/main/book/src/installers/homebrew.md].
- D5 — Add `windows-latest` to the CI clippy+test matrix. The release builds
  `x86_64-pc-windows-msvc` but CI only runs ubuntu+macos, so Windows breakage
  ships untested [src: .github/workflows/ci.yml:38-55; dist-workspace.toml
  targets].
- D6 — crates.io publish is its own final, optional tier. The bare name
  `ariadne` is taken (8.1M downloads) [src: https://crates.io/api/v1/crates/ariadne],
  and publishing the CLI forces publishing the entire `ariadne-*` dependency
  tree first, because crates.io forbids dependencies on code outside the
  registry [src: https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html].
- D7 — README gets a committed text-based SVG wordmark in `assets/`, a
  shields.io badge row, and a GitHub-anchored table of contents [src:
  https://shields.io/badges]. No external designer; SVG authored in-repo.
- D8 — Each launch concern is a separate tier so a fresh session can build and
  verify one milestone without the others (license ⊥ CI ⊥ docs).
</decisions>

<architecture>
Files-and-config change, no code paths. Components touched:
- Licensing surface: `Cargo.toml [workspace.package] license`, `LICENSE.md`,
  `LICENSE-COMMERCIAL.md`, `docs/adr/0033-licensing-model.md`.
- Release surface: `dist-workspace.toml` (installers, tap), regenerated
  `.github/workflows/release.yml`, external `mind-decay/homebrew-tap` repo +
  `HOMEBREW_TAP_TOKEN` secret.
- CI surface: `.github/workflows/ci.yml` matrix.
- Community surface: `CONTRIBUTING.md`, `CODE_OF_CONDUCT.md`, `SECURITY.md`,
  `.github/ISSUE_TEMPLATE/*.yml`, `CHANGELOG.md`.
- Docs surface: `README.md`, `assets/ariadne-wordmark.svg`.
Dependency DAG: 01 → {02, 03, 04} → 05; 06 depends on 01 only (run last).
</architecture>

<tech_inventory>
| Tech | Version | Doc fetched this session |
|---|---|---|
| PolyForm Noncommercial | 1.0.0 | polyformproject.org/licenses/noncommercial/1.0.0 |
| cargo-dist (`dist`) | 0.31.0 | raw.githubusercontent.com/axodotdev/cargo-dist/main/book/src/reference/config.md |
| cargo-dist Homebrew | 0.31.0 | …/book/src/installers/homebrew.md |
| crates.io dep rule | current | doc.rust-lang.org/cargo/reference/specifying-dependencies.html |
| GitHub issue forms | current | docs.github.com/.../syntax-for-issue-forms |
| Contributor Covenant | 2.1 | contributor-covenant.org/version/2/1/code_of_conduct/code_of_conduct.md |
| shields.io | current | shields.io/badges |
| cocogitto (changelog) | v7+ | cog.toml; docs.cocogitto.io |
</tech_inventory>

<risks>
- R1 — A non-OSI license deters contributors/adoption. Likelihood: medium.
  Mitigation: clear README "License" + "Commercial licensing" sections stating
  noncommercial use is free; the choice is the owner's explicit business intent
  (D1). Owner.
- R2 — crates.io publish requires publishing ~12 interdependent crates under
  available names + a package rename for the CLI; high effort and ongoing
  per-release burden. Likelihood: high. Mitigation: isolate in tier-06, gate on
  a name-availability check, recommend deferring (D6, R2). Owner/Claude.
- R3 — Windows MSVC build may surface tree-sitter `cc` / `protoc-bin-vendored`
  issues not seen on unix. Likelihood: medium. Mitigation: tier-03 makes the CI
  failure visible before tagging a release. Claude.
- R4 — Homebrew tap publish needs an out-of-band repo + PAT secret a script
  cannot create. Likelihood: high (manual). Mitigation: tier-02 documents the
  steps and verifies via `dist plan`; formula publish proven only on a real tag.
  Owner.
- R5 — crates.io may reject or flag a noncommercial (non-OSI) license. Mitigation:
  tier-06 verifies SPDX acceptance with `cargo publish --dry-run` before any real
  publish. Claude.
</risks>

<verification>
- License: `cargo build --workspace` green with the new SPDX `license`; `LICENSE.md`
  byte-matches the PolyForm canonical text; ADR-0033 committed.
- Release: `dist plan` lists shell, powershell, and homebrew installer artifacts
  for macOS (arm64+x64) and Windows (x64); regenerated `release.yml` passes
  `dist generate --check`.
- CI: a PR run shows green `clippy (windows-latest)` and `nextest (windows-latest)`.
- Community: GitHub renders the issue-form chooser, CODE_OF_CONDUCT, SECURITY,
  CONTRIBUTING; `cog changelog` emits a populated `CHANGELOG.md`.
- README: TOC anchors resolve, badge row renders, logo SVG displays, install
  matrix + commercial-licensing section present.
- End-to-end (owner-run): push a `v*` tag → GitHub Release carries installers;
  `brew install mind-decay/homebrew-tap/ariadne-cli` and the curl|sh / irm|iex
  one-liners install a working `ariadne` on macOS and Windows.
</verification>

<sources>
- [ADR-0033 licensing model](../../../docs/adr/0033-licensing-model.md)
- [PolyForm Noncommercial 1.0.0](https://polyformproject.org/licenses/noncommercial/1.0.0)
- [SPDX PolyForm-Noncommercial-1.0.0](https://spdx.org/licenses/PolyForm-Noncommercial-1.0.0)
- [Business Source License 1.1](https://mariadb.com/bsl11/) · [Functional Source License](https://fsl.software/)
- [cargo-dist config reference](https://raw.githubusercontent.com/axodotdev/cargo-dist/main/book/src/reference/config.md)
- [cargo-dist Homebrew installer](https://raw.githubusercontent.com/axodotdev/cargo-dist/main/book/src/installers/homebrew.md)
- [Cargo specifying dependencies](https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html)
- [GitHub issue forms syntax](https://docs.github.com/en/communities/using-templates-to-encourage-useful-issues-and-pull-requests/syntax-for-issue-forms)
- [Contributor Covenant 2.1](https://www.contributor-covenant.org/version/2/1/code_of_conduct/code_of_conduct.md)
- [shields.io badges](https://shields.io/badges)
</sources>
</content>
</invoke>
