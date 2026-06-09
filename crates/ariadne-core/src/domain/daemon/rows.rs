//! Row / summary payloads shared across daemon responses. Each mirrors the
//! matching MCP wire row so a thin client maps 1:1 with no new shape
//! [src: .claude/plans/post-v1-roadmap/tier-07-daemon-warm-graph.md step 2].

use serde::{Deserialize, Serialize};

/// One symbol row (list / find / blast / doc). The cryptic fields (`id`,
/// `byte_start`, `byte_end`) are `Option` so a concise-verbosity projection can
/// drop them while detailed keeps the lossless superset (Block 1, tier-02 D3,
/// mirroring [`ReferenceSite`]). No `skip_serializing_if` here: this is a
/// postcard-framed daemon-IPC type (the codec underflows the decoder if a field
/// is omitted), so the daemon path carries the dropped fields as `None` and the
/// MCP wire type `From`-projects the JSON-level omission at the serving boundary
/// [src: crates/ariadne-daemon/src/adapters/codec.rs].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SymbolSummary {
    /// Numeric symbol id (`SymbolId::get()`); unset in concise verbosity.
    pub id: Option<u64>,
    /// Canonical name.
    pub name: String,
    /// Free-form kind tag.
    pub kind: String,
    /// Defining file path (project-root-relative).
    pub file: String,
    /// Defining-span byte start; unset in concise verbosity.
    pub byte_start: Option<u32>,
    /// Defining-span byte end; unset in concise verbosity.
    pub byte_end: Option<u32>,
}

/// One reference site surfaced by `find_references`. The cryptic fields
/// (`caller` id, `byte_start`, `byte_end`) are `Option` so concise verbosity
/// leaves them unset while detailed populates the lossless superset (Block 1,
/// tier-01 D3). No `skip_serializing_if` here: this is a daemon-IPC type and
/// the codec is postcard (non-self-describing), which underflows the decoder
/// if a field is omitted — the JSON-level omission lives on the MCP wire type
/// `From`-projected at the serving boundary [src: crates/ariadne-daemon/src/adapters/codec.rs].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReferenceSite {
    /// Caller symbol id; unset in concise verbosity.
    pub caller: Option<u64>,
    /// Caller canonical name.
    pub caller_name: String,
    /// Evidence file path.
    pub file: String,
    /// Evidence span byte start; unset in concise verbosity.
    pub byte_start: Option<u32>,
    /// Evidence span byte end; unset in concise verbosity.
    pub byte_end: Option<u32>,
}

/// One dependency row inside a file summary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DependencyRow {
    /// Depended-on file path.
    pub file: String,
    /// Number of edges crossing this file boundary.
    pub edges: u32,
}

/// One component-graph row inside a file summary (ADR-0012).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComponentRow {
    /// Component symbol canonical name.
    pub component: String,
    /// Child components it renders, sorted.
    pub renders: Vec<String>,
    /// Hooks / reactive primitives it uses, sorted.
    pub hooks: Vec<String>,
}

/// One plan-assist file row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlanFileRow {
    /// File path.
    pub file: String,
    /// Per-symbol reasons collected during the walk (canonical names).
    pub why: Vec<String>,
    /// Rank certainty (higher = stronger reason to touch).
    pub certainty: f32,
}

/// One coupling row (per file-as-module).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CouplingRow {
    /// File path serving as the module identity.
    pub module: String,
    /// Afferent coupling (Ca).
    pub afferent: u32,
    /// Efferent coupling (Ce).
    pub efferent: u32,
    /// Instability `I = Ce / (Ca + Ce)`.
    pub instability: f32,
    /// Abstractness `A`.
    pub abstractness: f32,
    /// Distance from the main sequence.
    pub distance: f32,
}

/// One cycle row in a weak-spots report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CycleRow {
    /// Sorted member symbol names.
    pub members: Vec<String>,
}

/// One outbound-traffic row inside a [`GodModuleRow`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutboundRow {
    /// Module member's canonical name.
    pub symbol: String,
    /// Number of that member's edges leaving the module (external fan-out).
    pub edges: u32,
}

/// One god-module finding in a refactor report.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GodModuleRow {
    /// Module (file) name.
    pub module: String,
    /// Efferent coupling (Ce).
    pub efferent: u32,
    /// Cohesion proxy in `[0, 1]`.
    pub cohesion: f32,
    /// Module members ranked by external fan-out (extraction candidates).
    pub top_outbound: Vec<OutboundRow>,
    /// Human-readable split suggestion.
    pub suggestion: String,
}

/// One cycle-break candidate in a refactor report.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CycleBreakRow {
    /// Source symbol canonical name.
    pub from: String,
    /// Destination symbol canonical name.
    pub to: String,
    /// Cut score in `(0, 1]`; higher = cheaper to cut.
    pub score: f32,
    /// Static design-principle rationale.
    pub rationale: String,
}

/// One ranked hotspot row (tier-15b). The grain is implied by which key is
/// populated: `file` carries the path for a file-grain row; `symbol` carries
/// the resolved symbol for a symbol-grain row (and `file` is empty).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HotspotRow {
    /// File path for a file-grain row; empty for a symbol-grain row.
    pub file: String,
    /// Resolved symbol for a symbol-grain row; `None` for a file-grain row.
    pub symbol: Option<SymbolSummary>,
    /// Raw churn (commits touching the unit) before normalization.
    pub churn: u32,
    /// Raw complexity (`McCabe`, summed for files) before normalization.
    pub complexity: u32,
    /// `norm_churn * norm_complexity` ∈ [0, 1]; `0` when either factor is `0`.
    pub score: f32,
}

/// One ranked complexity row (tier-15b). The grain is implied by which key is
/// populated, matching [`HotspotRow`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComplexityRow {
    /// File path for a file-grain row; empty for a symbol-grain row.
    pub file: String,
    /// Resolved symbol for a symbol-grain row; `None` for a file-grain row.
    pub symbol: Option<SymbolSummary>,
    /// `McCabe` complexity: per-file Σ (file grain) or the symbol's own value.
    pub complexity: u32,
}

/// One changed symbol's blast radius inside a [`super::DiffBlastReport`]
/// (tier-15c). Mirrors `ariadne_graph::DiffSeed`, projected to wire rows: the
/// changed symbol plus its own must / may dependents.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiffSeed {
    /// The changed symbol the radius was seeded from.
    pub symbol: SymbolSummary,
    /// First-hop dependents of the seed, bounded by the fixed per-seed cap
    /// (= `limit`); see `must_touch_total` for the full count (Block 1, tier-04).
    pub must_touch: Vec<SymbolSummary>,
    /// Transitive dependents beyond the first hop, bounded by the same cap; see
    /// `may_touch_total` for the full count.
    pub may_touch: Vec<SymbolSummary>,
    /// Largest hop depth in this seed's returned set.
    pub depth_used: u8,
    /// Full first-hop dependent count before the per-seed inner cap — `>
    /// must_touch.len()` means rows were capped, never silently dropped
    /// (tier-04). Equals `must_touch.len()` when nothing was capped.
    pub must_touch_total: u32,
    /// Full transitive dependent count before the per-seed inner cap, paired
    /// with `must_touch_total`.
    pub may_touch_total: u32,
}

/// One logical-coupling edge between two files (tier-15b). Mirrors
/// `ariadne_graph::CoChangeEdge` field-for-field.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CoChangeEdge {
    /// Lexicographically-smaller path of the pair.
    pub a: String,
    /// Lexicographically-larger path of the pair.
    pub b: String,
    /// Commits that changed both files (the pair's support).
    pub shared_commits: u32,
    /// Coupling degree `shared / mean(revs_a, revs_b)` ∈ [0, 1].
    pub degree: f32,
}

/// One misplaced-symbol finding in a refactor report.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MisplacedRow {
    /// Symbol canonical name.
    pub symbol: String,
    /// Module the symbol currently lives in.
    pub current_module: String,
    /// Module most of its callers belong to.
    pub target_module: String,
    /// Ratio of dominant-external call count to own-module call count.
    pub ratio: f32,
}
