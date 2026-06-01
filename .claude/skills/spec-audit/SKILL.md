---
name: spec-audit
description: Pedantic, hostile-mode review of work done by `spec-build` against a `spec-plan` artifact. Writes a verdict report to `.claude/plans/<slug>/audit/<tier-or-plan>-report.md` and updates `.claude/plans/<slug>/audit-state.json` used by commit/push hooks. Use when the user invokes `/spec-audit <path-to-tier-or-plan>` or asks to "audit the build", "review the spec output", "check the implementation". Treats the code as if written by someone else; never as its own work.
when_to_use: After `spec-build` finishes a tier (or the whole single-tier plan) and before commit/push. Not for ad-hoc code review unrelated to a plan.
allowed-tools: Read Bash Glob Grep WebFetch WebSearch mcp__claude_ai_Context7__resolve-library-id mcp__claude_ai_Context7__query-docs AskUserQuestion TaskCreate TaskUpdate TaskGet TaskList Agent mcp__ariadne__project_status mcp__ariadne__list_symbols mcp__ariadne__find_references mcp__ariadne__blast_radius mcp__ariadne__coupling_report mcp__ariadne__weak_spots
disable-model-invocation: true
---

<purpose>
Adversarial review. Assume the implementer cut corners. Assume the plan was followed sloppily until proven otherwise. Produce a written verdict the user (and possibly Codex) will use to accept or reject the work.
</purpose>

<reviewer_stance>
Treat the diff as written by Codex or the user — never as own work. No defensiveness, no benefit of the doubt, no "I'll fix it later". Findings that would block the change in a strict code review get reported as `FAIL`; nits get reported as `INFO` and never gate the verdict. Bias toward catching false positives rather than missing real defects.
</reviewer_stance>

<non_negotiables>
- Input is `$ARGUMENTS` = path to `tier-NN-<name>.md` or single-tier `plan.md`. Reject any other invocation.
- Load only the tier file under review, its sibling `plan.md`, and the diff scoped to the tier's `<files>`. Other tier files belong to other audits.
- Verify, do not trust. Re-run every command in `<verification>`. Read every file in `<files>`. Compare to `<decisions>` and `<exit_criteria>`.
- Any claim about external API, library behavior, or framework semantics requires a citation. Re-fetch via Context7 → WebSearch fallback when the plan's citations are silent on the specific behavior under review.
- Verdict is `PASS` or `FAIL`. No "PASS with concerns". Concerns either block (FAIL) or do not (INFO).
- No findings for the sake of findings. A finding requires a concrete defect, risk, or stated-expectation violation — not personal preference, equivalent-style rewrites, or speculative future cleanup. Code health over perfection: an implementation that satisfies the plan and is safe ships, even when the reviewer would have written it differently [src: https://google.github.io/eng-practices/review/reviewer/standard.html]. If a comment cannot name a specific defect and locate it by file:line, drop it.
- Update `audit-state.json` truthfully. The commit/push hook depends on it; lying breaks the gate.
- Subagent spawning is permitted only when the user asks for it or when the audit scope explicitly requires isolated, parallel verification (e.g., long-running security scan). Single-threaded review is the default — orchestration overhead degrades quality at the margin.
</non_negotiables>

<code_intelligence>
Ariadne MCP is a read-only semantic graph of the current code (symbols, references, dependency edges). Prefer it over `grep`/`Read` for any symbol, reference, impact, or architecture question — one call where text search needs many and misses cross-file edges [src: CLAUDE.md "Ariadne code intelligence"; .mcp.json].
- Load: Ariadne tools may be schema-deferred when many MCP servers are connected (deferral triggers above ~10% of context). If a tool is not immediately callable, load it via `ToolSearch` `select:mcp__ariadne__<tool>` then retry [src: https://platform.claude.com/docs/en/agents-and-tools/tool-use/tool-search-tool].
- Freshness: call `mcp__ariadne__project_status` once before trusting the graph; if it reports stale or the server is down, fall back to `grep`/`Read` and state it [src: CLAUDE.md].
- Scope: the graph reflects the built code under review; after `spec-build` the `--watch` daemon should have re-indexed — `project_status` confirms before relying on edges.
- Use here: `coupling_report`/`weak_spots` to verify architecture matches `<decisions>`; `blast_radius`/`find_references` to confirm nothing outside `<files>` is affected and no dangling refs (steps 2, 3). Augments the evidence pass — never replaces reading the diff [src: CLAUDE.md].
</code_intelligence>

<inputs>
- `$ARGUMENTS`: path to tier or single-tier plan file.
- Plan files in the same `.claude/plans/<slug>/` directory.
- Repo state and `git diff`.
</inputs>

<workflow>
<step id="1" name="locate_and_scope">
Resolve `$ARGUMENTS`. Read it and sibling `plan.md`. Compute the scoped diff: `git diff` filtered to files listed in the tier's `<files>` plus any new files the build created. Reject if frontmatter unparseable or `status` is not `completed` (audit a not-yet-built tier is invalid).
</step>

<step id="2" name="checklist_build">
Construct an audit checklist by category. Each item is objectively checkable:
- plan_adherence: every `<files>` entry touched as intended; nothing outside the list (or explicitly justified).
- correctness: logic matches `<steps>`; edge cases the plan called out are handled.
- security: input validation, secret handling, authn/authz, injection, deserialization. Cite [OWASP Top 10](https://owasp.org/www-project-top-ten/) for any flagged item.
- performance: complexity, N+1 queries, sync I/O on hot paths, allocation in loops, cited budget violations.
- architecture: matches `<architecture>` and `<decisions>`; no smuggled tech.
- tests: present, asserting behavior not implementation; failure mode is loud.
- docs: `<verification>` is reproducible; in-code comments only where non-obvious.
- exit_criteria: every item in the tier's `exit_criteria` independently verified.
</step>

<step id="3" name="evidence_pass">
Walk the checklist:
- Read every changed file end-to-end. Do not skim.
- Re-run `<verification>` commands. Capture full output.
- For each library behavior under doubt, fetch the canonical doc (Context7 → WebSearch fallback). Pin the exact version in the citation.
- Compare diff to `<decisions>` and `<tech_inventory>`. Flag any smuggled dependency or pattern.
</step>

<step id="4" name="findings">
For each defect, produce one finding with: id, category, severity (`FAIL` | `INFO`), file:line(s), one-sentence problem, one-sentence fix, `[src: …]` when relying on external behavior. Severity rule:
- FAIL: violates a non-negotiable, an `exit_criterion`, a security/perf budget, or introduces undefined behavior.
- INFO: an actionable nit — a real but non-blocking defect the reviewer can name and locate by file:line. Pure taste, equivalent-style rewrites, or speculative cleanup are not findings; drop them entirely rather than logging them as INFO [src: https://google.github.io/eng-practices/review/reviewer/comments.html].
</step>

<step id="5" name="verdict_and_state">
Verdict = `PASS` if zero `FAIL` findings; else `FAIL`.
Write report to `.claude/plans/<slug>/audit/<tier-id-or-plan>-report.md`:
- Frontmatter: `tier_id`, `audited: <YYYY-MM-DD>`, `verdict`, `commit: <HEAD sha>`.
- Body XML sections: `<scope>`, `<checks_run>`, `<findings>` (table), `<verdict>`, `<next_steps>`, `<sources>`.
Update `.claude/plans/<slug>/audit-state.json`:
```json
{
  "slug": "<slug>",
  "audited_commit": "<HEAD sha>",
  "tier_id": "<tier-id or single>",
  "verdict": "PASS|FAIL",
  "audited_at": "<ISO-8601>",
  "report": "audit/<tier-id-or-plan>-report.md"
}
```
Create the file fresh per audit; do not append.
</step>

<step id="6" name="report_to_user">
Print: report path, verdict, count of FAIL/INFO findings, and the top 3 FAIL findings inline. On FAIL, name the specific tier steps to redo; do not fix the code here (that is `spec-build`'s job in a new session).
</step>
</workflow>

<gates>
- Refuse to start if `$ARGUMENTS` does not resolve to a tier or single-tier file with `status: completed`.
- Refuse to write a `PASS` verdict if any `<verification>` command fails when re-run.
- Refuse to weaken the audit by skipping categories.
</gates>

<output>
One `audit/<tier-id-or-plan>-report.md` and one updated `audit-state.json`. No fixes to the code. No edits to the plan or tier file beyond the implicit "audited" record in `audit-state.json`.
</output>

<anti_patterns>
- "Looks good to me" reports. State what was checked and how.
- Re-running only the tests the implementer wrote. Run the plan's `<verification>` in full.
- Treating own prior work as trusted. The session must behave as if the diff arrived from outside.
- Auto-fixing defects mid-audit. Audit reports; build fixes.
- Mass-spawning subagents for parallelism. Default is single-threaded; spawn only with cause.
- Citing "the docs" without a URL.
</anti_patterns>

<sources>
- [OWASP Top 10](https://owasp.org/www-project-top-ten/)
- [Skill authoring best practices — Anthropic](https://platform.claude.com/docs/en/agents-and-tools/agent-skills/best-practices)
- [Claude Code best practices](https://code.claude.com/docs/en/best-practices)
- [Claude Code hooks reference](https://code.claude.com/docs/en/hooks-guide)
</sources>
