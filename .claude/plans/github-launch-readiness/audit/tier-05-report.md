---
tier_id: tier-05
audited: 2026-06-09
verdict: PASS
commit: 0030156c122ab45fe9c2ec6ac746d39a2f668ad1
---

<scope>
Tier-05 "Redesign the README — logo, badges, TOC, install matrix, commercial
section". Scoped diff: two files, both in the tier's `<files>`:
- `assets/ariadne-wordmark.svg` — new, self-contained text wordmark.
- `README.md` — full structural rewrite preserving the factual tables.

The `M` on the tier file itself is the spec-build status/deviation record
(`status: completed`, `<deviations>` block) — expected, not a code deliverable.
No file outside `<files>` is touched. Index fresh at revision 1903 (419 files,
4183 symbols, 12720 edges) — graph trusted for cross-checks.
</scope>

<checks_run>
- Re-ran every `<verification>` command in the tier file:
  - `test -f assets/ariadne-wordmark.svg` → EXISTS.
  - `grep -E '## (Install|License|Commercial licensing)'` → all three match
    (README.md:141, 350, 358).
  - Install block contains `brew install` (148), `curl … | sh` (154), Releases
    link (160); `irm … | iex` ABSENT (matches the approved `<deviations>`).
  - `grep -i 'mit or apache'` → empty.
  - README references the SVG by relative path `assets/ariadne-wordmark.svg`
    (README.md:2).
- Badge URLs: all four return HTTP 200 (`curl -o /dev/null -w %{http_code}`),
  including the two dynamic shields badges for the pre-launch repo (shields
  renders placeholder state as a 200 SVG) [src: https://shields.io/badges].
- TOC ↔ headings: all 16 TOC anchors map to a real `## ` heading; GitHub
  auto-anchor rules (lowercase, spaces→hyphens, punctuation dropped) verified by
  hand against `grep '^## '` output [src:
  https://docs.github.com/en/get-started/writing-on-github/getting-started-with-writing-and-formatting-on-github/basic-writing-and-formatting-syntax#section-links].
- Install matrix vs shipped `dist` config (`dist-workspace.toml`): installers =
  `shell` + `homebrew` (no powershell), targets = macOS arm64/x64 + Linux
  arm64/x64 (no Windows), tap = `mind-decay/homebrew-tap`. README's channel
  matrix, "Windows is not yet a published release target", formula name
  `ariadne-cli`, and installer filename `ariadne-cli-installer.sh` all match the
  cargo-dist naming convention (package `ariadne-cli`, cf. release.yml:67's own
  `cargo-dist-installer.sh`).
- Tool catalog count: catalog table lists exactly 23 tools; cross-checked
  against the 23 `mcp__ariadne__*` tools. "all 23" claim accurate.
- Language count: `Lang` enum (crates/ariadne-core/src/domain/types/lang.rs:17)
  has 14 concrete variants (excluding `Other`); README's "14 out of the box" and
  the enumerated list match.
- Commands table: each row cross-checked against the `Cmd` clap enum
  (crates/ariadne-cli/src/main.rs:31-176). 13 of 15 rows accurate; 2 usage
  strings wrong (see findings).
- Performance claims: "~0.6 ms per single-token change on a 10 MB file" matches
  ADR-0005 ("incremental hot-path measured ~0.6 ms", jQuery-replicated 10 MB
  fixture). Repo-size figures (419 files / 4.2K symbols / 12.7K edges) match
  `project_status`.
- `DaemonClient` example path: `find_definition` confirms the struct lives at
  `crates/ariadne-mcp/src/adapters/daemon_client.rs` — the README example path
  is correct.
- Relative-link integrity: all 12 README link targets exist on disk (LICENSE.md,
  LICENSE-COMMERCIAL.md, CONTRIBUTING.md, CODE_OF_CONDUCT.md, SECURITY.md,
  docs/architecture.md, docs/folder-layout.md, docs/codebase-overview.md,
  docs/adr/, ADR-0005, plan.md, the SVG).
- SVG self-containment: inline `style=` attributes only (no `<style>` block),
  web-safe `font-family` stack, inline `<linearGradient>`, no remote refs —
  satisfies step 1 and GitHub's SVG sanitizer constraints [src:
  https://docs.github.com/en/get-started/writing-on-github].
</checks_run>

<findings>
| id | category | severity | location | problem | fix |
|---|---|---|---|---|---|
| F1 | correctness/docs | INFO | README.md:232 | `ariadne api-diff <base> <head>` shows two positional args, but `Cmd::ApiDiff` takes a single `spec` positional that `parse_spec` requires to be `<base>..<head>` (split on `..`); a two-arg invocation errors with "unexpected argument" [src: crates/ariadne-cli/src/main.rs:104-110; src/commands/api_diff.rs:39-46]. | Document as `ariadne api-diff <base>..<head>`. |
| F2 | correctness/docs | INFO | README.md:233 | `ariadne fitness` is shown without a subcommand, but `Cmd::Fitness` has a required non-`Option` `#[command(subcommand)]` (`FitnessAction::Check`), so the bare invocation errors "requires a subcommand"; cf. the `daemon` row which correctly shows `<start\|stop\|status>` [src: crates/ariadne-cli/src/main.rs:111-116, 247-249]. | Document as `ariadne fitness check`. |
</findings>

<verdict>
PASS. Zero FAIL findings. All four `<exit_criteria>` are independently
satisfied: the SVG wordmark is present and referenced at the top of the README;
the README carries a four-badge shields row and a GitHub-anchored table of
contents whose 16 anchors all resolve; the install section is a channel matrix
covering Homebrew, shell, prebuilt archives, and a `cargo install` note, with
PowerShell/Windows omitted exactly as the approved `<deviations>` require and the
shipped `dist-workspace.toml` dictates; and the README has distinct "License" and
"Commercial licensing" sections. Every `<verification>` command re-ran green. The
two INFO findings are real but non-blocking documentation usage-string errors in
the (newly expanded) Commands table; they touch neither an exit criterion nor a
non-negotiable and do not gate the verdict.
</verdict>

<next_steps>
None block the commit. Optional follow-up (non-gating): correct the two Commands
table usage strings per F1 and F2 in a docs touch-up — `ariadne api-diff
<base>..<head>` and `ariadne fitness check`. These can fold into tier-06 or a
standalone docs commit; no re-build of tier-05 is required.
</next_steps>

<sources>
- [shields.io badges](https://shields.io/badges)
- [GitHub section links / auto-anchors](https://docs.github.com/en/get-started/writing-on-github/getting-started-with-writing-and-formatting-on-github/basic-writing-and-formatting-syntax#section-links)
- [GitHub writing & SVG rendering](https://docs.github.com/en/get-started/writing-on-github)
- [ADR-0005 parse-SLO baseline](../../../../docs/adr/0005-tier-03-parse-slo-baseline.md)
- Repo code: crates/ariadne-cli/src/main.rs, crates/ariadne-cli/src/commands/api_diff.rs, crates/ariadne-core/src/domain/types/lang.rs, dist-workspace.toml, .github/workflows/release.yml
</sources>
</content>
