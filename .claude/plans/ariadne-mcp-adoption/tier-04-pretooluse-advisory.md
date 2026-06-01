---
tier_id: tier-04
title: PreToolUse advisory — non-blocking steer on symbol-shaped grep/Read toward Ariadne
deps: [tier-01]
exit_criteria:
  - "A `PreToolUse` hook on Grep/Glob/Read returns `permissionDecision: allow` plus `additionalContext` naming the matching Ariadne tool, only when the query is symbol-shaped."
  - "Non-symbol queries (log strings, comments, non-code paths) pass through with no added context (decision `defer`/`allow`, empty context)."
  - "`ariadne setup` installs the script and PreToolUse entry idempotently, preserving the existing Bash audit-gate PreToolUse hook."
  - "This repo's settings.json gains the advisory hook; a real grep for a symbol shows the injected suggestion."
status: pending
---

<context>
Visibility (tier-01) + bootstrap (tier-03) may not fully flip reflexive grepping,
so add a gentle nudge: `PreToolUse` can return `permissionDecision: allow` with
`additionalContext` injected alongside the tool result [src:
https://code.claude.com/docs/en/hooks]. Advisory (not `deny`/`ask`) avoids breaking
legitimate text search and constant prompts [src: plan.md D5, R5; user decision].
The existing Bash audit-gate PreToolUse hook must be preserved [src:
.claude/settings.json].
</context>

<files>
- Hook script template (emitted by setup) → `<root>/.claude/hooks/ariadne-grep-advisor.sh`.
- `crates/ariadne-cli/src/commands/setup.rs` — install script + register PreToolUse
  entry (matcher covering Grep/Glob/Read) without clobbering the Bash matcher.
- `crates/ariadne-cli/tests/` — assert classification + settings merge.
- This repo: `.claude/settings.json`, `.claude/hooks/ariadne-grep-advisor.sh`.
</files>

<steps>
1. **Failing test.** Feed the advisor script representative PreToolUse payloads on
   stdin (Claude Code passes the tool input as JSON): (a) `Grep` for an
   identifier-shaped pattern (e.g. `^[A-Za-z_][A-Za-z0-9_]*$`, CamelCase, or
   `snake_case::path`) → expect `permissionDecision:"allow"` + non-empty
   `additionalContext` mentioning `find_references`/`list_symbols`; (b) `Grep` for a
   quoted log string or a `*.md` path → expect pass-through, empty context. Run —
   fails (script absent).
2. **Author the advisor.** A `sh`/`jq`-free script (parse with the shipped
   `ariadne`? no — keep it dependency-light) that reads the JSON payload, applies a
   tight symbol-shaped heuristic to the query/pattern, and prints
   `{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"allow","additionalContext":"…"}}`
   on a match, else minimal `{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"defer"}}`
   [src: https://code.claude.com/docs/en/hooks PreToolUse schema]. Suggestion text
   is factual and names the specific tool for the shape (definition vs references).
3. **Heuristic boundaries.** Match: bare identifiers, `Type`/`fn` names,
   `module::path`. Skip: whitespace-containing phrases, quoted strings, glob-only
   patterns, and non-source path filters. Document the regex inline; favor
   precision over recall (R5).
4. **Install via setup.** Append a PreToolUse entry with a matcher for
   `Grep|Glob|Read` alongside the existing Bash matcher; deep-merge the array so the
   audit-gate survives [src: setup.rs:44-86; .claude/settings.json].
5. **Dogfood.** Run `ariadne setup`; verify both PreToolUse hooks coexist.
</steps>

<verification>
- `cargo nextest run -p ariadne-cli` — classification cases + settings merge pass;
  re-running `setup` is idempotent and keeps the Bash audit-gate entry.
- `cargo clippy … -D warnings`; `cargo fmt --all --check`;
  `cargo test --test architecture`.
- Real run: in a fresh session here, `grep` for an existing symbol (e.g. a struct
  name) and confirm the advisory line appears; `grep` for a quoted phrase and
  confirm no nudge. Report both observations; if not runnable in-session, say so.
- Fail loudly: if the advisor ever returns `deny`, that is a regression against D5
  — the script must never block.
</verification>

<rollback>
Delete the PreToolUse advisory entry from `.claude/settings.json` and remove the
script; the Bash audit-gate entry is untouched. Config-only.
</rollback>
