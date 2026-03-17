# Review Implementation Plan

You are performing a rigorous review of an Ariadne phase implementation plan.

## Input

The user provides a path to a plan file (e.g., `design/specs/2026-03-17-phase1-implementation-plan.md`).

If no argument provided, find the most recently modified `design/specs/*-implementation-plan.md` file.

## Process

### Phase 1: Load Context

Before dispatching agents, read:
1. The plan file being reviewed
2. The corresponding spec (same date/phase prefix, without `-implementation-plan`)
3. `CLAUDE.md` — Phase Implementation Process section, commit message format, file structure

Extract from the plan: chunks, tasks, file paths, dependency graph.

### Phase 2: Parallel Review Agents (dispatch all in one message)

**Agent 1 — Spec Coverage Verification**

You are checking whether an implementation plan fully covers its spec.

Read the plan at: {plan_path}
Read the spec at: {spec_path}

Check:
- For EACH deliverable in the spec: is there at least one task in the plan that produces it?
- For EACH success criterion in the spec: does the plan include verification steps?
- Does the plan introduce work NOT in the spec? (scope creep)
- Does the plan skip any spec deliverable? (coverage gap)
- Does the plan's build order respect the spec's dependency structure?

Output: coverage matrix (spec deliverable → plan chunk/task) + gaps + scope creep items.

---

**Agent 2 — Accuracy & File Verification**

You are verifying the accuracy of file paths, values, and references in an implementation plan.

Read the plan at: {plan_path}
Read: `design/architecture.md`, `design/error-handling.md`, `design/performance.md`, `design/testing.md`

For EACH task that references files:
1. If the task references an EXISTING file — verify it exists at the specified path (use Glob/Read)
2. If the task creates a NEW file — verify the path matches architecture.md File Structure and CLAUDE.md File Structure
3. If the task quotes values from design docs (type names, field names, enum values, thresholds, error codes) — read the source and verify
4. If the task references Cargo.toml dependencies — verify crate names and feature flags are plausible

Also check:
- Are commit messages in the correct format (`ariadne(<scope>): <description>`) with valid scopes (core, parser, graph, detect, cli, ci, test, design)?
- Do chunk boundaries make sense (each chunk is independently committable)?
- Does each chunk end with `cargo test`?

Output: file verification results + incorrect references + commit message issues.

---

**Agent 3 — Design Rule Compliance**

You are checking an implementation plan against Ariadne's development rules.

Read the plan at: {plan_path}
Read: `CLAUDE.md`, `design/decisions/log.md`

CLAUDE.md compliance:
- [ ] Plan describes WHAT, not full code (no full file contents in the plan)
- [ ] Each chunk specifies: files to create/modify, design source, key implementation points
- [ ] Dependency graph included (which chunks depend on which)
- [ ] Each chunk has explicit dependencies listed
- [ ] Plan is additive over modifying (prefers new files to changing existing ones)

Decision log check:
- Does the plan contradict any decision (D-001 through D-009)?
- Does the plan make implicit decisions? (choosing between approaches, defining new types/constants not in design docs, inventing field names not in architecture.md)
- If the plan adds types, traits, or fields — are they justified by architecture.md or do they need a decision?

Read the relevant design docs referenced by tasks and verify:
- Each "key point" in the plan actually comes from the cited design doc
- No design doc changes are needed that the plan doesn't mention

Output: rule compliance checklist + implicit decisions found + design doc verification.

---

**Agent 4 — Dependency & Order Analysis**

You are analyzing the dependency structure and execution order of an implementation plan.

Read the plan at: {plan_path}
Read the spec at: {spec_path}

Analyze:

1. **Dependency graph:** Map chunk → depends-on-chunks. Check for:
   - Circular dependencies
   - Missing dependencies (chunk uses something built in another chunk but doesn't list it as dependency)
   - Over-constrained dependencies (chunk claims dependency that isn't actually needed)

2. **Testability:** Can each chunk be tested in isolation?
   - If a chunk depends on unbuilt components, does the plan include stubs/mocks?
   - Does each chunk produce something `cargo test` can verify?

3. **Parallelism:** Which chunks could theoretically run in parallel? Are these correctly identified?

4. **Build order:** Is there a valid topological order through the dependency graph? What is the critical path (longest chain)?

5. **Incremental value:** Does each chunk deliver independently useful progress, or are there chunks that produce nothing usable until later chunks complete?

Output: dependency graph analysis + critical path + parallelism opportunities + testability issues.

### Phase 3: Consolidation

After all 4 agents return, synthesize:

```
# Plan Review: {plan file name}

## Verdict: {APPROVE | APPROVE WITH CHANGES | NEEDS REVISION}

## Summary
{1-3 sentences: coverage quality, biggest concerns}

## Spec Coverage
### Covered
{deliverables with matching chunks/tasks}
### Gaps
{spec deliverables without plan tasks}
### Scope Creep
{plan tasks not justified by spec}

## Accuracy
### Verified References
{correct file paths, values, quotes}
### Incorrect References
{wrong paths, stale values, naming mismatches}

## Design Compliance
{rule violations, implicit decisions}

## Dependency Analysis
{dependency graph issues, critical path, parallelism, testability}

## Recommended Changes
{numbered list of specific changes needed before approval}
```

Display the review directly to the user (do NOT write to a file unless asked).

## Rules

- The #1 source of implementation bugs is wrong values copied into plans. Verify EVERY value against its source design doc.
- Plans that make design decisions are the #2 source of bugs. If the plan says "we'll use X approach" without a D-xxx reference — it's making a decision.
- Be strict on accuracy and coverage. Be lenient on style/formatting.
- Every finding must cite specific file and section.
- Do NOT suggest improvements beyond correctness — only verify the plan is accurate, complete, and compliant.
