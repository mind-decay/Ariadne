---
name: spec-plan
description: Produces an evidence-backed, tiered implementation plan under `.claude/plans/<slug>/` so a separate `spec-build` session can execute it without ambiguity. Use when the user asks to "plan a feature", "spec a change", "produce a plan", "design before building", or invokes `/spec-plan`. Runs an alignment loop, fetches latest docs for every tech in scope, and refuses to write until the user gives explicit "go".
when_to_use: A new feature, refactor, or non-trivial change. Not for one-line fixes; not for executing an existing plan (use spec-build) or reviewing built code (use spec-audit).
allowed-tools: Read Write Edit Bash Glob Grep WebFetch WebSearch mcp__claude_ai_Context7__resolve-library-id mcp__claude_ai_Context7__query-docs AskUserQuestion TaskCreate TaskUpdate TaskGet TaskList mcp__ariadne__project_status mcp__ariadne__list_symbols mcp__ariadne__find_definition mcp__ariadne__find_references mcp__ariadne__search_code mcp__ariadne__read_symbol mcp__ariadne__read_outline mcp__ariadne__file_summary mcp__ariadne__blast_radius mcp__ariadne__plan_assist mcp__ariadne__diff_blast_radius mcp__ariadne__fitness_report mcp__ariadne__coupling_report mcp__ariadne__weak_spots mcp__ariadne__refactor_suggestions mcp__ariadne__hotspots mcp__ariadne__complexity mcp__ariadne__co_change mcp__ariadne__doc_for mcp__ariadne__doc_for_module mcp__ariadne__doc_for_project
disable-model-invocation: true
---

<purpose>
Output a plan that survives hostile review by the user and possibly Codex. Every decision is sourced (docs/code/research). Every tier is independently executable in a fresh Claude session by `spec-build`. Length is the loser; precision wins.
</purpose>

<non_negotiables>
- One plan per invocation, written to `.claude/plans/<slug>/plan.md`. Multi-tier plans add `tier-NN-<name>.md` siblings.
- Each file ≤200 lines (project rule). Split into more tiers before bloating one file.
- Every external technology, library, framework, or service is verified this session via Context7 (`resolve-library-id` → `query-docs`); fallback to `WebSearch` + `WebFetch` only when Context7 has no entry or quota is exhausted. No syntax, flag, or API from training-data recall.
- Every architectural decision carries an inline `[src: …]` pointing to documentation, an authoritative article, a research paper, or repo code with line number. Decisions without a source are deleted, not "soft-stated".
- Body is XML-structured ([Anthropic prompting best practices](https://platform.claude.com/docs/en/build-with-claude/prompt-engineering/claude-prompting-best-practices)).
- Architectural lens is fixed: scalability, reliability, efficiency, maintainability. Speed of delivery is not a tradeoff axis here.
- Plans assume hostile review. Anticipate the question "why this and not X?" for every non-trivial choice and answer it inline with a citation.
</non_negotiables>

<code_intelligence>
Ariadne MCP is a read-only semantic graph of the current code (symbols, references, dependency edges). Prefer it over `grep`/`Read` for any symbol, reference, impact, or architecture question — one call where text search needs many and misses cross-file edges [src: CLAUDE.md "Ariadne code intelligence"; .mcp.json].
- Load: Ariadne tools may be schema-deferred when many MCP servers are connected (deferral triggers above ~10% of context). If a tool is not immediately callable, load it via `ToolSearch` `select:mcp__ariadne__<tool>` then retry [src: https://platform.claude.com/docs/en/agents-and-tools/tool-use/tool-search-tool].
- Freshness: call `mcp__ariadne__project_status` once before trusting the graph; if it reports stale or the server is down, fall back to `grep`/`Read` and state it [src: CLAUDE.md].
- Scope: the graph covers code that exists now. For symbols a tier will create (greenfield), Ariadne has nothing — use it for what exists, not what the plan adds.
- Use here: `plan_assist` + `blast_radius`/`diff_blast_radius` to scope impact and integration boundaries (step 2); `coupling_report`/`weak_spots`/`refactor_suggestions` plus `fitness_report` and `hotspots`/`complexity`/`co_change` to ground architecture decisions in structure, the project's `ariadne-fitness.toml` rules, and Git risk; `search_code`/`read_symbol`/`read_outline`/`file_summary`/`find_references`/`list_symbols`/`find_definition` to map touched code; `doc_for`/`doc_for_module`/`doc_for_project` to summarize the area being changed (steps 2, 4) [src: CLAUDE.md; ariadne MCP tool catalog].
</code_intelligence>

<inputs>
- `$ARGUMENTS` (optional): a free-form description of what to plan.
- Repo state: `git status`, existing modules, existing `.claude/plans/` for slug collisions.
</inputs>

<workflow>
<step id="1" name="intake_and_slug">
Read `$ARGUMENTS`. Derive a kebab-case slug (≤40 chars). If `.claude/plans/<slug>/` already exists, ask whether to revise, branch (`<slug>-v2`), or abort.
</step>

<step id="2" name="alignment_loop">
Maintain this readiness checklist; refuse to draft until every item is resolved. Ask via `AskUserQuestion` (≤4 per batch, with a recommended option leading):
- [ ] Problem statement in one sentence; success in one measurable sentence.
- [ ] In-scope and explicit out-of-scope.
- [ ] Hard constraints (perf budgets, security/compliance, deadlines, deployment targets, language/runtime versions).
- [ ] Existing systems this touches; integration boundaries; data ownership.
- [ ] Non-functional requirements: throughput, latency p95/p99, RPO/RPTO, concurrency, multi-tenant?
- [ ] Tech stack candidates (drives the doc-fetch list). Capture rejected candidates too.
- [ ] Risks and unknowns; what proof-of-concept or spike is needed before commit?
- [ ] Tier-cut hypothesis: how the work splits into independently-executable Claude sessions.
When checklist is full, print a 6-line summary and ask explicit "go" before any Write.
</step>

<step id="3" name="evidence_fetch">
Build a `tech_inventory` from step 2 (every lib, framework, runtime, service). For each entry:
1. `mcp__claude_ai_Context7__resolve-library-id` then `query-docs` for the exact API/pattern the plan relies on. Pin a version when one is fetched.
2. Fallback to `WebSearch` (canonical docs domain) + `WebFetch` when Context7 has no match or returns quota-exceeded.
3. For architectural decisions (e.g., DB choice, queue choice, consistency model), cite primary sources: vendor docs, peer-reviewed papers, well-known engineering posts (cite the author/org and date).
Discard any fact, flag, or syntax that lacks a citation. Note unresolved questions explicitly in the plan rather than guessing.
</step>

<step id="4" name="tier_design">
Decide single-tier vs multi-tier using these rules:
- A tier is executable end-to-end in one fresh Claude session of `spec-build`. If a tier needs >~200 lines of plan or crosses an integration boundary, split it.
- Tiers have explicit `deps` (`tier_id` of prerequisites). No cycles.
- Tier boundaries align with verifiable milestones (failing test → passing test, or a self-contained refactor that leaves the build green).
- Each tier has `exit_criteria` checkable without human interpretation.
</step>

<step id="5" name="draft">
Compose files in memory:

`plan.md` frontmatter:
```yaml
---
slug: <slug>
title: <one line>
created: <YYYY-MM-DD>
owners: [user, claude]
review: [user, codex?]
single_tier: <true|false>
tiers: [tier-01-<name>, tier-02-<name>, ...]   # omit when single_tier
---
```

`plan.md` body sections (XML):
- `<context>` — problem, scope, out-of-scope.
- `<constraints>` — hard constraints with `[src: …]`.
- `<decisions>` — each decision: choice, rejected alternatives, rationale, `[src: …]`.
- `<architecture>` — diagram-in-text or component list with responsibilities.
- `<tech_inventory>` — table: tech, version pinned, doc URL fetched this session.
- `<risks>` — risk, likelihood, mitigation, owner.
- `<verification>` — how the whole feature is proven done (tests, metrics, SLOs).
- `<sources>` — canonical refs.

`tier-NN-<name>.md` frontmatter:
```yaml
---
tier_id: tier-NN
title: <one line>
deps: [tier-MM, ...]   # empty list when no deps
exit_criteria:
  - <objective check>
  - <objective check>
status: pending   # pending | in_progress | completed | blocked
---
```

`tier-NN-<name>.md` body sections (XML):
- `<context>` — minimal slice the executor needs; link `plan.md` for full context.
- `<files>` — exact paths to create/modify, with one-line intent per file.
- `<steps>` — ordered, imperative steps. Each step references a `[src: …]` when it relies on external API/syntax.
- `<verification>` — commands to run; expected outcomes; how to fail loudly.
- `<rollback>` — how to revert if mid-tier failure.
</step>

<step id="6" name="self_validate">
Before Write:
- `wc -l` ≤200 per file.
- XML tags balanced.
- Every external API/flag mentioned has a `[src: …]` resolved in step 3.
- Tier `deps` form a DAG; each tier is independently startable from its deps.
- `exit_criteria` are objectively checkable (no "looks good").
- No vague verbs ("handle", "support") without specifics.
If any check fails, fix and re-validate.
</step>

<step id="7" name="write_and_report">
Write all files. Report:
- Paths created.
- Tier execution order.
- The exact `spec-build` invocation per tier (e.g., `/spec-build .claude/plans/<slug>/tier-01-foundations.md`).
- Open questions deferred to build/audit.
</step>
</workflow>

<gates>
- Refuse to draft past step 2 without explicit "go".
- Refuse to write files if step 6 fails.
- Refuse to include any fact lacking a citation from step 3.
</gates>

<output>
`.claude/plans/<slug>/plan.md` plus zero or more `tier-NN-<name>.md`. No companion docs, no scratchpads.
</output>

<anti_patterns>
- Wishful timelines or estimates. The plan describes work, not schedule.
- "We'll use Redis" without naming version, deployment topology, eviction policy, and a citation for each.
- Soft language: "consider", "maybe", "should probably". Decide or defer explicitly with a `<risks>` entry.
- Tiers that depend on Claude remembering session state. Each tier is self-contained.
- One-tier plans that bury 5 independent migrations together — split.
</anti_patterns>

<sources>
- [Prompting best practices (XML tags) — Anthropic](https://platform.claude.com/docs/en/build-with-claude/prompt-engineering/claude-prompting-best-practices)
- [Skill authoring best practices — Anthropic](https://platform.claude.com/docs/en/agents-and-tools/agent-skills/best-practices)
- [Extend Claude with skills — Claude Code docs](https://code.claude.com/docs/en/skills)
</sources>
