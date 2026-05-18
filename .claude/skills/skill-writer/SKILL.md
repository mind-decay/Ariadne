---
name: skill-writer
description: Authors a new Claude Code SKILL.md in `.claude/skills/<name>/` using the project's evidence-first, XML-structured, ≤200-line standard. Use when the user asks to "create a skill", "write a SKILL.md", "add a skill", "new skill", or invokes `/skill-writer`. Enforces alignment loop, mandatory doc fetch, and frontmatter validation before writing.
when_to_use: User wants a new SKILL.md authored to repo standards (XML body, third-person frontmatter, ≤200 lines, evidence-cited). Not for editing CLAUDE.md (use rules-writer) or for executing skills.
allowed-tools: Read Write Edit Bash WebFetch WebSearch mcp__claude_ai_Context7__resolve-library-id mcp__claude_ai_Context7__query-docs AskUserQuestion TaskCreate TaskUpdate
disable-model-invocation: true
---

<purpose>
Produce one SKILL.md in `.claude/skills/<slug>/SKILL.md` that meets repo standards: evidence-cited, XML-structured body, ≤200 lines, third-person frontmatter. Skill body is for another Claude instance — write for that reader, not the human.
</purpose>

<non_negotiables>
- Body ≤200 lines (project rule: shorter beats longer when content holds).
- Frontmatter `name` ≤64 chars, `[a-z0-9-]` only, no reserved words (`anthropic`, `claude`).
- Frontmatter `description` ≤1024 chars, third person, includes both *what* and *when to use* with explicit trigger phrases ([Anthropic skill best practices](https://platform.claude.com/docs/en/agents-and-tools/agent-skills/best-practices)).
- Body uses imperative voice, not second person ([Anthropic plugin-dev SKILL.md canonical reference](https://github.com/anthropics/claude-code/blob/main/plugins/plugin-dev/skills/skill-development/SKILL.md)).
- XML tags wrap semantic sections (`<purpose>`, `<workflow>`, `<gates>`, `<output>`, `<sources>`, …) — Claude is trained on XML structure ([Anthropic prompting best practices](https://platform.claude.com/docs/en/build-with-claude/prompt-engineering/claude-prompting-best-practices)).
- Every non-trivial claim/decision in the skill carries an inline `[src: url]` citation. No invented APIs, flags, or library syntax.
- Mandatory doc fetch this session: WebSearch + Context7 (when `resolve-library-id` succeeds) for every external technology the skill instructs Claude to use. WebSearch is the fallback when Context7 has no entry or quota is exhausted.
- Skill references at most one level deep; supporting files live next to SKILL.md and are linked directly from it.
</non_negotiables>

<inputs>
- `$ARGUMENTS` (optional): a free-form description of the desired skill.
- Repo context: existing skills under `.claude/skills/` (read for naming consistency, conventions).
</inputs>

<workflow>
<step id="1" name="intake">
Read `$ARGUMENTS` and any user-provided context. If empty, ask: "Describe the skill: trigger phrases, single-sentence purpose, expected inputs/outputs."
</step>

<step id="2" name="alignment_loop">
Maintain this readiness checklist and refuse to write SKILL.md until every item is resolved. Ask via `AskUserQuestion` (≤4 questions per batch) until done; prefer concrete options with a recommendation:
- [ ] Trigger phrases the user would actually say.
- [ ] Single-sentence purpose (one capability, not many).
- [ ] Invocation policy: `disable-model-invocation` true/false, `user-invocable` true/false.
- [ ] `allowed-tools` set (least-privilege, explicit list).
- [ ] External technologies/libraries the skill instructs about (drives doc fetch).
- [ ] Supporting files needed (`scripts/`, `references/`, `examples/`) or none.
- [ ] Output artifacts and where they land.
- [ ] Failure modes the skill must guard against.
When checklist is full, print a 6-line summary and ask explicit "go" before any Write.
</step>

<step id="3" name="evidence_fetch">
For every external tech enumerated in step 2:
1. Call `mcp__claude_ai_Context7__resolve-library-id`. If a match returns, follow with `query-docs` for the exact API/syntax the skill cites.
2. If Context7 has no match or returns quota-exceeded, fall back to `WebSearch` against the official docs domain, then `WebFetch` the canonical page.
3. Record `[src: url]` per fact. Discard any fact without a source.
</step>

<step id="4" name="draft">
Build the file in memory before writing:
- Frontmatter: `name`, `description` (third-person + triggers + when-to-use), `when_to_use`, `allowed-tools`, `disable-model-invocation`, `user-invocable`, `paths` (if path-scoped).
- Body sections (XML tags, in this order, omit when N/A): `<purpose>`, `<non_negotiables>`, `<inputs>`, `<workflow>` (with numbered `<step>` children), `<gates>`, `<output>`, `<anti_patterns>`, `<sources>`.
- Imperative voice. No second person. No filler.
</step>

<step id="5" name="self_validate">
Before Write, verify:
- `wc -l` of drafted body ≤ 200.
- Frontmatter regexes pass (`name`, `description` length, reserved-word check).
- Every `[src: …]` resolves to a URL fetched in step 3.
- Every XML opening tag has a matching close.
- No second-person ("you", "your") outside quoted user-facing text.
If any check fails, fix and re-validate. Do not Write a non-compliant file.
</step>

<step id="6" name="write_and_report">
Write to `.claude/skills/<slug>/SKILL.md`. Create supporting subdirs only if step 2 listed them. Report: file path, line count, citations used, and what the user should test next.
</step>
</workflow>

<gates>
- Refuse to proceed past step 2 without explicit user "go".
- Refuse to Write if step 5 reports any failure.
- Refuse to invent CLI flags, library APIs, or version numbers — fetch or omit.
</gates>

<output>
A single SKILL.md plus any explicitly-scoped supporting files. No README. No commentary files. No emojis unless the user asked.
</output>

<anti_patterns>
- Padding the body to look thorough. Concision is the rule ([Anthropic skill best practices](https://platform.claude.com/docs/en/agents-and-tools/agent-skills/best-practices)).
- Vague descriptions like "helps with X" — the description is the trigger, not documentation.
- Multi-level reference chains (SKILL.md → A.md → B.md). Keep references one level deep.
- Including time-sensitive language ("after 2026 use …"). Use a stable phrasing or an "old patterns" section.
- Skipping evidence fetch because "I know this lib" — training data drifts; the rule is mandatory fetch per session.
</anti_patterns>

<sources>
- [Skill authoring best practices — Anthropic](https://platform.claude.com/docs/en/agents-and-tools/agent-skills/best-practices)
- [Extend Claude with skills — Claude Code docs](https://code.claude.com/docs/en/skills)
- [Canonical plugin-dev SKILL.md — anthropics/claude-code GitHub](https://github.com/anthropics/claude-code/blob/main/plugins/plugin-dev/skills/skill-development/SKILL.md)
- [Prompting best practices (XML tags) — Anthropic](https://platform.claude.com/docs/en/build-with-claude/prompt-engineering/claude-prompting-best-practices)
</sources>
