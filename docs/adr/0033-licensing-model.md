# ADR-0033: Licensing model — PolyForm Noncommercial with a commercial grant

<status>
Accepted
Date: 2026-06-09
Decider: user
</status>

<context>
Ariadne is preparing a public GitHub launch. The workspace manifest declared
`MIT OR Apache-2.0` [src: Cargo.toml `[workspace.package] license`], which permits
unrestricted commercial use — the opposite of the owner's intent. The owner wants
the project to be **source-available** and free for any noncommercial purpose, but
**not commercially usable without the owner's consent and benefit**.

The OSI Open Source Definition forbids discrimination against fields of endeavor,
so no OSI-approved "open source" license can bar commercial use [src:
https://opensource.org/osd — clause 6]. The license therefore must be a
source-available, non-OSI license. The choice is foundational: release metadata,
the Homebrew formula, a future crates.io publish, and the README all read this
license [src: .claude/plans/github-launch-readiness/plan.md `<context>`].

The fixed architectural lenses are scalability, reliability, efficiency, and
maintainability [src: ../../CLAUDE.md `<rules>`]; for a licensing decision the
governing lens is maintainability — a single, unambiguous, SPDX-recognized
identifier that downstream tooling (cargo, cargo-deny, crates.io) accepts without
special-casing.
</context>

<decision>
License the public distribution under **PolyForm Noncommercial License 1.0.0**
(SPDX `PolyForm-Noncommercial-1.0.0`) and offer **commercial use under a separate
paid grant** documented in `LICENSE-COMMERCIAL.md`. `LICENSE.md` carries the
verbatim canonical PolyForm text; `Cargo.toml [workspace.package] license` holds
the SPDX id, inherited by every member via `license.workspace = true`.
</decision>

<rationale>
- **Maintainability** — PolyForm Noncommercial 1.0.0 has a registered SPDX
  identifier, so `cargo`, `cargo-deny`, and crates.io parse it without a custom
  expression [src: https://spdx.org/licenses/PolyForm-Noncommercial-1.0.0]. The
  text is standardized and lawyer-reviewed by the PolyForm project, so we neither
  draft nor maintain bespoke license prose [src:
  https://polyformproject.org/licenses/noncommercial/1.0.0].
- **Reliability (of intent)** — "Any noncommercial purpose is a permitted
  purpose," and personal, research, educational, nonprofit, and government uses
  are enumerated as permitted; everything else (commercial use) falls outside the
  public grant and needs a separate license. This matches the owner's "open but my
  benefit" intent exactly, with no time-bomb or competitor-only carve-out to
  reason about [src: https://polyformproject.org/licenses/noncommercial/1.0.0].
- **Maintainability (dual-license posture)** — PolyForm's permitted-purpose
  framing cleanly supports a split where the noncommercial grant is public and the
  commercial grant is sold separately, so one upstream-maintained license plus a
  short commercial notice covers both audiences (D2) [src:
  .claude/plans/github-launch-readiness/plan.md D2].
</rationale>

<alternatives>
- **Business Source License 1.1 (BUSL-1.1)** — rejected. It auto-converts to an
  OSS license after a change date (≤4 years) and requires an "Additional Use
  Grant" production carve-out, giving a weaker and time-limited "my benefit"
  guarantee than a perpetual noncommercial license [src: https://mariadb.com/bsl11/].
- **Functional Source License (FSL)** — rejected. It permits free internal
  commercial use and only blocks competing products, which contradicts the intent
  that *any* commercial use needs the owner's consent [src: https://fsl.software/].
- **MIT / Apache-2.0** — rejected. OSI-approved permissive licenses permit
  unrestricted commercial use and cannot discriminate against commercial fields of
  endeavor, the precise outcome the owner wants to prevent [src:
  https://opensource.org/osd].
</alternatives>

<consequences>
- `LICENSE.md` must remain byte-identical to the canonical PolyForm Noncommercial
  1.0.0 text; edits to it require superseding this ADR.
- `cargo-deny`'s license check covers the workspace's own members; because they
  are `publish = false`, they are exempted via `[licenses.private] ignore = true`
  rather than adding PolyForm to the third-party `allow` list [src:
  https://embarkstudios.github.io/cargo-deny/checks/licenses/cfg.html].
- A non-OSI license may deter some contributors and adopters; the README must
  state plainly that noncommercial use is free and commercial use is available by
  contact (plan R1).
- crates.io publishing (a later, optional tier) must verify SPDX acceptance of a
  noncommercial license with `cargo publish --dry-run` before any real publish
  (plan R5).
- Commercial licensing terms live in `LICENSE-COMMERCIAL.md` with an owner contact
  placeholder; the owner fills the contact before launch.
</consequences>

<sources>
- [PolyForm Noncommercial 1.0.0](https://polyformproject.org/licenses/noncommercial/1.0.0)
- [SPDX PolyForm-Noncommercial-1.0.0](https://spdx.org/licenses/PolyForm-Noncommercial-1.0.0)
- [OSI Open Source Definition](https://opensource.org/osd)
- [Business Source License 1.1](https://mariadb.com/bsl11/)
- [Functional Source License](https://fsl.software/)
- [cargo-deny licenses config](https://embarkstudios.github.io/cargo-deny/checks/licenses/cfg.html)
- [`.claude/plans/github-launch-readiness/plan.md` D1, D2](../../.claude/plans/github-launch-readiness/plan.md)
</sources>
