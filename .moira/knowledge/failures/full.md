<!-- moira:knowledge failures L2 -->
# Failures -- Full

> This file is auto-populated by /moira:init. Manual edits are preserved.
>
> Failure format:
> ## [TASK-VERSION] Failure title
> APPROACH: What was tried
> REJECTED BECAUSE: Why it failed
> LESSON: What to learn
> APPLIES TO: When this lesson is relevant

## [task-2026-04-05-001] "Minimal scope" specification language interpreted as optional by implementers
APPROACH: D-142 (Android manifest parsing) was specified in architecture doc as "lightweight mapping" with "minimal scope" qualifiers — listed as a full acceptance criterion in classification.md
REJECTED BECAUSE: Implementing agent skipped the implementation; reviewer Themis accepted this as a non-blocking suggestion (S-3) rather than a warning requiring redo
LESSON: Architecture spec language with scope-limiting adjectives ("minimal", "lightweight", "simple") creates ambiguity about whether the item is required or optional. The final reviewer may accept non-implementation as a suggestion rather than a warning, especially when the advertised feature does not cause functional regressions.
APPLIES TO: Any spec item that appears required in the classification acceptance criteria but uses limiting language in the architecture document. Such items should be re-stated with "MUST implement" language in both the arch doc and the spec.
