---
tier_id: tier-03
audited: 2026-06-03
verdict: PASS
commit: e273c6dca1a7dedddef38438735eb7a6dd7a16f0
---

<scope>
Tier-03 — SessionStart hook that injects the `ariadne digest` as factual
`additionalContext`, installed by `ariadne setup`. Reviewed the working-tree diff
on top of HEAD `e273c6d` scoped to the tier's `<files>`:

- `crates/ariadne-cli/src/commands/setup.rs` — embedded hook template + binary-path
  substitution + executable bit + `merge_settings_json` (idempotent `hooks` deep-merge).
- `.claude/hooks/ariadne-session-start.sh` — the installed (dogfooded) hook script.
- `crates/ariadne-cli/tests/setup.rs` — new `setup_installs_session_start_hook_preserving_existing_hooks` test + CLAUDE.md tool-surface assertions.
- `.claude/settings.json` — gained the `SessionStart` entry; PreToolUse retained.

Out of tier `<files>` but present in the working tree: `.claude/hooks/audit-gate.sh`
(sibling-state rewrite described in D3e — see F3).
</scope>

<checks_run>
- `cargo nextest run -p ariadne-cli` → **29/29 pass**, incl. the new hook test, `setup_is_byte_idempotent`, and `digest_emits_bounded_agent_shaped_markdown`.
- `cargo fmt -p ariadne-cli --check` → clean (exit 0).
- `cargo clippy -p ariadne-cli --all-targets --all-features -- -D warnings` → clean (exit 0).
- `echo '{}' | ./.claude/hooks/ariadne-session-start.sh` → exit 0, valid JSON, `hookSpecificOutput.hookEventName=="SessionStart"`, `additionalContext` = 1570 chars (well under the 10k cap), carries the live digest (revision 380).
- Fail-open: BIN→`/nonexistent` and BIN→`/usr/bin/true` (empty stdout) both → exit 0, valid JSON, factual fallback. jq-missing branch reviewed (silent `exit 0`, no malformed JSON).
- `jq` over `.claude/settings.json` → valid; `hooks.PreToolUse` audit-gate present (matcher `Bash`), `hooks.SessionStart` registers the script (no matcher, fires every start).
- Read all four in-scope files end-to-end; cross-read `digest.rs`/`main.rs` to confirm `ariadne digest <root>` (positional, default `.`) matches the hook's `"$BIN" digest "$DIR"` call.
- **EC4 dogfood verified by direct observation**: this very session's SessionStart context contained `## Ariadne project digest … revision 380: 362 files, 3371 symbols, 4730 dependency edges`, byte-matching the hook's stdout — the hook fires and the digest reaches context end-to-end.
</checks_run>

<findings>
| id | category | severity | location | problem | fix |
|----|----------|----------|----------|---------|-----|
| F1 | correctness | INFO | `.claude/hooks/ariadne-session-start.sh:20-22` (template `setup.rs:54-55`) | The shell fallback `additionalContext` is imperative ("Prefer the Ariadne MCP tools…", "query project_status…"), while the comment claims it is "phrased as project state rather than an instruction" — mild tension with EC3/D3 factual-phrasing intent. Non-blocking: the primary digest path is factual, the stakes are a degraded edge, and it mirrors tier-02's already-shipped `fallback()` phrasing. | Rephrase the fallback as declaratives (e.g. "The Ariadne MCP tools answer symbol, reference, impact, and architecture questions; `project_status` reports index freshness."). |
| F2 | tests | INFO | `setup.rs:merge_settings_json` (`to_string_pretty`); `tests/setup.rs:228-241` | `serde_json::Value` uses `BTreeMap` (no `preserve_order`), so the merge alphabetically sorts **all** keys — a hand-authored `PreToolUse` block is key-reordered on first `setup` (visible in the repo diff: `matcher`/`type` moved). EC2 ("preserving existing hooks") and idempotency hold, but D3d's literal "byte-identical `PreToolUse`" does not for unsorted input. The test masks this: its pre-seeded input is already `to_string_pretty`-sorted and it asserts `Value` (order-insensitive) equality. | Either soften D3d's wording to "semantically preserved", or feed the test an unsorted `PreToolUse` block and assert the audit-gate hook still resolves. |
| F3 | plan_adherence | INFO | `.claude/hooks/audit-gate.sh` | Modified in the working tree but not in tier-03 `<files>` (it is sibling-state per D3e, a separate concern). Tier-03 only had to *preserve* the PreToolUse entry, which it does. | Commit the audit-gate rewrite separately from the tier-03 deliverable. |
| F4 | plan_adherence | INFO | `setup.rs:render_block` (+`tests/setup.rs:71-80`) | `render_block` gained `diff_blast_radius`/`hotspots`/`complexity`/`co_change` — CLAUDE.md-block maintenance tangential to tier-03's SessionStart focus. Correct, tested, and keeps the block in sync with the digest cheat-sheet and shipped tools. | None required; note the bundling. |
</findings>

<verdict>
**PASS** — 0 FAIL, 4 INFO.

All four exit criteria independently verified:
1. The hook runs `ariadne digest` and emits valid JSON with `hookEventName=="SessionStart"` + non-empty `additionalContext` holding the digest — confirmed by running it and by `jq`.
2. `ariadne setup` installs the executable script and registers the SessionStart entry idempotently while preserving the PreToolUse audit-gate — confirmed by the passing merge/idempotency test.
3. `additionalContext` primary content (the digest) is factual; only the degraded fallback carries mild imperative phrasing (F1, non-blocking).
4. This repo's `settings.json` gained the SessionStart hook and the digest **is observably present in this session's context** — the strongest possible real-run evidence.

Design decisions honored: jq-built JSON, no string interpolation (D3a); exact SessionStart emit shape (D3b); fail-open on missing binary / empty digest / missing jq, never a non-zero exit or malformed JSON (D3c); SessionStart-only deep-merge leaving PreToolUse semantically intact, no matcher on the entry (D3d). No hexagonal violation — the hook is a config artifact owned by the `ariadne-cli` composition root; no new dependency, no domain leak. No security finding: digest path is single-quote escaped at install, `$DIR` is quoted, and the payload is jq `--arg`-escaped.
</verdict>

<next_steps>
Verdict is PASS; no tier steps require redo. Optional, non-blocking polish for a future
touch: F1 (factual fallback wording, also applies to the tier-02 `digest::fallback`),
F2 (assert byte-preservation against unsorted input). Commit the F3 audit-gate change
on its own.
</next_steps>

<sources>
- [Claude Code hooks — SessionStart/PreToolUse output schema](https://code.claude.com/docs/en/hooks)
- [Claude Code MCP — alwaysLoad / Tool Search & 10k additionalContext cap](https://code.claude.com/docs/en/mcp)
- [SessionStart hooks force context — MindStudio](https://www.mindstudio.ai/blog/session-start-hooks-claude-code-force-context)
- [Google eng-practices — reviewer standard (ship code health, not perfection)](https://google.github.io/eng-practices/review/reviewer/standard.html)
- Repo: tier-03 `<exit_criteria>`/`<decisions>`/`<steps>`; plan.md D3/D4/D6; setup.rs; digest.rs; .claude/settings.json.
</sources>
