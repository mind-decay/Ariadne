---
tier_id: tier-03
title: SessionStart hook — inject the digest as factual additionalContext, installed by setup
deps: [tier-02]
exit_criteria:
  - "A `SessionStart` hook script runs `ariadne digest` and emits valid JSON: `hookSpecificOutput.hookEventName==SessionStart` + `additionalContext` holding the digest."
  - "`ariadne setup` installs the script and registers the SessionStart entry in `.claude/settings.json`, idempotently, preserving existing hooks (e.g. the Bash audit-gate)."
  - "additionalContext is phrased as factual project state, not imperative commands."
  - "This repo's settings.json gains the SessionStart hook; a fresh session shows the digest in context."
status: completed
completed: 2026-06-03
---

<context>
Fulfills the "knows the project at session start" intent: the SessionStart hook's
`hookSpecificOutput.additionalContext` is added to context before the first prompt
(≤10k chars) [src: https://code.claude.com/docs/en/hooks]. Phrase it as factual
statements — imperative out-of-band text trips Claude's prompt-injection defenses
[src: https://www.mindstudio.ai/blog/session-start-hooks-claude-code-force-context].
`setup` already owns idempotent config writes, so it installs this too [src:
plan.md D3, D6; setup.rs:25-86]. The existing PreToolUse/Bash audit-gate must
survive the merge [src: .claude/settings.json].
</context>

<decisions>
Recorded this session while hardening the sibling Bash audit-gate hook; they
constrain how this tier is built [src: https://code.claude.com/docs/en/hooks
PreToolUse/SessionStart output; .claude/settings.json; .claude/hooks/audit-gate.sh].
- D3a — Build the hook's JSON with `jq -n --arg`/`--rawfile`, never string
  interpolation. The digest can carry quotes, backslashes, and newlines;
  interpolating it into a hand-written `{...}` is the parse-failure bug class.
  `jq` escaping guarantees the `additionalContext` payload always parses
  [verified this session: the audit-gate rewrite uses `jq -n --arg`].
- D3b — Exact emit on exit 0:
  `{"hookSpecificOutput":{"hookEventName":"SessionStart","additionalContext":<digest>}}`.
  `additionalContext` is factual (the tier-02 digest already is); add no
  imperative wrapper. Do not confuse with PreToolUse: `permissionDecision`/`deny`
  is a *blocking* verb and has no place in a SessionStart injection.
- D3c — Make the script fail-open: a missing binary, empty digest, or non-zero
  exit prints the minimal factual fallback and exits 0 — never a non-zero exit
  or malformed JSON (both surface as a `hook error` and defeat the bootstrap).
- D3d — The settings.json merge must DEEP-merge `hooks`: add/replace only
  `hooks.SessionStart`, leaving `hooks.PreToolUse` (the audit-gate) semantically
  intact — `serde_json` re-sorts object keys on serialize (`Value` is a
  `BTreeMap`), so a hand-authored entry's key order may change but its keys and
  values survive. Because the first run normalizes the file, a second `setup`
  run yields byte-identical settings.json. Mirror `merge_mcp_json`'s
  object-entry pattern [setup.rs:72-81]. `matcher` filters tool NAME, so the
  SessionStart entry needs no matcher (fires on every session start).
- D3e — Sibling state the new session inherits: the live audit-gate this tier must
  preserve was rewritten this session — it now fires only on real `git commit`/`push`
  invocations (segment-aware, not substring), blocks via `permissionDecision:"deny"`
  JSON, is fail-open, and blocks ONLY on a FAIL verdict (the stale-PASS/HEAD-advanced
  gate was removed). Build's `git commit` of this tier is therefore no longer
  blocked by a stale audit; the merge must still preserve that PreToolUse entry
  verbatim.
</decisions>

<files>
- `crates/ariadne-cli/src/commands/setup.rs` — install hook script + register the
  SessionStart entry in `.claude/settings.json` (idempotent JSON merge mirroring
  `merge_mcp_json`) [setup.rs:44-86].
- Hook script template (emitted by setup, e.g. embedded `&str`) →
  `<root>/.claude/hooks/ariadne-session-start.sh`.
- `crates/ariadne-cli/tests/` — assert settings.json + script after `setup`.
- This repo: `.claude/settings.json`, `.claude/hooks/ariadne-session-start.sh`.
</files>

<steps>
1. **Failing test.** Run `setup` on a temp project pre-seeded with an existing
   `.claude/settings.json` (a dummy PreToolUse hook). Assert: the dummy hook
   survives; a `hooks.SessionStart` entry now points at
   `.claude/hooks/ariadne-session-start.sh`; the script exists and is executable.
   Run — fails.
2. **Author the hook script.** A POSIX `sh` script that runs the resolved
   `ariadne digest` for `$CLAUDE_PROJECT_DIR` and prints JSON:
   `{"hookSpecificOutput":{"hookEventName":"SessionStart","additionalContext":<digest>}}`.
   On non-zero/empty digest, print a minimal factual fallback. Keep the wrapper
   factual; the digest content is already factual [src:
   https://code.claude.com/docs/en/hooks SessionStart schema; mindstudio phrasing].
3. **Install via setup.** Write the script (chmod +x) and merge the SessionStart
   entry into `.claude/settings.json` without disturbing other events — deep-merge
   the `hooks` object, append to the `SessionStart` array if present [src:
   setup.rs:44-86 merge pattern; https://code.claude.com/docs/en/hooks structure].
4. **Length guard.** The digest is already <10k (tier-02); the hook does not add
   bulk. If digest ever overflows, Claude Code spills to a file+preview — acceptable
   but assert the digest path stays under cap in tier-02.
5. **Dogfood.** Run `ariadne setup` here; verify the SessionStart entry + script.
</steps>

<verification>
- `cargo nextest run -p ariadne-cli` — merge/idempotency test passes; re-running
  `setup` twice yields byte-identical settings.json (idempotent).
- `echo '{}' | .claude/hooks/ariadne-session-start.sh` (or invoke directly) emits
  JSON that parses and contains `hookEventName:"SessionStart"` and a non-empty
  `additionalContext`. Validate with a JSON parser; report the result.
- Real run: start a fresh Claude Code session here; confirm the digest text is
  present in context (e.g. ask "what did you receive at session start?"). State the
  observation; if not runnable in-session, say so explicitly.
- Fail loudly: a malformed-JSON hook is a hard fail — do not ship a script whose
  output fails to parse.
</verification>

<rollback>
`setup` re-run can't un-register; to revert, delete the SessionStart entry from
`.claude/settings.json` and remove the hook script. Config-only, no data.
</rollback>
