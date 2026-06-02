//! Daemon IPC wire types shared between the daemon host (`ariadne-daemon`)
//! and any driving adapter that speaks to it over the local socket.
//!
//! These types are pure data. They carry `serde` derives so a transport
//! adapter can frame them, but the *codec* choice (postcard, JSON, …) stays
//! in the adapter and never leaks into the domain interior
//! [src: docs/adr/0015-daemon-mode-ipc.md]. The tier-06 skeleton modelled
//! only the `Ping`/`Pong` liveness handshake; tier-07 extends the set with
//! one variant per v1 read query and a `revision` staleness handshake
//! [src: .claude/plans/post-v1-roadmap/tier-07-daemon-warm-graph.md].

mod query;
mod response;
mod rows;

use serde::{Deserialize, Serialize};

pub use query::{DaemonQuery, EdgeKindFilter, Grain};
pub use response::{
    BlastRadiusReport, CoChangeReport, ComplexityReport, CouplingReport, DaemonResponse,
    DocForReport, DocReport, FileSummaryReport, HotspotReport, PlanAssistReport,
    ProjectStatusReport, RefactorReport, WeakSpotsReport,
};
pub use rows::{
    CoChangeEdge, ComplexityRow, ComponentRow, CouplingRow, CycleBreakRow, CycleRow, DependencyRow,
    GodModuleRow, HotspotRow, MisplacedRow, OutboundRow, PlanFileRow, ReferenceSite, SymbolSummary,
};

/// A request a client sends to the daemon over the local socket.
///
/// Every request carries the latest redb `revision` the client has
/// observed. The daemon compares it against the revision its warm graph was
/// built from and refreshes the graph before answering when the client has
/// seen a newer index (risk R-B2). Liveness probes pass `revision: 0`,
/// which never triggers a refresh.
///
/// `Eq` is not derived: the wrapped [`DaemonQuery`] carries an `f32` threshold
/// (`CoChange`), so the request is only `PartialEq` (tier-15b).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DaemonRequest {
    /// Latest redb revision the client has observed (`0` = unknown).
    pub revision: u64,
    /// The query to run against the warm graph.
    pub query: DaemonQuery,
}

impl DaemonRequest {
    /// A liveness probe: [`DaemonQuery::Ping`] with no revision
    /// expectation. The daemon answers [`DaemonResponse::Pong`] and never
    /// refreshes for it.
    #[must_use]
    pub fn ping() -> Self {
        Self {
            revision: 0,
            query: DaemonQuery::Ping,
        }
    }
}
