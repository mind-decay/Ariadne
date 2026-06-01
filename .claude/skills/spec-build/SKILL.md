---
name: spec-build
description: Executes one tier of a plan produced by `spec-plan`. Reads the named plan/tier file under `.claude/plans/<slug>/`, follows its steps, and updates tier status. Use when the user invokes `/spec-build <path-to-tier-or-plan>` or asks to "build tier N", "execute the plan", "run the spec". Refuses to act outside the plan; refuses to start when tier dependencies are unmet.
when_to_use: A `spec-plan` artifact exists and the user wants exactly one tier (or a single-tier plan) executed in this session. Not for ad-hoc coding (just code), not for reviewing built work (use spec-audit).
allowed-tools: Read Write Edit Bash Glob Grep WebFetch WebSearch mcp__claude_ai_Context7__resolve-library-id mcp__claude_ai_Context7__query-docs AskUserQuestion TaskCreate TaskUpdate TaskGet TaskList mcp__ariadne__project_status mcp__ariadne__list_symbols mcp__ariadne__find_definition mcp__ariadne__find_references mcp__ariadne__blast_radius
disable-model-invocation: true
---

<purpose>
Turn one tier (or a single-tier plan) into working, plan-conformant code. The plan is the contract: do exactly what it says, no more, no less. Goal is zero slop — no rewrite loops, no scope drift, no invented APIs.
</purpose>

<non_negotiables>
- Input is `$ARGUMENTS` = path to `tier-NN-<name>.md` (multi-tier) or `plan.md` (single-tier). Reject any other invocation.
- Load only two files of plan context: the specified tier file and the sibling `plan.md`. Do not load other tier files in this session — they belong to other sessions ([Anthropic skill best practices: progressive disclosure](https://platform.claude.com/docs/en/agents-and-tools/agent-skills/best-practices)).
- Refuse to start when tier frontmatter `deps` list any tier whose latest status is not `completed`.
- Code and decisions follow the plan's `<decisions>` and `<tech_inventory>`. New tech, new dependencies, or new architectural decisions trigger a hard stop and a user question — never improvise.
- Any step requiring API/syntax not already cited in the plan triggers a JIT doc fetch (Context7 → WebSearch fallback) before the edit. No syntax from memory.
- Every file edit is intentional and minimal. No drive-by refactors, no unrelated cleanups, no "while I'm here" changes ([rule reinforced by Claude Code defaults: do not add features beyond what the task requires](https://code.claude.com/docs/en/best-practices)).
- Run the tier's `<verification>` commands and act on real output. Fabricated test results are a hard fail.
- Validate by execution. For every change: build runs green, relevant tests run green, the feature is exercised end-to-end (real run, not stub or reasoning), and every observed result is compared against a stated expectation. UI/frontend changes require launching the dev server and walking the golden path; type-check + unit tests alone do not count [src: https://code.claude.com/docs/en/best-practices]. When validation cannot run in-session, state it explicitly — never claim success.
- Failures are root-caused, not silenced. No `--no-verify`, no weakened asserts, no `try/except: pass`, no deleted tests, no commented-out checks.
- Update the tier file's `status` only after `<verification>` passes for real.
</non_negotiables>

<code_intelligence>
Ariadne MCP is a read-only semantic graph of the current code (symbols, references, dependency edges). Prefer it over `grep`/`Read` for any symbol, reference, impact, or architecture question — one call where text search needs many and misses cross-file edges [src: CLAUDE.md "Ariadne code intelligence"; .mcp.json].
- Load: Ariadne tools may be schema-deferred when many MCP servers are connected (deferral triggers above ~10% of context). If a tool is not immediately callable, load it via `ToolSearch` `select:mcp__ariadne__<tool>` then retry [src: https://platform.claude.com/docs/en/agents-and-tools/tool-use/tool-search-tool].
- Freshness: call `mcp__ariadne__project_status` once before trusting the graph; if it reports stale or the server is down, fall back to `grep`/`Read` and state it [src: CLAUDE.md].
- Scope: the graph covers code that exists now. For symbols a tier will create (greenfield), Ariadne has nothing — use it for what exists, not what the plan adds.
- Use here: `find_references` + `blast_radius` before editing a symbol to find call sites and impact (steps 3, 5); `find_definition`/`list_symbols` to navigate instead of `grep` [src: CLAUDE.md].
</code_intelligence>

<inputs>
- `$ARGUMENTS`: path to a tier file or single-tier `plan.md`. If missing, ask the user for it.
- The plan files themselves.
- Repo state.
</inputs>

<workflow>
<step id="1" name="locate_and_validate">
Resolve `$ARGUMENTS` to an existing file under `.claude/plans/<slug>/`. Read it. Read sibling `plan.md`. Reject if either is missing or malformed (YAML frontmatter must parse; required fields present).
</step>

<step id="2" name="dep_check">
For each `tier_id` in this tier's `deps`, open the sibling tier file and confirm `status: completed`. If any is not completed, stop and report which tier blocks. Do not proceed.
</step>

<step id="3" name="diff_with_repo">
Compare the tier's `<files>` and `<steps>` to current repo state (`git status`, `git diff`, Read affected files). Identify already-done steps from previous partial runs. Plan only what remains.
</step>

<step id="4" name="task_list">
Create one `TaskCreate` entry per remaining step. Mark the next one `in_progress` before starting it.
</step>

<step id="5" name="execute">
For each step in order:
- Re-read referenced docs/source. If the step needs an API/flag not cited in the plan, JIT-fetch via Context7 → WebSearch fallback, then proceed. Record the citation in your reasoning, not in the code.
- Make the minimal edit the step requires. Prefer Edit over Write for existing files.
- Run any per-step check the plan specifies.
- Mark the task `completed` when, and only when, the step is fully done.
- Halt and ask the user when a step is ambiguous, contradicts the repo, or requires a decision not in the plan. Do not invent.
</step>

<step id="6" name="verify">
Run every command listed in `<verification>` exactly as specified. Capture full output. For each, write down the expected outcome before reading the actual outcome; declare match or mismatch explicitly. Then exercise the feature end-to-end (real invocation: HTTP request, CLI run, dev-server click-through for UI). Compare the observed behavior against the tier's `exit_criteria` one by one.
If a command fails or behavior diverges from expectation, do not silence it — fix the root cause or stop and report. Never delete tests, weaken assertions, add `--no-verify`, or wrap failures in `try/except: pass`. When a check cannot run in-session, say so explicitly and leave `status: blocked`.
</step>

<step id="7" name="update_status_and_report">
Only when `<verification>` truly passes:
- Edit the tier file: set `status: completed`. Add a `completed: <YYYY-MM-DD>` frontmatter line.
- Report: files touched, commands run with outcomes, citations consulted JIT, deviations escalated, and the next tier (or that the plan is done).
On failure: leave `status: in_progress` or set `status: blocked` with a one-line reason added under a new `<blockers>` body section.
</step>
</workflow>

<gates>
- Refuse to start if `$ARGUMENTS` does not resolve to a tier or single-tier plan file.
- Refuse to start when any dep tier is not `completed`.
- Refuse to add deps, files, or steps not in the plan; stop and ask.
- Refuse to mark `completed` without real, passing `<verification>` output.
</gates>

<output>
Code changes implementing exactly the tier's scope, plus the in-place status update on the tier file. No new docs, no scratchpads, no "summary" markdown unless the plan explicitly lists one.
</output>

<anti_patterns>
- "I think the API is …" — fetch or stop.
- Bundling refactors with the tier's work to "save time" — splits scope, complicates audit.
- Suppressing test failures, adding `try/except: pass`, or `--no-verify` shortcuts.
- Loading other tier files for context — that is the next session's job.
- Editing `plan.md` to retrofit the work. The plan is the contract; deviations escalate, not silently rewrite.
- Producing a "done" report when steps were skipped. Truth over politeness.
</anti_patterns>

<sources>
- [Skill authoring best practices — Anthropic](https://platform.claude.com/docs/en/agents-and-tools/agent-skills/best-practices)
- [Claude Code best practices](https://code.claude.com/docs/en/best-practices)
- [Extend Claude with skills — Claude Code docs](https://code.claude.com/docs/en/skills)
</sources>
