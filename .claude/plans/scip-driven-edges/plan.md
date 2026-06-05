---
slug: scip-driven-edges
title: Trustworthy edges — activate the stubbed SCIP layer to drive precise graph edges
created: 2026-06-04
owners: [user, claude]
review: [user, codex?]
single_tier: false
tiers:
  - tier-01-occurrence-ingest-reference-edges
  - tier-02-access-role-read-write-edges
  - tier-03-relationship-impl-type-edges
---

<context>
Problem: every edge in the graph is derived from tree-sitter bare-name
heuristics; the precise SCIP layer that the `ariadne-scip` adapter exists to
provide is decoded then discarded. PROOF: `scip_symbols` touches its inputs for
memoization then returns an empty vec [src: crates/ariadne-salsa/src/derived.rs:167-171];
`ScipDocInput` is always built `None` [src: crates/ariadne-salsa/src/db.rs:146,185];
the stub is intentional — "SCIP ingest is still stubbed (empty) so the cold-path
output is byte-identical to the pre-refactor CLI committer [RD11]"
[src: crates/ariadne-salsa/src/derived.rs:177-179]. So symbols come from
tree-sitter decls [src: derived.rs:181-204] and edges from the scoped bare-name
resolver [src: crates/ariadne-salsa/src/derive.rs:220-278].

Consequence the user is closing: the name-based resolver had to trade recall for
precision (ADR-0024) — a genuine cross-crate call to an *ambiguous* name now
yields no edge — and it can never produce method/trait/type edges. SCIP carries
exactly the missing signal: occurrences with def/ref/read/write roles and
implementation/type relationships [src: crates/ariadne-scip/proto/scip.proto;
https://github.com/sourcegraph/scip/blob/main/scip.proto].

Solution (user decisions D1/D5): activate SCIP to drive EDGES while tree-sitter
keeps symbol identity (Strategy B), mapping SCIP occurrences to tree-sitter
symbols by source range. Land it in three tiers of rising edge fidelity:
precise References/Imports (recall fix) → Reads/Writes → Implements/TypeOf, and
reconcile the advertised `EdgeKindFilter` to what the graph can actually emit.

In scope: SCIP fact extraction (occurrences + relationships) at the composition
root; a pure salsa-input fact type; range-mapped edge resolution; `EdgeKind`
widening + storage tag round-trip; `EdgeKindFilter` honesty.
Out of scope: changing the `SymbolId` scheme or letting SCIP create/override
symbols (Strategy A — rejected D1); LLM/embeddings (the consumer is the LLM
[src: post-v1-roadmap plan.md `<context>`]); re-enabling withheld docgen
sections — handed to docgen-overview-fidelity tier-03 on the improved edges;
making `--scip` default-on (separate CLI decision, see R1).
</context>

<constraints>
- `ariadne-salsa` may depend only on `ariadne-core` + `ariadne-storage`; SCIP is a
  driven adapter no use-case crate may import [src: tests/architecture.rs:13-14,31-43].
  Therefore SCIP proto is decoded to a pure type at the composition root, never
  inside salsa (D2).
- Pure-Rust critical path; no new dependency — prost + the generated SCIP types
  are already vendored [src: crates/ariadne-scip/src/proto.rs:23;
  crates/ariadne-scip/src/indexer/mod.rs:85].
- Hexagonal + TDD: a failing test precedes implementation each tier
  [src: CLAUDE.md `<rules>`].
- Determinism: same input → identical edge set; BTree/sorted containers only
  [src: crates/ariadne-graph/src/docgen.rs:1-8; ADR-0024 rationale].
- `SymbolId` stability (RD12/ADR-0017) must not regress — SCIP feeds edges only,
  symbols untouched (D1) [src: crates/ariadne-salsa/src/derived.rs:181-204].
- Parity holds: cold==warm and incremental==fresh, both over the one shared
  derivation [src: post-v1-roadmap plan.md RD11; tier-07a/07b].
- SLOs hold (incremental p95 <500ms, query p95 <100ms); per-tier `memory_report()`
  delta reported, >256MB/table is a hard fail (R1) [src: ariadne-core plan.md `<risks>`].
- No symbol-name denylist; abstention is driven by structure, not spelling
  [src: ADR-0024; .claude/plans/god-module-suggestion-fix/plan.md D2].
</constraints>

<decisions>
**D1 — SCIP drives edges; tree-sitter keeps symbols + identity (Strategy B, user).**
The `SymbolId` scheme, goldens, and daemon warm-graph node identity stay
unchanged; SCIP occurrences map to existing tree-sitter symbols by source range.
*Rejected:* SCIP-authoritative symbols (Strategy A) — changes the id scheme for
covered files, a graph-wide re-baseline that churns every golden and revisits
RD12 stability [src: user; crates/ariadne-salsa/src/derived.rs:177-179; ADR-0017].

**D2 — SCIP facts cross into salsa as a pure core type at the composition root.**
salsa cannot decode SCIP proto (would need `ariadne-scip` prost types →
arch violation [src: tests/architecture.rs:13-14,31-43]). A new
`ariadne-scip::extract_facts(&proto::Index) -> ScipFactsRaw` runs at the
`ariadne-cli`/`ariadne-daemon` composition root; `ScipFactsRaw` (pure: per
occurrence `{symbol: String, range, roles: u32}`, per relationship
`{from, to, flags}`) is fed via a salsa input, replacing
`ScipDocInput.raw_proto: Option<Vec<u8>>` [src: crates/ariadne-salsa/src/inputs.rs:50-57].
This mirrors `SyntacticFactsRaw`/`SyntacticFactsInput` produced by
`ariadne-parser` (RD11) [src: inputs.rs:66-70]. *Rejected:* decode proto in salsa.

**D3 — Occurrence→symbol resolution is by source range, not name.** A SCIP
definition occurrence (role `Definition` 0x1) maps to the innermost enclosing
tree-sitter symbol by `def_range`; its (normalized) SCIP symbol string becomes a
global key → `SymbolId`. A non-definition occurrence resolves `src` = enclosing
ts symbol, `dst` = key→`SymbolId`; an unmapped `dst` (std/external, no indexed
definition) drops the edge — precisely recovering the cross-crate calls ADR-0024
abstained on, with zero name collision (SCIP symbol strings are globally unique)
[src: scip.proto Occurrence `range`/`symbol`/`symbol_roles`;
crates/ariadne-scip/src/normalize/mod.rs:160-162;
crates/ariadne-salsa/src/derive.rs:281-296]. *Rejected:* SCIP-canonical→`SymbolId`
direct (that is Strategy A's id migration).

**D4 — SCIP edges supersede tree-sitter per covered+current file; tree-sitter is
the live fallback (user decision).** A file is "covered" only while its content
hash matches the hash its SCIP facts were indexed at; for such files edges come
from SCIP and the tree-sitter `resolve_edges` pass is skipped. A file with no
indexer, `--scip` off, OR an edit that moves its hash off the SCIP index falls
back to tree-sitter edges until SCIP re-runs — so a live edit never shows a stale
resolved edge and v1 liveness holds (SCIP is batch + brittle on uncompiled code;
tree-sitter parses every keystroke [src: https://tree-sitter.github.io/tree-sitter/;
https://github.com/sourcegraph/scip]). The content hash is already tracked
[src: crates/ariadne-salsa/src/inputs.rs:25-36; db.rs:141-148]. *Rejected:* union
both (double counts; conflicting `dst`); keep stale SCIP edges past an edit
(shows renamed/deleted targets as live).

**D5 — Edge-kind set tracks SCIP's actual signal; `EdgeKindFilter` reconciled to
producible kinds (user: highest-quality option).** Add `Reads`/`Writes` (from
`SymbolRole` ReadAccess 0x8 / WriteAccess 0x4) and `Implements`/`TypeOf` (from
`Relationship.is_implementation` / `is_type_definition`) to `EdgeKind`; emit a
kind only when its SCIP bit/flag is present (no fabrication). Reconcile the
daemon `EdgeKindFilter` so every advertised kind maps to a producible `EdgeKind`
— closing the latent advertised-but-unproducible gap [src: query.rs:14-22 vs
records.rs:161-191]. Edges are derived, so widening the enum needs only the
storage tag round-trip + re-index, no redb data migration. *Rejected:* fabricate
Reads/Writes/Inherits from tree-sitter (non-portable, untrustworthy — the failure
this vector fixes) [src: scip.proto SymbolRole/Relationship; records.rs:174-191].
</decisions>

<architecture>
Pipeline today: indexer subprocess → `proto::Index::decode`
[src: crates/ariadne-scip/src/indexer/mod.rs:85] → discarded. New flow:

Composition root (CLI cold, daemon warm) — both already may import `ariadne-scip`
and `ariadne-salsa`: after running indexers, decode each file's SCIP `Document`,
call `ariadne-scip::extract_facts` → `ScipFactsRaw` (pure), set it on the salsa
`ScipFactsInput` for that file (D2). No adapter→adapter edge; the root wires both.

Derivation (`ariadne-salsa`, the one shared path): a memoized per-file query
exposes the file's occurrences/relationships + coverage flag. A driver pass
(sibling to `resolve_edges`, which also needs the whole corpus) builds the global
`scip_symbol → SymbolId` map from definition occurrences (range→enclosing ts
symbol, D3), then resolves the remaining occurrences and the relationships to
typed `EdgeRecord`s; for covered files it replaces the tree-sitter pass (D4).
Output merges into the same `Changeset` the committer already writes, so cold,
warm, and incremental share it [src: crates/ariadne-salsa/src/db.rs:204-258].

Core (`ariadne-core`): `ScipFactsRaw` pure type; `EdgeKind` gains `Reads`,
`Writes`, `Implements`, `TypeOf` with stable byte tags [src: records.rs:161-191].
Storage encodes the new tags (round-trip; old DBs only ever hold tags 0–4).
Daemon `EdgeKindFilter` → `EdgeKind` mapping reconciled [src: query.rs:14-22].
</architecture>

<tech_inventory>
| tech | version pinned | role | tier | source verified this session |
|---|---|---|---|---|
| SCIP schema | proto/SCIP_COMMIT pin | Occurrence roles + Relationship flags = the edge signal | 01,02,03 | crates/ariadne-scip/proto/scip.proto ; https://github.com/sourcegraph/scip/blob/main/scip.proto (Occurrence/SymbolRole/Relationship fields confirmed) |
| prost (generated SCIP types) | workspace pin (vendored) | decode `proto::Index` at the root; no new dep | 01 | crates/ariadne-scip/src/proto.rs:23 ; indexer/mod.rs:85 |
| salsa | =0.26.2 (v1 pin) | new `ScipFactsInput`; memoized occurrence query | 01 | crates/ariadne-salsa/src/inputs.rs:66-70 (SyntacticFactsInput precedent) |
| redb | 4.1.0 (v1 pin) | edge tag round-trip for widened EdgeKind (no data migration) | 02,03 | crates/ariadne-core/src/domain/records.rs:174-191 |
</tech_inventory>

<risks>
| id | risk | likelihood | mitigation |
|---|---|---|---|
| R1 | SCIP indexers are opt-in (`--scip`); absent ⇒ edges stay tree-sitter-quality | high | graceful per-file fallback (D4); v1 behaviour preserved; default-on per available indexer recorded as a follow-up CLI decision/ADR, not built here |
| R2 | SCIP vs tree-sitter disagree on def spans (SFC-bridged, macro expansion) ⇒ range map misses | medium | innermost-enclosing match; an occurrence with no enclosing ts `src` or no mapped `dst` drops — never mis-attributes; coverage is per-(file,mappability) |
| R3 | Reads/Writes/Implements population is indexer-dependent (uneven role/relationship emission) | medium | emit only on a present bit/flag (no fabrication); fixtures chosen to carry them; absence = missing edge, never a wrong edge |
| R4 | edge-count delta breaks insta goldens / parity / blast-radius tests | high | re-baseline goldens deliberately per tier with the recall delta documented; cold==warm and incremental==fresh parity must stay green |
| R5 | non-determinism from occurrence/map iteration order | medium | sort occurrences by (file, range); BTree maps; existing determinism tests stay green |
| R6 | `SymbolId` stability (RD12) regresses | low | SCIP feeds edges only, symbols untouched (D1); the stable-id proptest stays green |
| R7 | new input + edges blow the per-table memory budget | low | per-tier `memory_report()` delta; >256MB/table hard fail [src: ariadne-core plan.md R1] |
| R8 | SCIP is batch (sec–min); edits outrun it ⇒ stale SCIP edges or SLO miss | high | coverage is hash-gated (D4): an edited file drops to live tree-sitter edges until SCIP re-runs; SCIP never runs on the incremental p95<500ms path |
</risks>

<verification>
- Dogfood self-index: the cross-crate edge set becomes precise AND recovers
  genuine calls ADR-0024 dropped (recall up, all true); `apply_writes` still has
  no phantom `new` edge; `cargo test --test architecture` green.
- Parity: cold==warm and incremental==fresh edge sets identical; determinism
  test (index twice → identical edges) green.
- Per kind: a fixture asserts a precise `References`/`Imports` edge (T1), a
  `Reads`/`Writes` edge from access roles (T2), an `Implements`/`TypeOf` edge from
  a trait-impl / typed-binding fixture (T3); `blast_radius` filters each new kind;
  a test asserts `EdgeKindFilter` advertised == producible `EdgeKind` (T3).
- Whole: `cargo nextest run --workspace`, `cargo clippy … -D warnings`,
  `cargo fmt --all --check`, `cargo deny check` (no new dep) green; per-tier memory
  probe within budget; every tier audit PASS. Re-enabling withheld docgen
  sections is handed to docgen-overview-fidelity tier-03 on these edges.
</verification>

<sources>
- SCIP schema (Occurrence / SymbolRole / Relationship): https://github.com/sourcegraph/scip/blob/main/scip.proto ; crates/ariadne-scip/proto/scip.proto (pinned at proto/SCIP_COMMIT)
- SCIP symbol grammar + canonical form: crates/ariadne-scip/src/normalize/mod.rs
- SCIP stub (the gap this plan closes): crates/ariadne-salsa/src/derived.rs:167-179 ; db.rs:146,185
- Edge resolver + enclosing-symbol mapping: crates/ariadne-salsa/src/derive.rs:220-296
- EdgeKind + filter mismatch: crates/ariadne-core/src/domain/records.rs:161-191 ; crates/ariadne-core/src/domain/daemon/query.rs:14-22
- Hexagonal dep invariant: tests/architecture.rs:13-14,31-43 ; docs/adr/0007-cli-composition-root.md
- Scoped-resolution precedent + recall trade: docs/adr/0024-scoped-call-resolution.md ; .claude/plans/docgen-overview-fidelity/plan.md
- Shared derivation + fact-as-input pattern: .claude/plans/post-v1-roadmap/plan.md RD11/RD12 ; ADR-0016/0017
</sources>
