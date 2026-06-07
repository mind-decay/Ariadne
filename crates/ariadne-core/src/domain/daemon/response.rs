//! Response side of the daemon protocol: the response enum and its
//! aggregate report payloads. Each mirrors the matching MCP tool output so
//! a thin client maps 1:1 with no new shape
//! [src: .claude/plans/post-v1-roadmap/tier-07-daemon-warm-graph.md step 2].

use serde::{Deserialize, Serialize};

use super::rows::{
    CoChangeEdge, ComplexityRow, ComponentRow, CouplingRow, CycleBreakRow, CycleRow, DependencyRow,
    DiffSeed, GodModuleRow, HotspotRow, MisplacedRow, PlanFileRow, ReferenceSite, SymbolSummary,
};

/// `blast_radius` report — the resolved target plus must / may dependents.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlastRadiusReport {
    /// Resolved target symbol, echoed so an empty radius reads as
    /// "resolved, no dependents" rather than "not found".
    pub symbol: SymbolSummary,
    /// First-hop callers (immediate dominators of the queried symbol).
    pub must_touch: Vec<SymbolSummary>,
    /// Transitive callers beyond the first hop.
    pub may_touch: Vec<SymbolSummary>,
    /// Deepest hop level any returned row sits at.
    pub depth_used: u8,
}

/// `file_summary` report — symbols, fan totals, deps, and components.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileSummaryReport {
    /// File path echoed back.
    pub path: String,
    /// Symbols defined in this file, sorted by `byte_start`.
    pub symbols: Vec<SymbolSummary>,
    /// Sum of incoming edges across the file's symbols.
    pub fan_in: u32,
    /// Sum of outgoing edges across the file's symbols.
    pub fan_out: u32,
    /// Top-5 files this file's symbols depend on.
    pub top_dependencies: Vec<DependencyRow>,
    /// `Component` symbols defined here with their render/hook neighbours.
    pub components: Vec<ComponentRow>,
}

/// `plan_assist` report — ranked file rows.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlanAssistReport {
    /// Ranked rows; first row has the highest certainty.
    pub files: Vec<PlanFileRow>,
}

/// `coupling_report` — one row per file-as-module.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CouplingReport {
    /// Per-module metrics.
    pub rows: Vec<CouplingRow>,
}

/// `weak_spots` report — cycles ∪ god modules ∪ dead code.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WeakSpotsReport {
    /// Strongly-connected components of size ≥ 2.
    pub cycles: Vec<CycleRow>,
    /// God modules — efferent coupling above the threshold.
    pub god_modules: Vec<CouplingRow>,
    /// Dead symbols (fan-in 0, not a root), capped.
    pub dead_symbols: Vec<SymbolSummary>,
}

/// `doc_for` report — structured single-symbol summary.
///
/// Tier-05 appends deterministic, system-only enrichment fields (`role`,
/// `file_risk`, `blast_must`, `blast_may`) after the original surface. The
/// pre-existing fields keep their name and order so no consumer breaks; the
/// cold (MCP/CLI) and warm (daemon) paths compute every field identically so
/// the structured output is byte-equal on either route (parity).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DocForReport {
    /// Synthesized signature line.
    pub signature: String,
    /// Free-form kind tag.
    pub kind: String,
    /// Defining file path.
    pub file: String,
    /// One-line brief.
    pub brief: String,
    /// Must-touch (funnel) callers — the blast-radius `must` set, first 16 by
    /// id, scope-filtered to source neighbours.
    pub public_refs: Vec<SymbolSummary>,
    /// Role one-liner: `kind` situated in the defining file's hexagonal layer.
    pub role: String,
    /// Defining file's churn × complexity risk ∈ [0, 1]; `None` when no Git
    /// history is indexed.
    pub file_risk: Option<f32>,
    /// Count of must-touch callers — the immediate-dominator predecessors within
    /// the doc blast depth (3), the unfiltered blast-radius `must`.
    pub blast_must: u32,
    /// Count of may-touch callers — the other transitive callers within the doc
    /// blast depth (3), the blast-radius `may`. `0` only when every caller is a
    /// funnel point.
    pub blast_may: u32,
}

/// `doc_for_module` / `doc_for_project` — one rendered Markdown document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocReport {
    /// Rendered Markdown body.
    pub markdown: String,
}

/// `project_status` report — coarse counts plus the persisted revision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectStatusReport {
    /// Persisted redb revision the warm graph currently holds.
    pub revision: u64,
    /// Number of indexed files.
    pub file_count: u32,
    /// Number of indexed symbols.
    pub symbol_count: u32,
    /// Number of graph edges.
    pub edge_count: u32,
    /// Project root path the daemon was launched against.
    pub root: String,
}

/// `refactor_suggestions` report — god modules ∪ cycle breaks ∪ misplaced
/// symbols. Every entry is a *hint* for review, never an authoritative
/// command.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RefactorReport {
    /// God-module split candidates.
    pub god_modules: Vec<GodModuleRow>,
    /// Cycle-break edge candidates.
    pub cycle_breaks: Vec<CycleBreakRow>,
    /// Symbols whose callers live mostly in another module.
    pub misplaced_symbols: Vec<MisplacedRow>,
}

/// `hotspots` report — churn × complexity rows ranked strongest-first
/// (tier-15b). Mirrors `ariadne_graph::HotspotReport` projected to wire rows.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HotspotReport {
    /// Ranked hotspot rows; the first is the strongest hotspot.
    pub rows: Vec<HotspotRow>,
}

/// `complexity` report — `McCabe` rows ranked complexity-descending (tier-15b).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComplexityReport {
    /// Ranked complexity rows; the first is the most complex unit.
    pub rows: Vec<ComplexityRow>,
}

/// `co_change` report — logical-coupling edges (tier-15b). Mirrors
/// `ariadne_graph::CoChangeReport` projected to wire edges.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CoChangeReport {
    /// Coupling edges that cleared the filters, degree-descending.
    pub edges: Vec<CoChangeEdge>,
}

/// `diff_blast_radius` report — per-seed radii plus the deduped must / may
/// union and the changed paths that resolved to no symbol (tier-15c). Mirrors
/// `ariadne_graph::DiffBlastReport` projected to wire rows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiffBlastReport {
    /// Per-seed radius for each changed symbol, sorted by `SymbolId`.
    pub seeds: Vec<DiffSeed>,
    /// Union of every seed's first-hop dependents.
    pub must_touch: Vec<SymbolSummary>,
    /// Union of every seed's transitive dependents, minus `must_touch`.
    pub may_touch: Vec<SymbolSummary>,
    /// Changed paths that resolved to no symbol seed, sorted.
    pub unresolved: Vec<String>,
}

/// `affected_tests` report — the tests a change reaches, the changed-symbol
/// seeds, and the changed paths that resolved to no symbol (Block A, A1).
/// Mirrors `ariadne_graph::AffectedTestsReport` projected to wire rows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AffectedTestsReport {
    /// Affected test symbols, sorted by `SymbolId`.
    pub tests: Vec<SymbolSummary>,
    /// Changed-symbol seeds, sorted by `SymbolId`.
    pub seeds: Vec<SymbolSummary>,
    /// Changed paths that resolved to no symbol seed, sorted.
    pub unresolved: Vec<String>,
}

/// The daemon's reply to a [`super::DaemonRequest`]. Matched exhaustively
/// by the transport adapter — see [`super::DaemonQuery`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DaemonResponse {
    /// Acknowledgement of a liveness probe — the daemon is alive.
    Pong,
    /// `list_symbols` rows.
    Symbols(Vec<SymbolSummary>),
    /// `find_definition` result.
    Definition(SymbolSummary),
    /// `find_references` rows.
    References(Vec<ReferenceSite>),
    /// `blast_radius` report.
    BlastRadius(BlastRadiusReport),
    /// `file_summary` report.
    FileSummary(FileSummaryReport),
    /// `plan_assist` report.
    PlanAssist(PlanAssistReport),
    /// `coupling_report`.
    Coupling(CouplingReport),
    /// `weak_spots` report.
    WeakSpots(WeakSpotsReport),
    /// `doc_for` report.
    DocFor(DocForReport),
    /// `doc_for_module` / `doc_for_project` markdown.
    Doc(DocReport),
    /// `project_status` report.
    ProjectStatus(ProjectStatusReport),
    /// `refactor_suggestions` report.
    Refactor(RefactorReport),
    /// `hotspots` report.
    Hotspots(HotspotReport),
    /// `complexity` report.
    Complexity(ComplexityReport),
    /// `co_change` report.
    CoChange(CoChangeReport),
    /// `diff_blast_radius` report (tier-15c).
    DiffBlast(DiffBlastReport),
    /// `affected_tests` report (Block A, A1).
    AffectedTests(AffectedTestsReport),
    /// A query-level failure (symbol / file / module not found, …). Mirrors
    /// the v1 MCP `NotFound` outcome without leaking an adapter error type.
    Error(String),
}
