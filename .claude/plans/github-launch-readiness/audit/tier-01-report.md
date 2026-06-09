---
tier_id: tier-01
audited: 2026-06-09
verdict: PASS
commit: 4f5855d6a031db99250a8a91381b1b61f9b6a007
---

<scope>
Audited tier-01 "Switch the workspace to PolyForm Noncommercial 1.0.0 +
commercial grant" against its sibling `plan.md` (github-launch-readiness).
Scoped diff = the tier's `<files>`:
- `LICENSE.md` (new) — verbatim PolyForm Noncommercial 1.0.0 text.
- `LICENSE-COMMERCIAL.md` (new) — commercial-grant notice + contact placeholder.
- `Cargo.toml` (modified) — `[workspace.package] license` → SPDX id.
- `docs/adr/0033-licensing-model.md` (new) — decision record.
- `README.md` (modified) — single `## License` section.
- `deny.toml` (modified) — `[licenses.private] ignore = true`.

Out of scope and excluded: the other uncommitted working-tree changes
(`CLAUDE.md`, `crates/ariadne-mcp/*`, `docs/adr/0029`, the `data-fidelity-arc`
plan files) belong to unrelated prior work, not tier-01.
</scope>

<checks_run>
- plan_adherence — every `<files>` entry touched as the steps describe; no code
  paths or out-of-scope files altered by this tier. Per-crate manifests all keep
  `license.workspace = true` (12/12 crates) — inheritance left intact per step 3.
- correctness — `LICENSE.md` byte-fidelity verified by `curl` of the canonical
  `1.0.0`-tag raw text + `diff` (IDENTICAL) + `sha256` match
  (`c0ea4a896d2c8c394b29f9427589996db826cd501c512279ff0ed3ef48fabbe5` on both;
  73 lines / 4563 bytes each)
  [src: https://raw.githubusercontent.com/polyformproject/polyform-licenses/1.0.0/PolyForm-Noncommercial-1.0.0.md].
- architecture — files-and-config only; no smuggled dependency, tech, or code
  path. Matches the plan `<architecture>` "Licensing surface".
- security — n/a (no executable code, no input handling, no secrets). The
  `<OWNER-CONTACT-PLACEHOLDER>` is an intended fill-in, not a leaked secret.
- docs — ADR-0033 matches `docs/adr/_template.md` section-for-section
  (`<status>` Accepted / `<context>` / `<decision>` / `<rationale>` /
  `<alternatives>` / `<consequences>` / `<sources>`); cites all three rejected
  licenses (BUSL-1.1, FSL, MIT/Apache). 0033 is the next free ADR number after
  0032. ADR is linked from `plan.md:165` and back-links to the plan; both
  relative paths resolve to repo root.
- exit_criteria — all four independently verified (see verification re-run).

Verification commands re-run:
- `cargo build --workspace` → exit 0; no SPDX/license warning on stderr.
- `cargo deny check` → exit 0 ("advisories ok, bans ok, licenses ok, sources ok").
- Stash-comparison: with the `deny.toml` change reverted, `cargo deny check
  licenses` exits 4 (PolyForm rejected); with it applied, exit 0 — confirming the
  `[licenses.private] ignore = true` edit is what flips the gate green. The 11
  "license was not encountered" warnings are identical before and after, i.e.
  pre-existing `unused-allowed-license = "warn"` noise, not introduced here.
- `grep 'PolyForm-Noncommercial-1.0.0' Cargo.toml` → matches at line 21.
- `grep -rin 'MIT OR Apache' Cargo.toml README.md` → no hits.

Ariadne code-intelligence tools were not used: this tier touches only docs and
manifest metadata, so there are no symbols, references, or dependency edges in
the diff for the graph to reason about.
</checks_run>

<findings>
| id | category | severity | location | problem | fix |
|---|---|---|---|---|---|
| F1 | plan_adherence | INFO | tools/ariadne-sfc-scip/package.json:5; package-lock.json:10 | The internal SFC→SCIP Node helper still declares `"license": "MIT OR Apache-2.0"`, a latent contradiction once the repo is public under PolyForm NC. Outside tier-01's `<files>` and outside the tier's verification grep (scoped to Cargo.toml/README), so non-blocking. | Owner/a later tier should reconcile or intentionally keep the tool separately licensed. |
</findings>

<verdict>
PASS. All four exit criteria are satisfied: `LICENSE.md` is byte-identical to the
canonical PolyForm Noncommercial 1.0.0 text (sha256 match), `Cargo.toml
[workspace.package] license` is the SPDX id `PolyForm-Noncommercial-1.0.0`,
ADR-0033 exists (Accepted, template-conformant, linked from the plan), and both
`cargo build --workspace` and `cargo deny check` run green. The `deny.toml`
exemption is the minimal, correct change (private workspace members ignored
rather than polluting the third-party `allow` list). One INFO finding, non-gating.

Note on "committed": the exit criterion's word "committed" cannot be literally
true at audit time — the spec lifecycle runs `spec-audit` before the commit it
gates (`.claude/hooks/audit-gate.sh`). The artifacts exist on disk, are
well-formed, and are linked; the substantive criterion is met. The PASS verdict
unblocks the commit.
</verdict>

<next_steps>
- Commit the tier-01 diff (now unblocked by this PASS).
- Owner: fill `Contact: <OWNER-CONTACT-PLACEHOLDER>` in `LICENSE-COMMERCIAL.md`
  before the public launch.
- Track F1 (tools/ariadne-sfc-scip license) for a later tier or an explicit
  owner decision; `.github/workflows/release.yml`'s SPDX header comment is
  tier-02's regeneration concern, not tier-01's.
</next_steps>

<sources>
- [PolyForm Noncommercial 1.0.0 canonical text](https://raw.githubusercontent.com/polyformproject/polyform-licenses/1.0.0/PolyForm-Noncommercial-1.0.0.md)
- [SPDX PolyForm-Noncommercial-1.0.0](https://spdx.org/licenses/PolyForm-Noncommercial-1.0.0)
- [cargo-deny licenses config (private.ignore)](https://embarkstudios.github.io/cargo-deny/checks/licenses/cfg.html)
- [Google eng-practices — reviewer standard](https://google.github.io/eng-practices/review/reviewer/standard.html)
</sources>
</content>
