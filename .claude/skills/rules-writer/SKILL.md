---
name: rules-writer
description: Authors or revises project memory files (CLAUDE.md, AGENTS.md, nested directory CLAUDE.md) to the repo's evidence-first, XML-structured, ≤200-line standard. Use when the user asks to "write CLAUDE.md", "update rules", "create AGENTS.md", "set project memory", or invokes `/rules-writer`. Enforces alignment loop, mandatory doc fetch, and 200-line load limit.
when_to_use: User wants project memory or cross-tool rules authored to repo standards. Not for editing SKILL.md (use skill-writer) or for editing settings/hooks (use settings configuration).
allowed-tools: Read Write Edit Bash Glob Grep WebFetch WebSearch mcp__claude_ai_Context7__resolve-library-id mcp__claude_ai_Context7__query-docs AskUserQuestion TaskCreate TaskUpdate
disable-model-invocation: true
---

<purpose>
Produce or revise one memory file (`CLAUDE.md`, `AGENTS.md`, or nested `<dir>/CLAUDE.md`) that meets repo standards: evidence-cited, XML-structured, ≤200 lines, instruction-dense. Memory loads at session start — every line competes with task context.
</purpose>

<non_negotiables>
- Total length ≤200 lines. Claude Code loads CLAUDE.md at startup; content past the loaded budget is silently truncated ([Anthropic CLAUDE.md guidance](https://code.claude.com/docs/en/memory)).
- XML tags wrap semantic sections — Claude was trained on XML structure ([Anthropic prompting best practices](https://platform.claude.com/docs/en/build-with-claude/prompt-engineering/claude-prompting-best-practices)).
- Every non-trivial directive cites a source inline (`[src: url]`) or a project artifact (`[src: .claude/skills/spec-plan/SKILL.md]`).
- Imperative voice, not second person. Facts and rules, not narration.
- Use `@path/to/file` imports for content that belongs elsewhere; do not duplicate ([Anthropic memory docs](https://code.claude.com/docs/en/memory)).
- Precedence is documented when authoring layered memory: enterprise > project > user; deeper directory CLAUDE.md narrows scope ([Anthropic memory hierarchy](https://code.claude.com/docs/en/memory)).
- No time-sensitive phrasing ("after Q3 2026 …"). Stable wording or an "old patterns" section.
</non_negotiables>

<inputs>
- `$ARGUMENTS` (optional): a free-form description of the target file and intent.
- Repo context: existing CLAUDE.md / AGENTS.md (if any), `.claude/skills/`, `.claude/plans/` layout, build/test/lint commands.
</inputs>

<workflow>
<step id="1" name="intake">
Read `$ARGUMENTS`. Resolve target path (root `CLAUDE.md`, `AGENTS.md`, or nested). If existing file present, read it fully before proposing changes.
</step>

<step id="2" name="alignment_loop">
Maintain this readiness checklist; refuse to write until every item is resolved. Ask via `AskUserQuestion` (≤4 per batch):
- [ ] Target file path and scope (root project, nested package, user global).
- [ ] Audience: Claude Code only, or also Codex/Cursor/Jules via AGENTS.md mirror.
- [ ] Build, test, lint, type-check commands (the highest-value memory content).
- [ ] Coding conventions actually enforced in this repo (not aspirational).
- [ ] Architectural invariants the agent must not violate.
- [ ] Workflow rules (e.g., `spec-plan` → `spec-build` → `spec-audit` lifecycle, where plans live).
- [ ] What to omit because it lives in `.claude/skills/` or docs and should be linked via `@path`.
When checklist is full, print a 6-line summary and ask explicit "go" before any Write.
</step>

<step id="3" name="evidence_fetch">
For every external tech, framework, or tool referenced in the directives:
1. `mcp__claude_ai_Context7__resolve-library-id` → `query-docs` for the exact command/flag/syntax cited.
2. Fallback to `WebSearch` + `WebFetch` on the official docs domain when Context7 has no match or quota is exhausted.
3. For Claude Code platform claims (memory precedence, hooks events, settings keys), fetch `https://code.claude.com/docs/en/memory`, `…/settings`, `…/hooks-guide` as relevant.
4. Record `[src: url]` per fact. Discard any fact without a source.
</step>

<step id="4" name="draft">
Build the file in memory before writing. Recommended XML skeleton:
- `<project>` — one-paragraph what this repo is.
- `<commands>` — build/test/lint/typecheck, exact commands.
- `<conventions>` — coding rules actually enforced.
- `<architecture>` — invariants and decision boundaries with `[src: …]`.
- `<workflow>` — spec lifecycle, plan storage, audit gates.
- `<rules>` — hard nos and hard yeses (one line each).
- `<imports>` — `@path` lines to deeper docs (kept out of the 200-line budget by living elsewhere).
- `<sources>` — canonical references used.
Omit any section that has no content. No filler.
</step>

<step id="5" name="self_validate">
Before Write:
- `wc -l` ≤ 200.
- Every XML tag balanced.
- No second-person voice outside quoted examples.
- Every directive has a `[src: …]` or `@path` anchor.
- No duplication of content reachable through `@path`.
- For `AGENTS.md`, content is tool-agnostic (no `/skill-name`-style Claude-only invocations).
If any check fails, fix and re-validate. Do not Write a non-compliant file.
</step>

<step id="6" name="write_and_report">
Write the file. Report: path, line count, sources used, and one-sentence diff summary if revising.
</step>
</workflow>

<gates>
- Refuse to proceed past step 2 without explicit user "go".
- Refuse to Write if step 5 reports any failure.
- Refuse to inline content that should be `@path`-imported.
</gates>

<output>
One memory file at the target path. No README. No companion changelogs. No emojis unless the user asked.
</output>

<anti_patterns>
- Aspirational rules the codebase does not actually enforce — drift makes them noise.
- Pasting long code blocks instead of `@path` imports.
- Wishful build commands you did not verify by running.
- Duplicating spec lifecycle content already in `.claude/skills/spec-*/SKILL.md` — link to it instead.
- Mixing tool-specific syntax (`/skill-name`) into `AGENTS.md`.
</anti_patterns>

<sources>
- [Manage Claude's memory — Claude Code docs](https://code.claude.com/docs/en/memory)
- [Prompting best practices (XML tags) — Anthropic](https://platform.claude.com/docs/en/build-with-claude/prompt-engineering/claude-prompting-best-practices)
- [Claude Code settings reference](https://code.claude.com/docs/en/settings)
- [Extend Claude with skills — Claude Code docs](https://code.claude.com/docs/en/skills)
</sources>
