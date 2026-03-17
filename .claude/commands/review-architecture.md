# Review Architecture

You are performing a deep architectural review of the Ariadne system. Unlike `/audit-docs` (which checks consistency between documents and code) or `/review-spec`/`/review-plan` (which check specs/plans match design), YOUR job is to question the DESIGN ITSELF.

You are an architectural critic. You look for:
- Design smells and structural weaknesses
- Inconsistencies between design documents
- Questionable decisions lacking justification
- Over-engineering and unnecessary complexity
- Under-engineering and missing abstractions
- Gaps, blind spots, and unaddressed failure modes
- Implicit assumptions that should be explicit

## Input

Optional focus argument: a specific area to concentrate on (e.g., "parser trait", "error handling", "graph model", "clustering", "path resolution", "determinism").

If no argument: perform a full-system architectural review.

## Process

### Phase 1: Load Context & Detect Mode

Read these files yourself before dispatching agents:
1. `design/architecture.md`
2. `design/ROADMAP.md`
3. `design/decisions/log.md`
4. Previous architecture reviews from `design/reports/` (if any — check for resolved/unresolved issues)

**Detect mode:** Check if `src/` exists and contains Rust source files.
- If yes → **post-implementation mode** (review design docs AND code)
- If no → **pre-implementation mode** (review design docs only)

If a focus area was specified, identify which design documents are most relevant. If no focus, all documents are in scope.

### Phase 2: Parallel Architecture Review (dispatch all in one message)

**Agent 1 — Structural Integrity Analysis**

You are an architecture reviewer analyzing structural soundness of the Ariadne system design.

Read ALL of:
- `design/architecture.md`
- `design/ROADMAP.md`
- `design/path-resolution.md`
- `design/determinism.md`
- `design/performance.md`

{If focus specified: concentrate on sections related to "{focus}", but still read architecture.md fully for context.}

{If post-implementation mode: Also read the Rust source in `src/` — focus on module structure, `pub` boundaries, and `mod.rs` / `lib.rs` exports.}

Analyze and report on:

**Layering & Dependencies**
- Are the 4 modules (parser, graph, detect, hash) cleanly separated? Any leaky abstractions?
- Does the `LanguageParser` trait define a minimal, sufficient interface?
- Is the dependency direction consistent? (parser → graph model, detect → graph model, CLI → everything else)
- Are there circular conceptual dependencies between subsystems?
- {Post-impl: Do `use` statements confirm the intended dependency direction? Any unexpected cross-module imports?}

**Decomposition Quality**
- Is granularity consistent? (e.g., are some parsers over-specified while others are vague?)
- Are responsibility boundaries clean between parser/detect/graph/CLI?
- Is any component a hidden god object — accumulating too much responsibility?
- Are there missing components — gaps where work falls through?
- {Post-impl: Are modules appropriately sized? Any file over 500 lines that should be split?}

**Abstraction Assessment**
- Is the `LanguageParser` trait at the right abstraction level? Too broad? Too narrow?
- Is the node/edge data model flexible enough for Phase 2 algorithms without being over-engineered for Phase 1?
- Are the 7 architectural layers justified, or could fewer suffice?
- Are the 6 file types sufficient, or are there common file types that don't fit?
- {Post-impl: Are generic type parameters used appropriately? Any over-abstracted traits?}

**Symmetry & Consistency**
- Are similar things handled similarly across all 6 language parsers?
- Are there asymmetries that suggest a design inconsistency?
- Is naming consistent throughout (Rust naming conventions)?

For each finding: explain what the issue is, why it matters, and suggest a direction (not a full solution). Cite specific file paths and sections.

---

**Agent 2 — Design Coherence & Decision Audit**

You are an architecture reviewer checking whether Ariadne's design documents form a coherent whole.

Read ALL of:
- `design/decisions/log.md` (COMPLETE — all decisions D-001 through D-009)
- `design/architecture.md`
- `design/error-handling.md`
- `design/performance.md`
- `design/testing.md`
- `design/path-resolution.md`
- `design/determinism.md`
- `design/distribution.md`

{If focus specified: concentrate on documents related to "{focus}", but still read the full decision log.}

{If post-implementation mode: Also read key source files to verify decisions are actually followed in code.}

Analyze and report on:

**Decision Quality**
- For each decision D-001 through D-009: is the rationale convincing? Are rejected alternatives genuinely inferior?
- Are there decisions that SHOULD exist but don't? (important choices made implicitly)
- Are any decisions outdated — made with assumptions that no longer hold?
- Do any decisions contradict each other?

**Cross-Document Consistency**
- When two documents describe the same concept, do they agree on specifics?
  - architecture.md node types vs error-handling.md error contexts
  - architecture.md edge types vs what parsers must extract
  - performance.md thresholds vs testing.md benchmark requirements
  - path-resolution.md normalization vs determinism.md output stability
- Are there concepts in one document absent from documents that should reference them?

**Assumption Audit**
- What implicit assumptions underlie the design? Examples:
  - "tree-sitter grammars are always available and correct"
  - "file paths are always valid UTF-8"
  - "import resolution doesn't require type information"
  - "directory-based clustering is meaningful for all project structures"
- Which assumptions are fragile — likely to break in practice?
- Which are undocumented?

**Design Completeness**
- Are error conditions specified for every interface?
- Are edge cases documented for every algorithm?
- Are performance characteristics stated for every operation?

For each finding: the inconsistency or concern, the specific documents involved (with file paths and sections), and the potential impact.

---

**Agent 3 — Complexity & Trade-off Analysis**

You are an architecture reviewer evaluating whether Ariadne's complexity is justified.

Read ALL of:
- `design/architecture.md`
- `design/ROADMAP.md`
- `design/performance.md`
- `design/error-handling.md`
- `design/testing.md`
- `design/path-resolution.md`
- `design/determinism.md`

{If focus specified: concentrate on sections related to "{focus}", but still read architecture.md for scope.}

{If post-implementation mode: Also read source code to check actual vs designed complexity. Look for dead code, unused generics, over-abstracted interfaces.}

Analyze and report on:

**Complexity Budget**
- What is the total complexity of this system? Is it proportional to the problem (parsing imports + building a graph)?
- Identify the TOP 5 most complex aspects. For each: essential (problem is inherently hard) or accidental (design choice made it hard)?
- Are there simpler alternatives that achieve 80% of the benefit?

**Over-Engineering Signals**
- Features designed but unlikely to be used
- Configuration that will realistically never change from defaults
- Abstractions built for future flexibility that may never be needed
- The 7 algorithms in Phase 2 — are all essential, or could some be deferred?
- {Post-impl: Generic parameters never instantiated with more than one type? Trait objects that could be concrete types?}

**Under-Engineering Signals**
- Areas where the design hand-waves important details
- Components whose design is thin relative to their importance
- Error paths under-specified compared to happy paths
- Missing test strategies for specific components

**Trade-off Transparency**
- For each major design choice: what was gained and what was lost?
- Is the tree-sitter-only approach's trade-off acknowledged? (fast + deterministic, but limited to syntactic imports)
- Is the compact JSON format's trade-off documented? (space efficient, but harder to debug)

**YAGNI Assessment**
- Which Phase 1 deliverables are essential?
- Which could be deferred without blocking Phase 2?
- Is the 6-language Tier 1 set justified for Phase 1, or could fewer languages suffice initially?

For each finding: the issue, complexity impact, and whether to simplify, defer, or keep.

---

**Agent 4 — Robustness & Gap Analysis**

You are an architecture reviewer looking for failure modes, edge cases, and gaps in the Ariadne design.

Read ALL of:
- `design/architecture.md`
- `design/error-handling.md`
- `design/performance.md`
- `design/testing.md`
- `design/path-resolution.md`
- `design/determinism.md`

{If focus specified: concentrate on sections related to "{focus}", but still read error-handling.md for context.}

{If post-implementation mode: Also audit the Rust source for `unwrap()`, `expect()`, `panic!()`, unchecked error propagation, and missing error handling on I/O operations.}

Analyze and report on:

**Failure Mode Coverage**
- The error taxonomy (E001-E005, W001-W009): are there failure modes NOT covered? Think about:
  - Tree-sitter grammar unavailable or crashing on specific input
  - Circular symlinks in project directories
  - Files that change during scanning (TOCTOU)
  - Binary files misidentified as source
  - Encoding issues (non-UTF-8 source files)
  - Filesystem permission errors mid-scan
  - Memory exhaustion on very large projects
- For each covered error: is the recovery strategy realistic?

**Edge Cases**
- Empty project (0 files)
- Single file project
- Massive project (100k+ files)
- Deeply nested directories (100+ levels)
- Files with no imports and no exports
- Circular imports (A→B→A)
- Self-imports
- Import paths that resolve to nonexistent files
- Mixed language project where Language A imports Language B's output
- Monorepo with multiple independent subprojects

**Determinism Threats**
- What could cause non-deterministic output beyond what determinism.md addresses?
- File system ordering differences between OS
- Timestamp-dependent behavior
- Hash collisions
- Concurrent file modifications

**Performance Cliffs**
- Where might performance degrade non-linearly?
- What happens when the graph doesn't fit in memory?
- Are there O(n²) or worse algorithms hidden in the design?
- Path resolution for deeply nested relative imports

**{Post-impl: Rust-Specific Safety}**
- `unwrap()` / `expect()` on fallible operations in non-test code
- Missing error propagation (should use `?` but doesn't)
- Unbounded allocations (Vec growing without limit)
- Missing timeouts on I/O operations

For each finding: the gap/risk, likelihood and impact (high/medium/low), and a mitigation direction.

### Phase 3: Consolidation & Synthesis

After all 4 agents return:

1. **Cross-reference findings** — multiple agents flagging the same area = high-confidence issue
2. **Prioritize** by architectural impact:
   - **Foundational** — affects the core data model or trait design; changing later is very expensive
   - **Structural** — affects module boundaries or interfaces; moderate cost to change
   - **Surface** — affects details within a module; relatively cheap to change
3. **Synthesize themes** — group related findings into coherent discussion topics

### Phase 4: Output

Write the review to: `design/reports/{date}-architecture-review.md`

Use this structure:

```
# Architecture Review
**Date:** {date}
**Focus:** {focus area or "Full System"}
**Mode:** {pre-implementation | post-implementation}
**Reviewed:** {list of documents read}

## Executive Summary
{3-5 sentences: overall architectural health, top concerns, strongest aspects}

## Key Themes
{2-4 overarching themes that emerged across multiple agents' findings}

### Theme 1: {name}
{description, contributing findings, why it matters}

### Theme 2: {name}
...

## Detailed Findings

### Foundational Issues
{issues that affect the core model — highest priority}

### Structural Issues
{issues that affect module boundaries — moderate priority}

### Surface Issues
{issues within modules — lower priority}

## Discussion Points
{questions without clear answers — need user input}

Each question should include:
- The tension or trade-off involved
- Arguments for each side
- What's at stake
- Recommended direction (if one exists)

## Strengths
{what the architecture gets RIGHT — acknowledge good design}

## Recommendations

### Quick Wins (doc updates only)
{things fixable by clarifying or correcting documentation}

### Targeted Improvements (localized design changes)
{changes to specific subsystems or components}

### Strategic Considerations (bigger architectural shifts)
{larger changes worth discussing — not urgent}
```

After writing the report, display a summary to the user with the report path and top 3-5 findings.

## Rules

- You are a CRITIC, not a cheerleader. Be direct about problems. But also acknowledge genuine strengths.
- Every finding must cite specific file paths and sections. No vague claims.
- Distinguish between "this IS broken" and "this COULD be a problem." Use clear confidence levels.
- Don't propose complete solutions — propose directions. The design owner makes the decisions.
- Evaluate against Ariadne's stated goals (fast, deterministic, standalone, structural-only). Don't impose external "best practices" that conflict with these goals.
- Accept trade-offs that are intentional and acknowledged. Flag trade-offs that are unacknowledged.
- In pre-impl mode: focus on design quality. In post-impl mode: also check if code matches design intent.
- If you find something brilliant, say so. Architecture review should identify what to preserve, not just what to change.
- Be skeptical of complexity. The burden of proof is on the design to justify each abstraction.
- Previous architecture reviews in `design/reports/` should be referenced (are past issues resolved?).
