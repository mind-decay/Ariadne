//! Graph analytics use case. Tier-07 wires petgraph-backed in-RAM
//! reachability, dominators, SCC, coupling, dead-code, and plan-assist on
//! top of `ariadne-core` ports + `ariadne-storage` reads.
//!
//! All public surface is re-exported from this façade; the analytics live
//! in dedicated sibling modules so each stays under the project's
//! ≤200-line authored-file cap (CLAUDE.md `<rules>`).

#![deny(missing_docs)]

mod blast;
mod build;
mod co_change;
mod coupling;
mod cycles;
mod dead;
mod diff_blast;
pub mod docgen;
pub mod errors;
mod heuristics;
mod hotspot;
mod plan_assist;
pub mod refactor;
pub mod roots;
mod span_lines;
mod symbol_churn;

pub use blast::BlastRadius;
pub use build::{EdgeDelta, EdgeKind, EdgeKindSet, EdgeMeta, GraphIndex};
pub use co_change::{CoChangeConfig, CoChangeEdge, CoChangeReport, co_change_report};
pub use coupling::{CouplingMetrics, CouplingReport, ModuleSpec};
pub use cycles::{Cycle, CycleReport};
pub use dead::{DeadCodeConfig, DeadCodeReport, DeadSymbol};
pub use diff_blast::{DiffBlastReport, DiffSeed};
pub use errors::GraphError;
pub use hotspot::{HotspotEntry, HotspotGrain, HotspotReport, file_hotspots, symbol_hotspots};
pub use plan_assist::{PlanAssist, PlanFile};
pub use refactor::{CycleBreakProposal, GodModuleFinding, MisplacedSymbol};
pub use span_lines::{FileSpanSource, FileSymbolSpans, line_starts, spans_from};
pub use symbol_churn::attribute_symbol_churn;
