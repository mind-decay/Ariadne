---
tier_id: tier-05
title: Redesign the README — logo, badges, TOC, install matrix, commercial section
deps: [tier-01, tier-02, tier-04]
exit_criteria:
  - assets/ariadne-wordmark.svg committed and referenced at the top of README
  - README has a shields badge row and a GitHub-anchored table of contents
  - README install section covers Homebrew, shell, prebuilt archives (+ cargo note); PowerShell/Windows omitted to match the shipped dist config (see <deviations>)
  - README has distinct "License" and "Commercial licensing" sections
status: completed
completed: 2026-06-09
---

<context>
README content is complete but visually plain: no logo, badges, table of
contents, or hero [src: README.md:1-160]. Bring it to the reference design of a
serious library while preserving the existing accurate content (commands, tool
catalog, language matrix, troubleshooting). Depends on tier-01 (license text),
tier-02 (install channels + commands), and tier-04 (links to CONTRIBUTING / CoC /
SECURITY) [src: plan.md D7].
</context>

<files>
- `assets/ariadne-wordmark.svg` — create; text-based wordmark (Ariadne's-thread
  motif), no external fonts beyond a web-safe stack.
- `README.md` — rewrite structure; keep existing factual sections verbatim where
  still correct.
</files>

<steps>
1. Author `assets/ariadne-wordmark.svg`: a self-contained SVG (~520×120) with the
   wordmark "Ariadne" and a thin thread/labyrinth accent. Inline styles only, a
   web-safe `font-family` stack, no remote refs (GitHub serves README SVGs via a
   sanitizing proxy) [src: https://docs.github.com/en/get-started/writing-on-github].
2. Hero block at the top: centered `<p align="center">` with the SVG, a one-line
   tagline ("Local-first code intelligence for Claude"), and a badge row.
3. Badge row — license + MSRV are static shields:
   `https://img.shields.io/badge/license-PolyForm--NC--1.0.0-orange` and
   `https://img.shields.io/badge/rust-1.85%2B-blue` [src: https://shields.io/badges].
   For CI status and latest release, fetch the exact dynamic URLs from the
   shields GitHub category (`github/actions/workflow/status/...` and
   `github/v/release/...`) at build time and point them at `mind-decay/ariadne`
   [src: https://shields.io/badges]. Verify each badge image returns 200.
4. Insert a `## Table of Contents` with GitHub auto-anchor links (lowercase,
   spaces→hyphens, punctuation dropped) to every `##` section
   [src: https://docs.github.com/en/get-started/writing-on-github/getting-started-with-writing-and-formatting-on-github/basic-writing-and-formatting-syntax#section-links].
5. Sections in order: Why (value prop / advantages), Features (grid or table of
   what it answers), Install, Quickstart, Claude Code integration, Commands, Tool
   catalog, Language support, Troubleshooting, Architecture, Contributing,
   Commercial licensing, License, Acknowledgements. Reuse the existing accurate
   tables for Commands / Tool catalog / Language support [src: README.md:70-147].
6. Install section = a channel matrix. Homebrew:
   `brew install mind-decay/homebrew-tap/ariadne`. Shell and PowerShell: copy the
   exact installer URLs `dist` prints (filenames like `ariadne-installer.sh` /
   `.ps1` from the Release `latest/download/` path) — do not guess filenames,
   take them from `dist plan` or the Release page [src: tier-02; cargo-dist
   installer output]. Archives: link to the Releases page. Add a `cargo install`
   line marked "available after the crates.io publish (tier-06)".
7. Commercial licensing section: noncommercial use is free under PolyForm NC;
   commercial use requires a paid license — link `LICENSE-COMMERCIAL.md` and the
   contact [src: plan.md D2]. License section: state PolyForm Noncommercial 1.0.0,
   link `LICENSE.md`.
</steps>

<verification>
- `test -f assets/ariadne-wordmark.svg`; opening it renders the wordmark (no
  broken refs); README references it with a relative path.
- Render README locally (e.g. `grip` or GitHub preview): logo shows, every badge
  image loads (HTTP 200), TOC links jump to their sections.
- `grep -E '## (Install|License|Commercial licensing)' README.md` matches all three.
- Install block contains `brew install`, a `curl ... | sh` line, and a Releases
  link. (No `irm ... | iex` line — PowerShell is not a shipped installer; see
  <deviations>.)
- No stale `MIT OR Apache` text remains (`grep -i 'mit or apache' README.md` empty).
</verification>

<rollback>
`git checkout -- README.md` and `rm assets/ariadne-wordmark.svg` (remove `assets/`
if now empty). Docs-only change; no build impact.
</rollback>

<deviations>
- PowerShell / Windows install path dropped (user-approved 2026-06-09). This tier
  was authored assuming a Windows release + PowerShell installer, but the
  completed tier-02/tier-03 deliberately removed both from `dist-workspace.toml`
  ("Ariadne is not Windows-ready yet"). `dist plan` emits only `shell` + `homebrew`
  installers for macOS/Linux, so an `irm | iex` line would advertise a 404 URL —
  forbidden by step 6 ("do not guess filenames"). The install matrix instead
  covers Homebrew, shell (`curl|sh`), prebuilt archives (macOS/Linux), a
  crates.io `cargo install` note (tier-06), and a from-source fallback. The stale
  "Windows" prebuilt-binary claim in the old README was corrected to match.
- Homebrew formula name is `ariadne-cli` (from `dist plan`: `ariadne-cli.rb`), per
  plan.md verification line 161 — not the `ariadne` of this tier's step 6 draft.
</deviations>
</content>
