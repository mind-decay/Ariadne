//! Per-tool input/output schemas. Every type derives `JsonSchema` so the
//! rmcp `#[tool]` macro can auto-generate the schema served on
//! `tools/list`. All wire ids are `u64` / `String` — the salsa-internal
//! `NonZeroU64`/`Lang::Other(&'static str)` shapes never leak to clients.
//!
//! [src: .claude/plans/ariadne-core/tier-08-mcp-server.md `<files>`]

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Edge-kind filter exposed to clients. Subset of the in-RAM graph
/// alphabet — clients pick which edge classes the analytic walks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EdgeKindFilter {
    /// Call edges.
    Calls,
    /// Import edges.
    Imports,
    /// Type-of edges.
    TypeOf,
    /// Definition edges.
    Defines,
    /// Override edges.
    Overrides,
    /// Read edges.
    Reads,
    /// Write edges.
    Writes,
    /// Inheritance edges.
    Inherits,
}

/// One symbol row returned by list/find/blast/plan tools.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct SymbolSummary {
    /// Numeric symbol id (`SymbolId::get()`).
    pub id: u64,
    /// Canonical name.
    pub name: String,
    /// Free-form kind tag.
    pub kind: String,
    /// Defining file path (project-root-relative).
    pub file: String,
    /// 1-based line approximation (`byte_start` mapped to a line via the
    /// stored span). Tier-08 leaves line resolution coarse — `byte_start`
    /// is exposed as the canonical anchor.
    pub byte_start: u32,
    /// `byte_end` paired with `byte_start`.
    pub byte_end: u32,
}

/// Reference / call site surfaced by `find_references`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct ReferenceSite {
    /// Caller symbol id.
    pub caller: u64,
    /// Caller canonical name.
    pub caller_name: String,
    /// Evidence file path.
    pub file: String,
    /// Evidence span `byte_start`.
    pub byte_start: u32,
    /// Evidence span `byte_end`.
    pub byte_end: u32,
}

/// Input to `list_symbols`.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct ListSymbolsInput {
    /// Substring filter on canonical name (case-insensitive). Empty = no filter.
    #[serde(default)]
    pub query: String,
    /// Optional kind filter.
    #[serde(default)]
    pub kind: Option<String>,
    /// Maximum rows returned. Defaults to 64.
    #[serde(default)]
    pub limit: Option<u32>,
}

/// Input to `find_definition` / `find_references` / `doc_for`.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct SymbolQuery {
    /// Canonical name of the queried symbol.
    pub symbol: String,
}

/// Input to `blast_radius`.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct BlastRadiusInput {
    /// Target symbol canonical name.
    pub symbol: String,
    /// Reverse-BFS hop limit. Defaults to 3.
    #[serde(default)]
    pub depth: Option<u8>,
    /// Edge-kind filter set. Empty / missing = all kinds.
    #[serde(default)]
    pub kinds: Option<Vec<EdgeKindFilter>>,
}

/// Output of `blast_radius`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct BlastRadiusOutput {
    /// First-hop callers (immediate dominators of the queried symbol).
    pub must_touch: Vec<SymbolSummary>,
    /// Transitive callers beyond the first hop.
    pub may_touch: Vec<SymbolSummary>,
    /// Deepest hop level any returned row sits at.
    pub depth_used: u8,
}

/// Input to `file_summary`.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct FileQuery {
    /// Project-root-relative file path.
    pub path: String,
}

/// Output of `file_summary`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct FileSummaryOutput {
    /// File path echoed back.
    pub path: String,
    /// Symbols defined in this file.
    pub symbols: Vec<SymbolSummary>,
    /// Sum of incoming edges across the file's symbols.
    pub fan_in: u32,
    /// Sum of outgoing edges across the file's symbols.
    pub fan_out: u32,
    /// Top-5 files this file's symbols depend on (by outgoing edge count).
    pub top_dependencies: Vec<DependencyRow>,
}

/// One row of `file_summary.top_dependencies`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct DependencyRow {
    /// Depended-on file path.
    pub file: String,
    /// Number of outgoing edges crossing this file boundary.
    pub edges: u32,
}

/// Input to `plan_assist`.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct PlanAssistInput {
    /// Target symbol canonical name.
    pub symbol: String,
    /// Maximum file rows. Defaults to 16.
    #[serde(default)]
    pub max_files: Option<u32>,
}

/// Output of `plan_assist`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PlanAssistOutput {
    /// Ranked file rows.
    pub files: Vec<PlanFileRow>,
}

/// One file row of `plan_assist`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct PlanFileRow {
    /// File path.
    pub file: String,
    /// Per-symbol reasons collected during the walk (canonical names).
    pub why: Vec<String>,
    /// Rank certainty (higher = stronger reason to touch).
    pub certainty: f32,
}

/// Input to `coupling_report` and `weak_spots`. Empty = all files.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, Default)]
pub struct ScopeInput {
    /// Optional path-prefix filter (project-root-relative).
    #[serde(default)]
    pub prefix: Option<String>,
}

/// Output of `coupling_report`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CouplingOutput {
    /// One row per file-as-module.
    pub rows: Vec<CouplingRow>,
}

/// One module row of `coupling_report`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct CouplingRow {
    /// File path serving as the module identity.
    pub module: String,
    /// Afferent coupling (Ca).
    pub afferent: u32,
    /// Efferent coupling (Ce).
    pub efferent: u32,
    /// Instability `I = Ce / (Ca + Ce)`.
    pub instability: f32,
    /// Abstractness (always 0 in tier-08: kind taxonomy is not yet wired).
    pub abstractness: f32,
    /// Distance from the main sequence.
    pub distance: f32,
}

/// Output of `weak_spots`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct WeakSpotsOutput {
    /// Strongly-connected components (size ≥ 2).
    pub cycles: Vec<CycleRow>,
    /// God modules — `efferent > god_threshold`.
    pub god_modules: Vec<CouplingRow>,
    /// Dead symbols (`fan_in` = 0, no exports).
    pub dead_symbols: Vec<SymbolSummary>,
}

/// One cycle row of `weak_spots`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct CycleRow {
    /// Sorted symbol names participating in the cycle.
    pub members: Vec<String>,
}

/// Output of `doc_for`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DocForOutput {
    /// Synthesized signature line.
    pub signature: String,
    /// Free-form kind tag.
    pub kind: String,
    /// Defining file path.
    pub file: String,
    /// One-line brief — tier-08 returns the canonical name; richer docs
    /// land alongside the SCIP doc-string ingest path.
    pub brief: String,
    /// Public callers (first 16 by id).
    pub public_refs: Vec<SymbolSummary>,
}

/// Output of `project_status`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ProjectStatusOutput {
    /// Latest persisted storage revision.
    pub revision: u64,
    /// File count.
    pub file_count: u32,
    /// Symbol count.
    pub symbol_count: u32,
    /// Edge count.
    pub edge_count: u32,
    /// Project root path.
    pub root: String,
}
