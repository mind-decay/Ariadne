# Review Phase Spec

You are performing a rigorous review of an Ariadne phase specification.

## Input

The user provides a path to a spec file (e.g., `design/specs/2026-03-17-phase1-core-cli.md`).

If no argument provided, find the most recently modified `design/specs/*-phase*` file that does NOT end in `-implementation-plan.md`.

## Process

### Phase 1: Load Context

Before dispatching agents, read these files yourself:
1. The spec file being reviewed
2. `design/ROADMAP.md`
3. `design/architecture.md`
4. `design/decisions/log.md`
5. `CLAUDE.md` — Phase Implementation Process section

Extract from the spec: which phase, what design docs it references, what deliverables it claims.

### Phase 2: Parallel Review Agents (dispatch all in one message)

**Agent 1 — Design Source Verification**

You are verifying that a phase spec accurately references Ariadne's design documents.

Read the spec file at: {spec_path}

Then read EACH design document the spec references. Also read these regardless:
- `design/architecture.md`
- `design/ROADMAP.md`
- `design/decisions/log.md`
- `design/error-handling.md`
- `design/performance.md`
- `design/testing.md`
- `design/path-resolution.md`
- `design/determinism.md`

For EACH claim the spec makes from a design doc:
- Verify every quoted value: numbers, thresholds, enum values, type names, field names, file paths
- Check: does architecture.md actually define this struct/trait/type as the spec claims?
- Check: do performance.md thresholds match what the spec states?
- Check: do error-handling.md error codes match?

Then check for UNCOVERED sections:
- Are there sections of architecture.md relevant to this phase that the spec IGNORES?
- Are there relevant decisions in log.md the spec doesn't reference?
- Are there error handling requirements the spec misses?

Decision log check:
- Does the spec contradict any existing decision (D-001 through D-009)?
- Does the spec make implicit decisions that should have a D-xxx entry?
- Are all D-xxx references in the spec valid?

Output: list of verified claims ("VERIFIED" or "WRONG: spec says X, design doc says Y") and uncovered design doc sections.

---

**Agent 2 — Completeness & Scope Audit**

You are checking a phase spec for completeness and scope compliance.

Read the spec file at: {spec_path}
Read: `CLAUDE.md`, `design/ROADMAP.md`

Required spec sections (check each):
- [ ] Goal clearly stated (matches ROADMAP.md)
- [ ] Dependencies listed (previous phases required)
- [ ] Risk classification (overall + per-deliverable with GREEN/YELLOW/ORANGE/RED)
- [ ] Deliverables listed (concrete file paths matching architecture.md File Structure)
- [ ] Design sources mapped (each deliverable → authoritative doc section)
- [ ] Success criteria defined (concrete, verifiable)
- [ ] Testing requirements specified

Scope checks:
- Does the spec stay within its phase boundaries per ROADMAP.md?
- Does it depend on phases not yet completed?
- Does it include deliverables that belong to a different phase?
- Are all file paths consistent with the File Structure in CLAUDE.md and architecture.md?

Output: completeness checklist results + scope issues.

---

**Agent 3 — Cross-Phase Impact Analysis**

You are analyzing how this phase spec interacts with other phases and the overall architecture.

Read the spec file at: {spec_path}
Read: `design/ROADMAP.md`, `design/architecture.md`
Read all existing specs in `design/specs/`

Analyze:

1. **Backward compatibility:** Does this phase's output remain compatible with what previous phases built? Does it extend or modify data structures from earlier phases?

2. **Forward compatibility:** Do any design choices in this spec close doors for future phases described in ROADMAP.md? For example:
   - Does the data model support Phase 2 algorithms?
   - Are interfaces extensible for future language support?
   - Will the output formats accommodate future enrichment?

3. **Architecture alignment:** Does the spec's decomposition match architecture.md's module boundaries? Are there deliverables that cross module boundaries awkwardly?

4. **Integration surface:** If this phase produces outputs consumed by later phases — are those outputs well-defined enough for consumers to depend on?

Output: cross-phase impact assessment + compatibility issues + architecture alignment issues.

### Phase 3: Consolidation

After all 3 agents return, synthesize findings:

```
# Spec Review: {spec file name}

## Verdict: {APPROVE | APPROVE WITH CHANGES | NEEDS REVISION}

## Summary
{1-3 sentences: overall quality, biggest concerns}

## Design Source Verification
### Verified Claims
{list of correct references}
### Incorrect Claims
{list with: what spec says → what design doc actually says}
### Uncovered Design Sections
{design doc sections relevant but not addressed by spec}

## Completeness
{checklist results}

## Scope Issues
{out-of-scope items, missing dependencies, phase boundary violations}

## Cross-Phase Impact
{forward/backward compatibility issues, architecture alignment}

## Recommended Changes
{numbered list of specific changes needed before approval}
```

Display the review directly to the user (do NOT write to a file unless asked).

## Rules

- Be strict. A spec that goes to implementation with wrong numbers or missing cross-references causes expensive rework.
- Every finding must cite specific file and section.
- "Looks fine" is not an acceptable review for any section — verify concretely.
- If the spec makes an architectural decision without a D-xxx reference, flag it as "IMPLICIT DECISION — needs decision log entry."
- Do NOT suggest improvements beyond the spec's stated scope — only verify correctness and completeness.
- Design documents are the source of truth, not the spec.
