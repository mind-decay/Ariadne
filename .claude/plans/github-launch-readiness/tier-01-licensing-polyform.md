---
tier_id: tier-01
title: Switch the workspace to PolyForm Noncommercial 1.0.0 + commercial grant
deps: []
exit_criteria:
  - LICENSE.md byte-matches the canonical PolyForm Noncommercial 1.0.0 text
  - Cargo.toml [workspace.package] license == "PolyForm-Noncommercial-1.0.0"
  - docs/adr/0033-licensing-model.md committed and linked from plan
  - cargo build --workspace and cargo deny check both green
status: completed
completed: 2026-06-09
---

<context>
The manifest declares `MIT OR Apache-2.0` [src: Cargo.toml:21], which permits
unrestricted commercial use — the opposite of the owner's intent. Replace it
with PolyForm Noncommercial 1.0.0: free for any noncommercial purpose, paid
commercial license required otherwise [src: plan.md D1;
https://polyformproject.org/licenses/noncommercial/1.0.0]. This tier is
foundational — release metadata, the Homebrew formula, crates.io, and the README
all read this license. No LICENSE file exists on disk today.
</context>

<files>
- `LICENSE.md` — create; verbatim PolyForm Noncommercial 1.0.0 canonical text.
- `LICENSE-COMMERCIAL.md` — create; short notice that commercial use needs a
  separate paid license, with a contact placeholder for the owner to fill.
- `Cargo.toml` — modify; `[workspace.package] license` → the SPDX id.
- `docs/adr/0033-licensing-model.md` — create; record the decision + rejected
  alternatives, using the ADR template.
- `README.md` — modify only the single license line (`## License` section,
  currently "MIT OR Apache-2.0." [src: README.md:158-160]) to point at PolyForm
  NC + commercial grant. Full README redesign is tier-05; touch one line here so
  no committed contradiction exists between tiers.
- `deny.toml` — modify only if `cargo deny check` flags the workspace's own
  license (see steps).
</files>

<steps>
1. Fetch the canonical text from
   `https://github.com/polyformproject/polyform-licenses/blob/1.0.0/PolyForm-Noncommercial-1.0.0.md`
   (raw) and write it verbatim to `LICENSE.md`. Do not paraphrase — byte-fidelity
   is the exit criterion [src: https://spdx.org/licenses/PolyForm-Noncommercial-1.0.0].
2. Write `LICENSE-COMMERCIAL.md`: state that the software is licensed to the
   public under PolyForm Noncommercial 1.0.0 (see `LICENSE.md`), that any
   commercial use requires a separate commercial license from the copyright
   holder, and give a contact line `Contact: <OWNER-CONTACT-PLACEHOLDER>` for the
   owner to fill. Reference the PolyForm dual-license model [src: plan.md D2].
3. In `Cargo.toml`, set `license = "PolyForm-Noncommercial-1.0.0"` in
   `[workspace.package]`. This is a valid SPDX identifier cargo accepts
   [src: https://spdx.org/licenses/PolyForm-Noncommercial-1.0.0]. Leave the
   per-crate `license.workspace = true` inheritance intact.
4. Create `docs/adr/0033-licensing-model.md` (next free number; 0032 is the
   current max [verify: ls docs/adr]). Status: Accepted. Record D1/D2: chosen =
   PolyForm Noncommercial 1.0.0; rejected = BUSL-1.1 [src: https://mariadb.com/bsl11/],
   FSL [src: https://fsl.software/], MIT/Apache (permit commercial use). Link the
   ADR from `plan.md`. Match the existing ADR template
   [src: docs/adr/0001-architecture-style.md].
5. Update the README `## License` line to: noncommercial use under PolyForm
   Noncommercial 1.0.0 (`LICENSE.md`); commercial use under `LICENSE-COMMERCIAL.md`.
6. Run `cargo build --workspace` then `cargo deny check`. If deny flags the
   workspace's own PolyForm license, exempt the `publish = false` workspace
   members via `[licenses.private] ignore = true` rather than adding PolyForm to
   the dependency `allow` list (the allow list is for third-party deps)
   [src: https://embarkstudios.github.io/cargo-deny/checks/licenses/cfg.html].
</steps>

<verification>
- `test -f LICENSE.md && head -1 LICENSE.md` shows the PolyForm title; diff the
  body against the fetched canonical text (must be identical).
- `grep 'PolyForm-Noncommercial-1.0.0' Cargo.toml` matches in `[workspace.package]`.
- `cargo build --workspace` → exits 0.
- `cargo deny check` → exits 0 (licenses + advisories pass).
- `grep -ri 'MIT OR Apache' .` returns no hits in Cargo.toml or README (only
  historical ADRs/comments may remain).
- ADR file exists, status Accepted, cites the three rejected licenses.
</verification>

<rollback>
`git checkout -- Cargo.toml README.md deny.toml` and
`rm LICENSE.md LICENSE-COMMERCIAL.md docs/adr/0033-licensing-model.md`. No code
or schema changed, so reverting metadata is sufficient; no rebuild artifacts to
clean beyond `cargo build`.
</rollback>
</content>
