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

/// One symbol row returned by list/find/blast/plan tools. The cryptic fields
/// (`id`, `byte_start`, `byte_end`) are `Option` with `skip_serializing_if` so
/// a concise-verbosity projection omits them from the JSON while detailed emits
/// the lossless superset (Block 1, tier-02 D3, mirroring [`ReferenceSite`]).
/// Tools that always emit detailed populate `Some`, so their JSON is unchanged.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct SymbolSummary {
    /// Numeric symbol id (`SymbolId::get()`); omitted in concise verbosity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<u64>,
    /// Canonical name.
    pub name: String,
    /// Free-form kind tag.
    pub kind: String,
    /// Defining file path (project-root-relative).
    pub file: String,
    /// Defining-span byte start (the canonical anchor); omitted in concise
    /// verbosity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub byte_start: Option<u32>,
    /// `byte_end` paired with `byte_start`; omitted in concise verbosity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub byte_end: Option<u32>,
}

impl From<ariadne_core::SymbolSummary> for SymbolSummary {
    fn from(s: ariadne_core::SymbolSummary) -> Self {
        Self {
            id: s.id,
            name: s.name,
            kind: s.kind,
            file: s.file,
            byte_start: s.byte_start,
            byte_end: s.byte_end,
        }
    }
}

/// Field verbosity for a growable tool's rows (Block 1, tier-01 D3). Concise
/// (the default) omits the cryptic id/offset fields the LLM reasons about
/// worse; `detailed` is a lossless superset. Mirrors
/// `ariadne_graph::economy::Verbosity` / `ariadne_core::Verbosity`; the server
/// maps between them (as it does for [`Grain`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Verbosity {
    /// Omit cryptic id/offset fields (the default).
    #[default]
    Concise,
    /// Emit every field — the lossless superset.
    Detailed,
}

/// Reference / call site surfaced by `find_references`. The cryptic fields
/// (`caller` id, `byte_start`, `byte_end`) are `Option` so concise verbosity
/// omits them while detailed emits the lossless superset (tier-01 D3).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct ReferenceSite {
    /// Caller symbol id; omitted in concise verbosity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub caller: Option<u64>,
    /// Caller canonical name.
    pub caller_name: String,
    /// Evidence file path.
    pub file: String,
    /// Evidence span `byte_start`; omitted in concise verbosity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub byte_start: Option<u32>,
    /// Evidence span `byte_end`; omitted in concise verbosity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub byte_end: Option<u32>,
}

/// Input to `find_references` — a dedicated type (not the shared
/// [`SymbolQuery`]) so adding `limit`/`cursor`/`verbosity` here leaves
/// `find_definition`/`doc_for` untouched (tier-01).
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct FindReferencesInput {
    /// Canonical name of the referenced symbol.
    pub symbol: String,
    /// Maximum rows in the page; defaults to the economy page size (50).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    /// Opaque pagination cursor from a prior page; absent = first page.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
    /// Field verbosity; defaults to concise.
    #[serde(default)]
    pub verbosity: Verbosity,
}

/// Output of `find_references` — one page of reference sites plus the
/// pagination cursor and a human steer (tier-01 D5). This is the JSON wire
/// type: it carries `skip_serializing_if` so concise rows omit their unset
/// fields. The warm daemon path decodes the postcard-framed
/// `ariadne_core::ReferencesReport` and `From`-projects it here, so both paths
/// serialize to byte-identical JSON (parity).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct FindReferencesOutput {
    /// Reference sites in this page, in stable order.
    pub references: Vec<ReferenceSite>,
    /// Opaque cursor for the next page; absent when this is the last page.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
    /// Human steer emitted only when the result was truncated.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

impl From<ariadne_core::ReferenceSite> for ReferenceSite {
    fn from(r: ariadne_core::ReferenceSite) -> Self {
        Self {
            caller: r.caller,
            caller_name: r.caller_name,
            file: r.file,
            byte_start: r.byte_start,
            byte_end: r.byte_end,
        }
    }
}

impl From<ariadne_core::ReferencesReport> for FindReferencesOutput {
    fn from(r: ariadne_core::ReferencesReport) -> Self {
        Self {
            references: r.references.into_iter().map(ReferenceSite::from).collect(),
            next_cursor: r.next_cursor,
            note: r.note,
        }
    }
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

/// Input to `search_code`. Generalises [`ListSymbolsInput`]: the name match
/// is a case-insensitive substring by default, or a regular expression when
/// `regex` is set, narrowed by optional `path` glob / `kind` / `lang` /
/// `visibility` filters [src: tier-07 D8].
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct SearchCodeInput {
    /// Name pattern. A case-insensitive substring of the canonical name by
    /// default; a regular expression when `regex` is true. Empty = no name
    /// filter.
    #[serde(default)]
    pub query: String,
    /// Treat `query` as a regular expression instead of a substring.
    #[serde(default)]
    pub regex: bool,
    /// Optional Unix glob the defining file path must match (e.g.
    /// `src/**/*.rs`).
    #[serde(default)]
    pub path: Option<String>,
    /// Optional exact kind filter (e.g. `function`, `struct`).
    #[serde(default)]
    pub kind: Option<String>,
    /// Optional language-tag filter (e.g. `rust`, `typescript`).
    #[serde(default)]
    pub lang: Option<String>,
    /// Optional visibility filter (`public`, `restricted`, `private`,
    /// `unknown`).
    #[serde(default)]
    pub visibility: Option<String>,
    /// Maximum rows returned. Defaults to 64.
    #[serde(default)]
    pub limit: Option<u32>,
}

/// Input to `read_symbol`. Resolves a symbol to its defining span and reads
/// the live file under the catalog root, returning just that slice — `file`
/// disambiguates an overloaded name, `mode` selects how much to return, and
/// `context_lines` widens the `context` mode [src: tier-08 D9].
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct ReadSymbolInput {
    /// Canonical name of the symbol to read.
    pub symbol: String,
    /// Optional defining-file path (project-root-relative) to pick one of
    /// several symbols sharing `symbol`. Omitted = the first match.
    #[serde(default)]
    pub file: Option<String>,
    /// How much to return: `signature` (the declaration line), `full` (the
    /// whole defining span, the default), or `context` (±`context_lines`).
    #[serde(default)]
    pub mode: Option<String>,
    /// Lines of surrounding context for `context` mode. Defaults to 3.
    #[serde(default)]
    pub context_lines: Option<u32>,
}

/// Output of `read_symbol` — a slice of source read live from disk for the
/// resolved symbol's defining span. `stale` is `true` when the recorded span
/// ran past the current file length (the slice is then clamped, never
/// fabricated) [src: tier-08 D9, R7].
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct SourceSlice {
    /// Canonical name of the resolved symbol.
    pub name: String,
    /// Defining file path (project-root-relative) the slice was read from.
    pub file: String,
    /// 1-based first line of the returned slice.
    pub line_start: u32,
    /// 1-based last line of the returned slice.
    pub line_end: u32,
    /// Byte offset of the returned slice's start in the file.
    pub byte_start: u32,
    /// Byte offset of the returned slice's end in the file (clamped to the
    /// current file length when the recorded span was stale).
    pub byte_end: u32,
    /// Catalog revision the span was resolved against.
    pub revision: u64,
    /// `true` when the recorded span exceeded the current file length, so the
    /// slice was clamped.
    pub stale: bool,
    /// The returned source text (lossy-UTF8 of the byte slice).
    pub source: String,
    /// Other defining-file paths sharing `name`, surfaced only when `file` was
    /// omitted and several symbols matched: the resolver returns the first and
    /// lists the rest here so the caller knows overloads existed and can
    /// re-query with `file` to pin one. Empty when the name was unambiguous or
    /// `file` was supplied (tier-08 step 4 "+ note").
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub alternatives: Vec<String>,
}

/// Input to `read_outline`. Names the file to project into a folded code
/// skeleton (signatures + doc comments kept, bodies elided to a marker) plus a
/// symbol index, so a consumer expands only the bodies it needs via
/// `read_symbol` [src: context-efficient-read plan.md D1/D2].
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct ReadOutlineInput {
    /// Project-root-relative path of the file to outline.
    pub path: String,
    /// Keep non-public symbols (and their folded bodies). Defaults to `true`;
    /// `false` drops everything below `Public` from both skeleton and index.
    #[serde(default)]
    pub include_private: Option<bool>,
}

/// One symbol-index row in a [`SourceOutline`] (mirrors
/// `ariadne_graph::OutlineEntry`): the source a consumer can expand on demand
/// via `read_symbol`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct OutlineEntry {
    /// Declared identifier name.
    pub name: String,
    /// Free-form kind tag carried from the indexed symbol.
    pub kind: String,
    /// 1-based first source line of the symbol.
    pub line_start: u32,
    /// 1-based last source line of the symbol.
    pub line_end: u32,
    /// Source lines spanned by the (folded or kept) body.
    pub body_lines: u32,
    /// Whether the symbol has a body beyond its signature line.
    pub has_body: bool,
}

/// Output of `read_outline` — a token-cheap folded code skeleton of a whole
/// file built from the live bytes + the indexed symbol spans, plus a compact
/// symbol index. `kept_lines + elided_lines` accounts for every source line;
/// `stale` is `true` when a recorded span ran past the current file length (the
/// skeleton is then clamped, never fabricated, R5). A file with no indexed
/// symbols returns an empty skeleton and a `note` instead of dumping the source
/// [src: context-efficient-read tier-02; tier-01 outline use case].
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct SourceOutline {
    /// File path echoed back (project-root-relative).
    pub path: String,
    /// Catalog revision the spans were resolved against.
    pub revision: u64,
    /// `true` when a recorded span exceeded the current file length, so the
    /// skeleton was clamped.
    pub stale: bool,
    /// The rendered folded source.
    pub skeleton: String,
    /// Retained symbols in source order, advertising `read_symbol` expansion.
    pub symbols: Vec<OutlineEntry>,
    /// Source lines kept verbatim in the skeleton.
    pub kept_lines: u32,
    /// Source lines folded away (bodies + hidden symbols + elided gaps).
    pub elided_lines: u32,
    /// Present only when the file has no indexed symbols: a line-count note
    /// advising a native `Read`, never a source dump.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
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

/// Input to `coupling_report` — a dedicated type (not the shared
/// [`ScopeInput`]) so adding `limit`/`cursor`/`verbosity` here leaves
/// `weak_spots`/`doc_for_project`/`refactor_suggestions` (which keep
/// [`ScopeInput`]) untouched (Block 1, tier-02).
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, Default)]
pub struct CouplingInput {
    /// Optional path-prefix filter (project-root-relative). Empty = all files.
    #[serde(default)]
    pub prefix: Option<String>,
    /// Maximum rows in the page; defaults to the economy page size (50).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    /// Opaque pagination cursor from a prior page; absent = first page.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
    /// Field verbosity; defaults to concise (a no-op — metric-only rows carry
    /// no cryptic fields, so concise == detailed).
    #[serde(default)]
    pub verbosity: Verbosity,
}

/// Output of `coupling_report` — one page of module rows plus the pagination
/// cursor and a human steer (tier-02 D5).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CouplingOutput {
    /// One row per file-as-module, in stable (Ca desc, module asc) order.
    pub rows: Vec<CouplingRow>,
    /// Opaque cursor for the next page; absent when this is the last page.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
    /// Human steer emitted only when the result was truncated.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

impl From<ariadne_core::CouplingRow> for CouplingRow {
    fn from(r: ariadne_core::CouplingRow) -> Self {
        Self {
            module: r.module,
            afferent: r.afferent,
            efferent: r.efferent,
            instability: r.instability,
            abstractness: r.abstractness,
            distance: r.distance,
        }
    }
}

impl From<ariadne_core::CouplingReport> for CouplingOutput {
    fn from(r: ariadne_core::CouplingReport) -> Self {
        Self {
            rows: r.rows.into_iter().map(CouplingRow::from).collect(),
            next_cursor: r.next_cursor,
            note: r.note,
        }
    }
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

/// Output of `doc_for`. Mirrors [`ariadne_core::DocForReport`] field-for-field
/// so the cold projection serializes to the byte-identical JSON the warm
/// daemon path produces (tier-05 parity).
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

/// Input to `hotspots` and `complexity` — a path-prefix scope, a grain, and the
/// economy page controls (Block 1, tier-02).
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, Default)]
pub struct GrainScopeInput {
    /// Optional path-prefix filter (project-root-relative). Empty = all files.
    #[serde(default)]
    pub prefix: Option<String>,
    /// File (default) or symbol grain.
    #[serde(default)]
    pub grain: Grain,
    /// Maximum rows in the page; defaults to the economy page size (50).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    /// Opaque pagination cursor from a prior page; absent = first page.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
    /// Field verbosity; defaults to concise (drops the embedded symbol's
    /// cryptic id/offset fields on symbol-grain rows).
    #[serde(default)]
    pub verbosity: Verbosity,
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

/// Output of `hotspots` — one page of ranked rows plus the pagination cursor
/// and a human steer (tier-02 D5).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct HotspotOutput {
    /// Ranked hotspot rows in this page; the first is the strongest hotspot.
    pub rows: Vec<HotspotRow>,
    /// Opaque cursor for the next page; absent when this is the last page.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
    /// Human steer emitted only when the result was truncated.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

impl From<ariadne_core::HotspotRow> for HotspotRow {
    fn from(r: ariadne_core::HotspotRow) -> Self {
        Self {
            file: r.file,
            symbol: r.symbol.map(SymbolSummary::from),
            churn: r.churn,
            complexity: r.complexity,
            score: r.score,
        }
    }
}

impl From<ariadne_core::HotspotReport> for HotspotOutput {
    fn from(r: ariadne_core::HotspotReport) -> Self {
        Self {
            rows: r.rows.into_iter().map(HotspotRow::from).collect(),
            next_cursor: r.next_cursor,
            note: r.note,
        }
    }
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

/// Output of `complexity` — one page of ranked rows plus the pagination cursor
/// and a human steer (tier-02 D5).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ComplexityOutput {
    /// Ranked complexity rows in this page; the first is the most complex unit.
    pub rows: Vec<ComplexityRow>,
    /// Opaque cursor for the next page; absent when this is the last page.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
    /// Human steer emitted only when the result was truncated.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

impl From<ariadne_core::ComplexityRow> for ComplexityRow {
    fn from(r: ariadne_core::ComplexityRow) -> Self {
        Self {
            file: r.file,
            symbol: r.symbol.map(SymbolSummary::from),
            complexity: r.complexity,
        }
    }
}

impl From<ariadne_core::ComplexityReport> for ComplexityOutput {
    fn from(r: ariadne_core::ComplexityReport) -> Self {
        Self {
            rows: r.rows.into_iter().map(ComplexityRow::from).collect(),
            next_cursor: r.next_cursor,
            note: r.note,
        }
    }
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
    /// Maximum edges in the page; defaults to the economy page size (50).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    /// Opaque pagination cursor from a prior page; absent = first page.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
    /// Field verbosity; defaults to concise (a no-op — edges carry no cryptic
    /// fields, so concise == detailed).
    #[serde(default)]
    pub verbosity: Verbosity,
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

/// Output of `co_change` — one page of coupling edges plus the pagination
/// cursor and a human steer (tier-02 D5).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CoChangeOutput {
    /// Coupling edges in this page that cleared the filters, degree-descending.
    pub edges: Vec<CoChangeEdge>,
    /// Opaque cursor for the next page; absent when this is the last page.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
    /// Human steer emitted only when the result was truncated.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

impl From<ariadne_core::CoChangeEdge> for CoChangeEdge {
    fn from(e: ariadne_core::CoChangeEdge) -> Self {
        Self {
            a: e.a,
            b: e.b,
            shared_commits: e.shared_commits,
            degree: e.degree,
        }
    }
}

impl From<ariadne_core::CoChangeReport> for CoChangeOutput {
    fn from(r: ariadne_core::CoChangeReport) -> Self {
        Self {
            edges: r.edges.into_iter().map(CoChangeEdge::from).collect(),
            next_cursor: r.next_cursor,
            note: r.note,
        }
    }
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

/// Input to `affected_tests` — mirrors `diff_blast_radius` (the same changeset
/// `spec` + reverse-reach `depth`/`kinds`), since A1 reuses the diff→seed→
/// reverse-reach machinery (block-a plan.md D1).
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, Default)]
pub struct AffectedTestsInput {
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

/// Output of `affected_tests` (mirrors `ariadne_core::AffectedTestsReport`).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AffectedTestsOutput {
    /// Affected test symbols, sorted by symbol id.
    pub tests: Vec<SymbolSummary>,
    /// Changed-symbol seeds, sorted by symbol id.
    pub seeds: Vec<SymbolSummary>,
    /// Changed paths that resolved to no symbol seed, sorted.
    pub unresolved: Vec<String>,
}

/// Input to `api_surface_diff` (block A, A2): the two revspecs whose
/// public-surface delta is classified. A2 runs entirely in the querying process
/// — git diff + base/head blob reads + parser surface extraction + pure
/// classify — with no daemon leg (D6 / ADR-0027).
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct ApiSurfaceDiffInput {
    /// Base (old) revspec the surface is compared *from*.
    pub base: String,
    /// Head (new) revspec the surface is compared *to*.
    pub head: String,
}

/// `SemVer` bump verdict (mirrors `ariadne_graph::SemverBump`), serialized as a
/// lowercase tag (`none` / `patch` / `minor` / `major`). `patch` is part of the
/// taxonomy but never emitted: the surface model classifies only additions,
/// removals, and signature changes [src:
/// <https://doc.rust-lang.org/cargo/reference/semver.html>].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SemverBumpWire {
    /// No public-surface change.
    None,
    /// Backward-compatible non-surface change (never emitted here).
    Patch,
    /// Backward-compatible addition.
    Minor,
    /// Breaking change.
    Major,
}

/// One public symbol row in an `api_surface_diff` `added` / `removed` list
/// (mirrors the identifying fields of `ariadne_core::PublicSymbol`; every entry
/// is public by construction, so visibility is omitted).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct ApiSymbolRow {
    /// Declared identifier name.
    pub name: String,
    /// Free-form kind tag (e.g. `function`, `struct`).
    pub kind: String,
    /// Whitespace-normalized declaration-header text.
    pub signature: String,
}

/// Input to `fitness_report` (block A, A3). The tool takes no parameters: it
/// reads the repo's `ariadne-fitness.toml` (ADR-0028) and runs the engine over
/// the indexed graph, so callers pass an empty `{}`.
#[derive(Debug, Clone, Default, Deserialize, Serialize, JsonSchema)]
pub struct FitnessReportInput {}

/// One architecture-fitness violation (mirrors `ariadne_graph::Violation`, with
/// `FileId`s resolved to project-root-relative paths and cycle members to
/// canonical symbol names). Externally tagged like [`DiffSpecInput`], so each
/// variant serializes under its `snake_case` name.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum FitnessViolation {
    /// An inter-file dependency crossed a forbidden layer boundary.
    ForbiddenDependency {
        /// Resolved layer of the depending (source) file.
        from_layer: String,
        /// Resolved layer of the depended-on (target) file.
        to_layer: String,
        /// Depending (source) file path.
        from_file: String,
        /// Depended-on (target) file path.
        to_file: String,
    },
    /// A dependency cycle present when the cycle count exceeds `max_cycles`.
    Cycle {
        /// Canonical names of the symbols participating in the cycle, sorted.
        members: Vec<String>,
    },
    /// A file whose instability `I = Ce / (Ca + Ce)` exceeded the ceiling.
    Instability {
        /// The over-coupled file path.
        module: String,
        /// The file's measured instability.
        instability: f32,
    },
}

/// Output of `fitness_report` (block A, A3): the architecture-fitness verdict.
/// `ok` is `true` exactly when `violations` is empty.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct FitnessOutput {
    /// `true` when the architecture passes every rule (no violations).
    pub ok: bool,
    /// Every violation found, sorted deterministically.
    pub violations: Vec<FitnessViolation>,
}

/// One signature-changed row in an `api_surface_diff` `changed` list (mirrors
/// `ariadne_graph::SignatureChange`): same identity, differing header.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct ApiChangeRow {
    /// Declared identifier name (the changed item's identity, with `kind`).
    pub name: String,
    /// Free-form kind tag (the changed item's identity, with `name`).
    pub kind: String,
    /// Declaration header on the base ref.
    pub base_signature: String,
    /// Declaration header on the head ref.
    pub head_signature: String,
}

/// Output of `api_surface_diff` (mirrors `ariadne_graph::ApiDiffReport`). Lists
/// are sorted by `(name, kind)`; `verdict` is the maximum bump over every delta.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct ApiSurfaceDiffOutput {
    /// Overall verdict: the maximum bump implied by any delta.
    pub verdict: SemverBumpWire,
    /// Public items present on head but not base (each a minor bump).
    pub added: Vec<ApiSymbolRow>,
    /// Public items present on base but not head (each a major bump).
    pub removed: Vec<ApiSymbolRow>,
    /// Public items present on both refs whose signature changed (each major).
    pub changed: Vec<ApiChangeRow>,
}
