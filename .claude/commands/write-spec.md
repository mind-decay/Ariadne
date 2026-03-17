# Write Phase Spec

You are generating a phase specification for the Ariadne project, following the Phase Implementation Process defined in CLAUDE.md (Step 1).

You do NOT make design decisions. If you find gaps in design docs, surface them as Discussion Points for the user.

## Input

The user provides a phase number or name (e.g., `/write-spec phase2` or `/write-spec "Algorithms & Queries"`).

If no argument provided, determine the next unimplemented phase from `design/ROADMAP.md` by checking which phases already have specs in `design/specs/`.

## Process

### Phase 1: Load Context

Read these files yourself before dispatching agents:
1. `design/ROADMAP.md` — phases and deliverables
2. `design/architecture.md` — full system design
3. `design/decisions/log.md` — all architectural decisions
4. `design/error-handling.md` — error taxonomy
5. `design/performance.md` — performance model
6. `design/testing.md` — testing strategy
7. `design/path-resolution.md` — path normalization
8. `design/determinism.md` — determinism strategy
9. `CLAUDE.md` — Phase Implementation Process section
10. All existing specs in `design/specs/` — to understand dependencies and what's already covered

Identify the target phase and extract its section from ROADMAP.md.

### Phase 2: Parallel Agents (dispatch all in one message)

**Agent 1 — Requirements Extraction**

You are extracting detailed requirements for a phase spec from Ariadne's design documents.

Read ALL of:
- `design/ROADMAP.md`
- `design/architecture.md`
- `design/performance.md`
- `design/testing.md`
- `design/error-handling.md`
- `design/path-resolution.md`
- `design/determinism.md`

For phase "{phase}":

1. Extract every deliverable mentioned in ROADMAP.md for this phase
2. For EACH deliverable, find the corresponding section in architecture.md and extract:
   - Exact file paths where it should live (per File Structure in architecture.md)
   - Data types, structs, traits, enums involved
   - Interfaces with other components
   - Constraints from performance.md, error-handling.md, determinism.md, path-resolution.md
3. Extract testing requirements from testing.md relevant to this phase
4. List all design documents that are authoritative sources for this phase

Output: structured list of deliverables with full detail from design docs.

---

**Agent 2 — Dependency & Risk Analysis**

You are analyzing dependencies and risks for a phase spec.

Read ALL of:
- `design/ROADMAP.md`
- `design/architecture.md`
- `design/error-handling.md`
- `design/performance.md`
- All existing specs in `design/specs/`

For phase "{phase}":

1. **Prerequisites:** What must exist before this phase can start? Check previous phase specs — are all prerequisites delivered?
2. **Internal dependencies:** Between deliverables within this phase — what order must they be built?
3. **Risk classification** for each deliverable:
   - **GREEN** — well-defined in design docs, straightforward implementation
   - **YELLOW** — defined but involves complexity (algorithms, multi-language support, edge cases)
   - **ORANGE** — partially defined, some design gaps that need resolution
   - **RED** — modifies existing critical components, touches core data model, or has unclear requirements
4. **Overall phase risk:** GREEN/YELLOW/ORANGE/RED with justification

Output: dependency graph + risk matrix with justification for each classification.

---

**Agent 3 — Design Gap Detection**

You are looking for gaps in design documentation that would block implementation of a phase.

Read ALL of:
- `design/ROADMAP.md`
- `design/architecture.md`
- `design/decisions/log.md`
- `design/error-handling.md`
- `design/performance.md`
- `design/testing.md`
- `design/path-resolution.md`
- `design/determinism.md`

For phase "{phase}":

1. For each deliverable, check: is the behavior fully specified in design docs?
   - Are interfaces clear enough to implement without guessing?
   - Are edge cases addressed?
   - Are error conditions defined?
2. Are there implicit decisions — places where the design assumes a choice without documenting it?
3. Are there contradictions between design docs for this phase's scope?
4. What questions would an implementer have that the design docs don't answer?

Output: list of gaps, each with: what's missing, which design doc should address it, suggested resolution direction (but NOT the decision itself).

### Phase 3: Consolidation

Assemble the spec following the existing pattern from `design/specs/2026-03-17-phase1-core-cli.md`:

```
# Phase N: {Name} — Specification

## Goal
{One clear sentence from ROADMAP.md}

## Dependencies
{What must be complete before this phase starts}

## Risk Classification
**Overall: {COLOR}**
{Brief justification}

### Per-Deliverable Risk
| Deliverable | Risk | Rationale |
|------------|------|-----------|
| ... | ... | ... |

## Deliverables
{Numbered list with exact file paths and descriptions}

## Design Sources
{Map each deliverable to its authoritative design document section}

## Success Criteria
{Concrete, verifiable conditions that prove the phase is complete}

## Testing Requirements
{From testing.md, specific to this phase}

## Discussion Points
{Design gaps found by Agent 3 — questions for the user, NOT answers}
```

### Phase 4: Output

Write the spec to: `design/specs/{date}-phase{N}-{name}.md` where date is today's date in YYYY-MM-DD format.

Display a summary to the user:
- Phase name and overall risk
- Number of deliverables
- Key discussion points that need resolution before implementation planning

## Rules

- Design documents are the source of truth. Extract, don't invent.
- Every claim in the spec must trace to a specific design document.
- If a deliverable is mentioned in ROADMAP.md but not detailed in architecture.md — flag it as a gap, don't fill it in.
- Do NOT write the implementation plan. That's a separate step (Step 2 in CLAUDE.md).
- Do NOT make architectural decisions. Surface gaps for user decision.
- Follow the existing spec format from previous phases in `design/specs/`.
- Previous specs are context — don't duplicate their deliverables unless explicitly part of this phase.
