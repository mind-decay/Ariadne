//! Per-tool input/output schemas. Every type derives `JsonSchema` so the
//! rmcp `#[tool]` macro can auto-generate the schema served on
//! `tools/list`. All wire ids are `u64` / `String` — the salsa-internal
//! `NonZeroU64`/`Lang::Other(&'static str)` shapes never leak to clients.
//!
//! \[src: .claude/plans/ariadne-core/tier-08-mcp-server.md `<files>`]

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
    /// The resolved target symbol, echoed back so callers can confirm
    /// which symbol the radius was computed for — empty `must_touch` /
    /// `may_touch` then reads as "resolved, no dependents" rather than
    /// "symbol not found".
    pub symbol: SymbolSummary,
    /// First-hop callers (immediate dominators of the queried symbol).
    pub must_touch: Vec<SymbolSummary>,
    /// Transitive callers beyond the first hop.
    pub may_touch: Vec<SymbolSummary>,
    /// Deepest hop level any returned row sits at.
    pub depth_used: u8,
}

/// Path-keyed input shared by `file_summary` and `doc_for_module`
/// (for `doc_for_module` the file path is the module identity).
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
    /// `Component` symbols defined in this file, each with the child
    /// components it renders and the hooks it uses. Empty for files that
    /// carry no framework components (ADR-0012).
    pub components: Vec<ComponentRow>,
}

/// One row of `file_summary.components` — a `Component` symbol paired with
/// its component-graph neighbourhood (ADR-0012). `renders` follows the
/// `Renders` edges to child components; `hooks` follows the `UsesHook`
/// edges to hooks / reactive primitives.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct ComponentRow {
    /// Canonical name of the component symbol.
    pub component: String,
    /// Canonical names of the child components it renders, sorted.
    pub renders: Vec<String>,
    /// Canonical names of the hooks / reactive primitives it uses, sorted.
    pub hooks: Vec<String>,
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

/// Path-prefix scope shared by `coupling_report`, `weak_spots`,
/// `doc_for_project`, and `refactor_suggestions`. Empty = all files.
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
    ///
    /// Computed on the syntactic graph, so the list carries known false
    /// positives: `#[test]` functions, `build.rs::main`, and
    /// serde-derived structs all show zero inbound edges because their
    /// callers (the test harness, Cargo, derive macros) are invisible to
    /// tree-sitter. The semantic `--scip` index resolves those references
    /// and drops the false positives; until then, treat this list as a
    /// triage hint, not a definitive dead-code verdict.
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

/// Output of `doc_for_module` / `doc_for_project` — one rendered
/// Markdown document.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DocOutput {
    /// Rendered Markdown body.
    pub markdown: String,
}

/// One outbound-traffic row inside a [`GodModuleRow`].
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct OutboundRow {
    /// Target symbol canonical name.
    pub symbol: String,
    /// Number of edges flowing to that symbol.
    pub edges: u32,
}

/// One god-module finding in [`RefactorOutput`].
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GodModuleRow {
    /// Module (file) name.
    pub module: String,
    /// Efferent coupling (Ce).
    pub efferent: u32,
    /// Cohesion proxy in `[0, 1]`.
    pub cohesion: f32,
    /// Outbound traffic grouped by target symbol.
    pub top_outbound: Vec<OutboundRow>,
    /// Human-readable split suggestion.
    pub suggestion: String,
}

/// One cycle-break candidate in [`RefactorOutput`].
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
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

/// One misplaced-symbol finding in [`RefactorOutput`].
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
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

/// Output of `refactor_suggestions`. Every entry is a *hint* for review,
/// never an authoritative command (tier-09 step 12).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RefactorOutput {
    /// God-module split candidates.
    pub god_modules: Vec<GodModuleRow>,
    /// Cycle-break edge candidates.
    pub cycle_breaks: Vec<CycleBreakRow>,
    /// Symbols whose callers live mostly in another module.
    pub misplaced_symbols: Vec<MisplacedRow>,
}

/// Grain a `hotspots` / `complexity` query ranks at (tier-15b D2). File grain
/// rolls each file's symbols up to one row (complexity summed); symbol grain
/// returns one row per symbol. Defaults to `File`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "snake_case")]
pub enum Grain {
    /// One row per file, complexity summed over its symbols.
    #[default]
    File,
    /// One row per symbol, carrying its own complexity.
    Symbol,
}

/// Input to `hotspots` and `complexity` — a path-prefix scope and a grain.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, Default)]
pub struct GrainScopeInput {
    /// Optional path-prefix filter (project-root-relative). Empty = all files.
    #[serde(default)]
    pub prefix: Option<String>,
    /// File (default) or symbol grain.
    #[serde(default)]
    pub grain: Grain,
}

/// One ranked hotspot row. Exactly one of `file` / `symbol` is populated,
/// matching the report's grain (mirrors `ariadne_core::HotspotRow`).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct HotspotRow {
    /// File path for a file-grain row; empty for a symbol-grain row.
    pub file: String,
    /// Resolved symbol for a symbol-grain row; `null` for a file-grain row.
    pub symbol: Option<SymbolSummary>,
    /// Raw churn (commits touching the unit) before normalization.
    pub churn: u32,
    /// Raw complexity (`McCabe`, summed for files) before normalization.
    pub complexity: u32,
    /// `norm_churn * norm_complexity` ∈ [0, 1]; `0` when either factor is `0`.
    pub score: f32,
}

/// Output of `hotspots`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct HotspotOutput {
    /// Ranked hotspot rows; the first is the strongest hotspot.
    pub rows: Vec<HotspotRow>,
}

/// One ranked complexity row (mirrors `ariadne_core::ComplexityRow`).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct ComplexityRow {
    /// File path for a file-grain row; empty for a symbol-grain row.
    pub file: String,
    /// Resolved symbol for a symbol-grain row; `null` for a file-grain row.
    pub symbol: Option<SymbolSummary>,
    /// `McCabe` complexity: per-file Σ (file grain) or the symbol's own value.
    pub complexity: u32,
}

/// Output of `complexity`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ComplexityOutput {
    /// Ranked complexity rows; the first is the most complex unit.
    pub rows: Vec<ComplexityRow>,
}

/// Input to `co_change`. The three optional thresholds default to code-maat's
/// published values (`min_revs = 5`, `min_shared_commits = 5`,
/// `min_degree = 0.30`) when omitted.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, Default)]
pub struct CoChangeInput {
    /// Optional path-prefix scope: keeps an edge when either endpoint path is
    /// in scope (project-root-relative). Empty = all files.
    #[serde(default)]
    pub prefix: Option<String>,
    /// Minimum individual revisions per endpoint. Defaults to 5.
    #[serde(default)]
    pub min_revs: Option<u32>,
    /// Minimum shared-commit support per pair. Defaults to 5.
    #[serde(default)]
    pub min_shared_commits: Option<u32>,
    /// Minimum coupling degree ∈ [0, 1]. Defaults to 0.30.
    #[serde(default)]
    pub min_degree: Option<f32>,
}

/// One logical-coupling edge (mirrors `ariadne_core::CoChangeEdge`).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
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

/// Output of `co_change`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CoChangeOutput {
    /// Coupling edges that cleared the filters, degree-descending.
    pub edges: Vec<CoChangeEdge>,
}

/// Which changeset a `diff_blast_radius` query scopes (tier-15c D4). Mirrors
/// `ariadne_core::DiffSpec` behind a `JsonSchema`-deriving input the MCP layer
/// owns; the handler maps it to the core type (core stays a wire/domain type
/// that need not derive `schemars`). Defaults to the uncommitted working tree.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum DiffSpecInput {
    /// Uncommitted index + worktree changes against `HEAD` (the default).
    #[default]
    WorkingTree,
    /// A single commit against its first parent; the string is a revspec
    /// (commit-ish) the git adapter resolves.
    Commit(String),
    /// The diff between two resolved revisions, `from` (old) → `to` (new); both
    /// strings are revspecs the git adapter resolves.
    RefRange {
        /// Old-side revspec.
        from: String,
        /// New-side revspec.
        to: String,
    },
}

/// Input to `diff_blast_radius`.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, Default)]
pub struct DiffBlastInput {
    /// Which changeset to scope. Defaults to the uncommitted working tree.
    #[serde(default)]
    pub spec: DiffSpecInput,
    /// Reverse-BFS hop limit per changed seed. Defaults to 3.
    #[serde(default)]
    pub depth: Option<u8>,
    /// Edge-kind filter set. Empty / missing = all kinds.
    #[serde(default)]
    pub kinds: Option<Vec<EdgeKindFilter>>,
}

/// One changed symbol's blast radius inside a [`DiffBlastOutput`] (mirrors
/// `ariadne_core::DiffSeed` field-for-field).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct DiffSeedRow {
    /// The changed symbol the radius was seeded from.
    pub symbol: SymbolSummary,
    /// First-hop dependents of the seed.
    pub must_touch: Vec<SymbolSummary>,
    /// Transitive dependents beyond the first hop.
    pub may_touch: Vec<SymbolSummary>,
    /// Largest hop depth in this seed's returned set.
    pub depth_used: u8,
}

/// Output of `diff_blast_radius` (mirrors `ariadne_core::DiffBlastReport`).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DiffBlastOutput {
    /// Per-seed radius for each changed symbol, sorted by symbol id.
    pub seeds: Vec<DiffSeedRow>,
    /// Union of every seed's first-hop dependents.
    pub must_touch: Vec<SymbolSummary>,
    /// Union of every seed's transitive dependents, minus `must_touch`.
    pub may_touch: Vec<SymbolSummary>,
    /// Changed paths that resolved to no symbol seed, sorted.
    pub unresolved: Vec<String>,
}
