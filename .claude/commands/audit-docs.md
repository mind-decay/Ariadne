# Audit Documentation

You are performing a comprehensive audit of Ariadne's design documentation for internal consistency, correctness, and (when code exists) code conformance.

## Input

Optional mode argument:
- `/audit-docs` — auto-detect (if `src/` exists with Rust code → both modes, else docs-only)
- `/audit-docs docs` — docs-only mode
- `/audit-docs code` — code conformance mode (requires `src/` to exist)

## Process

### Phase 1: Load Context & Detect Mode

Read `CLAUDE.md` to get the full list of design documents.

Check if `src/` exists and contains Rust source files:
- If yes (and mode is not `docs`) → include code conformance agents
- If no (and mode is `code`) → tell user no code exists yet, suggest `/audit-docs docs`

Read previous audit reports from `design/reports/` (files matching `*-doc-audit.md`). Note unresolved findings.

### Phase 2: Parallel Audit Agents

#### Always dispatched (docs-only):

**Agent 1 — Cross-Document Consistency**

You are auditing Ariadne's design documents for internal consistency.

Read ALL of these documents completely:
- `design/architecture.md`
- `design/ROADMAP.md`
- `design/error-handling.md`
- `design/performance.md`
- `design/testing.md`
- `design/path-resolution.md`
- `design/determinism.md`
- `design/distribution.md`

Cross-reference and check:

**architecture.md ↔ ROADMAP.md:**
- Do ROADMAP phases reference the correct deliverables from architecture.md?
- Do file paths in ROADMAP match architecture.md's file structure?
- Are Phase 2 algorithms consistent with architecture.md's algorithm descriptions?
- Do testing requirements in ROADMAP match architecture.md's testing section (if any)?

**architecture.md ↔ performance.md:**
- Do parallelism strategies reference the correct components?
- Do memory limits and thresholds agree?
- Are performance targets consistent (e.g., "1000+ files under 3 seconds" in ROADMAP vs performance.md targets)?

**architecture.md ↔ error-handling.md:**
- Do error codes (E001-E005) cover all failure points described in architecture.md?
- Do warning codes (W001-W009) match the graceful degradation paths in architecture.md?
- Are recovery strategies compatible with architecture.md's data flow?

**architecture.md ↔ testing.md:**
- Does testing.md cover all components described in architecture.md?
- Do fixture descriptions match architecture.md's supported languages and features?
- Are benchmark targets consistent?

**architecture.md ↔ path-resolution.md:**
- Does path normalization affect the graph model as described?
- Are edge cases consistent between the two docs?

**architecture.md ↔ determinism.md:**
- Does the determinism strategy account for all output formats in architecture.md?
- Are sort orders specified for all collections?

**performance.md ↔ testing.md:**
- Do benchmark targets agree?
- Are performance test strategies aligned with performance.md's model?

For EACH inconsistency found: cite both documents with specific sections, state what each says, and explain the conflict.

---

**Agent 2 — Decision Log Integrity**

You are auditing the integrity of Ariadne's architectural decision log.

Read:
- `design/decisions/log.md` (complete)
- `design/architecture.md`
- `design/ROADMAP.md`
- `design/error-handling.md`
- `design/performance.md`
- `design/testing.md`
- `design/path-resolution.md`
- `design/determinism.md`
- `design/distribution.md`

Check:

1. **Reference validity:** For each D-xxx in the log, are all file paths and section references valid?

2. **Implementation in docs:** For each decision, is it reflected in the relevant design documents?
   - D-001 (Rust + tree-sitter) → architecture.md
   - D-002 (trait-based parsers) → architecture.md parser section
   - D-003 (graceful degradation) → error-handling.md
   - ...continue for all decisions

3. **Missing decisions:** Scan all design documents for places where a choice was made without a D-xxx reference. Examples:
   - Specific algorithms chosen (Brandes, Tarjan, Louvain)
   - Output format choices (compact tuple JSON)
   - Specific hash algorithm (xxHash64)
   - Layer names and detection heuristics

4. **Contradictions:** Do any two decisions conflict with each other?

5. **Staleness:** Are any decisions based on assumptions that have since changed?

Output: decision verification matrix + missing decisions + contradictions + stale decisions.

---

**Agent 3 — Spec ↔ Design Alignment**

You are auditing that Ariadne's phase specs and plans accurately reflect the current design documents.

Read all files in `design/specs/`.
Read: `design/architecture.md`, `design/ROADMAP.md`, `design/decisions/log.md`

For EACH spec file:
1. Are all references to architecture.md still accurate? (file paths, type names, field names, enum values)
2. Do deliverables match what ROADMAP.md says for that phase?
3. Are D-xxx references valid and current?
4. Has any design doc been updated AFTER the spec was written, invalidating spec claims?

For EACH plan file:
1. Does the plan still match its spec? (if spec was updated after plan was written)
2. Are file paths in the plan still valid per architecture.md?
3. Are quoted values still accurate?

Output: alignment matrix (spec/plan → design doc → status: current/stale/wrong) + specific discrepancies.

#### Only dispatched in code conformance mode:

**Agent 4 — Code ↔ Design Conformance**

You are auditing that Ariadne's Rust implementation conforms to its design documents.

Read: `design/architecture.md`, `design/error-handling.md`, `design/performance.md`, `design/path-resolution.md`, `design/determinism.md`

Then read the Rust source code in `src/` and `tests/`. Also read `Cargo.toml`.

Check:

**Data Model Conformance:**
- Node struct fields match architecture.md's node definition (path, file_type, layer, arch_depth, lines, hash, exports, cluster)
- Edge struct fields match architecture.md's edge definition (from, to, edge_type, symbols)
- Enum variants for FileType match architecture.md (source, test, config, style, asset, type_def)
- Enum variants for EdgeType match architecture.md (imports, tests, re_exports, type_imports)
- Enum variants for Layer match architecture.md (api, service, data, util, component, hook, config, unknown)

**Parser Trait Conformance:**
- `LanguageParser` trait methods match architecture.md's trait definition
- Each language parser implements exactly the interface described
- Supported import patterns per language match architecture.md

**CLI Conformance:**
- Commands match ROADMAP.md / architecture.md (build, info, and Phase 2 commands if implemented)
- Output format matches architecture.md's JSON format specification

**Error Handling Conformance:**
- Error types in code match error-handling.md taxonomy (E001-E005)
- Warning types match W001-W009
- Recovery behavior matches documented strategies

**Path Resolution Conformance:**
- Normalization logic matches path-resolution.md
- Case sensitivity handling matches path-resolution.md

**Determinism Conformance:**
- Output sorting matches determinism.md
- No sources of non-determinism that determinism.md doesn't address

For EACH deviation: cite the design doc (file + section) and the code (file + line), state what design says vs what code does.

### Phase 3: Consolidation

After all agents return:

1. **Deduplicate** — same issue found by multiple agents = one finding
2. **Classify by severity:**
   - **Critical** — code behavior contradicts design (wrong types, missing error handling)
   - **High** — design docs contradict each other on concrete values
   - **Medium** — missing cross-references, incomplete coverage, stale specs
   - **Low** — cosmetic inconsistencies, documentation gaps
3. **Build fix plan** — for each finding:
   - ID, severity, title
   - Exact file paths and sections
   - What is wrong (current state)
   - What should be (target state)
   - Which file is the source of truth (design doc wins over code; architecture.md wins over other docs unless a D-xxx decision says otherwise)
   - Dependencies (if fix X must happen before fix Y)

### Phase 4: Output

Write the audit report to: `design/reports/{date}-doc-audit.md`

```
# Documentation Audit Report
**Date:** {date}
**Mode:** {docs-only | code-conformance | both}
**Documents audited:** {list}

## Summary
{total findings by severity, overall documentation health}

## Critical
{findings with fix plans}

## High
{findings with fix plans}

## Medium
{findings with fix plans}

## Low
{findings with fix plans}

## Fix Dependency Graph
{which fixes depend on which, suggested execution order}

## Parallel Fix Groups
{independent fixes that can be done simultaneously}
```

After writing the report, display a summary with the report path and ask:

> "Audit complete. {N} findings ({critical} critical, {high} high, {medium} medium, {low} low). Report written to `{path}`. Want me to apply the fixes?"

If the user confirms, execute fixes in dependency order:
1. Group independent fixes into parallel batches
2. Dispatch one agent per fix (or per logical cluster)
3. Each agent receives: finding ID, severity, file paths, fix description, and the rule "Make ONLY the specified fix. Do not improve surrounding code."
4. Design docs are updated FIRST, code changes SECOND
5. After all fixes: verify cross-references still hold
6. Do NOT commit — let the user review the diff

## Rules

- Every claim must cite specific file path and section
- Never guess — if uncertain, mark as "NEEDS VERIFICATION"
- Design docs are source of truth over code; architecture.md is primary over other design docs unless a D-xxx decision overrides
- Flag when design docs contradict each other (list both sides, don't pick one)
- Include previous audit reports in context if they exist in `design/reports/`
- Do NOT fix anything during the audit phase — only audit and plan
- Fixes happen only after user confirms
- One fix = minimal change. Do not refactor, do not improve, do not expand scope
- If a fix is ambiguous — skip it and flag for manual review
