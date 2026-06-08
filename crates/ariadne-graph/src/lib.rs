//! Graph analytics use case. Tier-07 wires petgraph-backed in-RAM
//! reachability, dominators, SCC, coupling, dead-code, and plan-assist on
//! top of `ariadne-core` ports + `ariadne-storage` reads.
//!
//! All public surface is re-exported from this façade; the analytics live
//! in dedicated sibling modules so each stays under the project's
//! ≤200-line authored-file cap (CLAUDE.md `<rules>`).

#![deny(missing_docs)]

mod api_surface;
mod blast;
mod build;
mod co_change;
mod coupling;
mod cycles;
mod dead;
mod diagram;
mod diff_blast;
pub mod doc_model;
pub mod docgen;
mod docgen_insights;
pub mod economy;
pub mod errors;
mod fitness;
mod heuristics;
mod hotspot;
pub mod outline;
mod plan_assist;
pub mod refactor;
pub mod roots;
mod span_lines;
mod symbol_churn;
mod test_impact;

pub use api_surface::{ApiDiffReport, SemverBump, SignatureChange, api_surface_diff};
pub use blast::BlastRadius;
pub use build::{EdgeDelta, EdgeKind, EdgeKindSet, EdgeMeta, GraphIndex};
pub use co_change::{CoChangeConfig, CoChangeEdge, CoChangeReport, co_change_report};
pub use coupling::{CouplingMetrics, CouplingReport, ModuleSpec};
pub use cycles::{Cycle, CycleReport};
pub use dead::{DeadCodeConfig, DeadCodeReport, DeadSymbol};
pub use diagram::{DiagramEdge, DiagramNode, DiagramOpts, render_svg};
pub use diff_blast::{DiffBlastReport, DiffSeed};
pub use doc_model::{DocKind, DocScope, LayerHint, crate_of, symbol_role};
pub use docgen::{architecture_svg, module_svg};
pub use economy::{
    Budget, Cursor, CursorError, DEFAULT_PAGE, Page, SubListPage, Verbosity, multi_cursor,
    multi_truncation_note, paginate, paginate_sublist, truncation_note,
};
pub use errors::GraphError;
pub use fitness::{FitnessReport, FitnessRules, Violation};
pub use hotspot::{
    HotspotEntry, HotspotGrain, HotspotReport, file_hotspots, file_risk, symbol_hotspots,
};
pub use outline::{Outline, OutlineEntry, OutlineOptions, OutlineRequest, OutlineSymbol, assemble};
pub use plan_assist::{PlanAssist, PlanFile};
pub use refactor::{CycleBreakProposal, GodModuleFinding, MisplacedSymbol};
pub use span_lines::{FileSpanSource, FileSymbolSpans, line_starts, spans_from};
pub use symbol_churn::attribute_symbol_churn;
pub use test_impact::{AffectedTestsReport, TestRootInput, classify_test_symbols};
