//! Request side of the daemon protocol: the query enum and its edge-kind
//! filter. One variant per v1 read query, mirroring the MCP tool inputs so
//! no new request shape is invented
//! [src: .claude/plans/post-v1-roadmap/tier-07-daemon-warm-graph.md step 2].

use serde::{Deserialize, Serialize};

/// Edge-kind filter a client passes to [`DaemonQuery::BlastRadius`].
/// Mirrors the MCP `EdgeKindFilter`; the daemon maps it to the in-RAM
/// graph's edge-kind set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
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

/// Grain a [`DaemonQuery::Hotspots`] / [`DaemonQuery::Complexity`] ranks at:
/// per-file (Σ over the file's symbols) or per-symbol. Mirrors the MCP
/// `Grain`; the daemon maps it to the matching graph use case (tier-15b D2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Grain {
    /// File grain: one row per file, complexity summed over its symbols.
    File,
    /// Symbol grain: one row per symbol, carrying its own complexity.
    Symbol,
}

/// One v1 read query the daemon answers against its warm graph.
///
/// Deliberately *not* `#[non_exhaustive]`: the transport adapter matches it
/// exhaustively, so adding a variant fails to compile until the dispatcher
/// learns it — the coupling we want for an internal protocol.
///
/// `Eq` is not derived: [`DaemonQuery::CoChange`] carries an `f32` coupling
/// threshold, so the enum is only `PartialEq` (sufficient for the test
/// assertions; the protocol is never a map key — tier-15b).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DaemonQuery {
    /// Liveness probe. Answered with `DaemonResponse::Pong`.
    Ping,
    /// Substring + kind filter over canonical names.
    ListSymbols {
        /// Case-insensitive substring on canonical name; empty = no filter.
        query: String,
        /// Optional exact kind filter.
        kind: Option<String>,
        /// Maximum rows; the daemon defaults to 64.
        limit: Option<u32>,
    },
    /// Resolve a canonical name to its defining symbol.
    FindDefinition {
        /// Canonical name to resolve.
        symbol: String,
    },
    /// Reference sites whose target is the named symbol.
    FindReferences {
        /// Canonical name of the referenced symbol.
        symbol: String,
    },
    /// Reverse-reachability blast radius of a symbol.
    BlastRadius {
        /// Target symbol canonical name.
        symbol: String,
        /// Reverse-BFS hop limit; the daemon defaults to 3.
        depth: Option<u8>,
        /// Edge-kind filter; empty / missing = all kinds.
        kinds: Option<Vec<EdgeKindFilter>>,
    },
    /// Per-file roll-up: symbols, fan-in/out, top deps, components.
    FileSummary {
        /// Project-root-relative file path.
        path: String,
    },
    /// Ranked file list implicated by changing a symbol.
    PlanAssist {
        /// Target symbol canonical name.
        symbol: String,
        /// Maximum file rows; the daemon defaults to 16.
        max_files: Option<u32>,
    },
    /// Per-file Martin coupling metrics.
    CouplingReport {
        /// Optional path-prefix scope (project-root-relative).
        prefix: Option<String>,
    },
    /// Cycles ∪ god modules ∪ dead-code candidates.
    WeakSpots {
        /// Optional path-prefix scope (project-root-relative).
        prefix: Option<String>,
    },
    /// Structured doc summary for one symbol.
    DocFor {
        /// Target symbol canonical name.
        symbol: String,
    },
    /// Markdown documentation for one module (file path).
    DocForModule {
        /// Module identity = project-root-relative file path.
        path: String,
    },
    /// Markdown architecture overview for the project.
    DocForProject {
        /// Optional path-prefix scope (project-root-relative).
        prefix: Option<String>,
    },
    /// Coarse counts + persisted revision of the indexed project.
    ProjectStatus,
    /// God modules ∪ cycle breaks ∪ misplaced symbols — refactor hints.
    RefactorSuggestions {
        /// Optional path-prefix scope (project-root-relative).
        prefix: Option<String>,
    },
    /// Churn × complexity hotspots at the requested grain (tier-15b).
    Hotspots {
        /// Optional path-prefix scope (project-root-relative).
        prefix: Option<String>,
        /// File or symbol grain.
        grain: Grain,
    },
    /// `McCabe` cyclomatic complexity ranking at the requested grain (tier-15b).
    Complexity {
        /// Optional path-prefix scope (project-root-relative).
        prefix: Option<String>,
        /// File (Σ over the file's symbols) or symbol grain.
        grain: Grain,
    },
    /// Logical-coupling (change-coupling) edges honoring the code-maat filters
    /// (tier-15b). Each threshold is optional; an absent one falls back to the
    /// `CoChangeConfig` default at the handler.
    CoChange {
        /// Optional path-prefix scope: keeps an edge when either endpoint
        /// path is in scope (project-root-relative).
        prefix: Option<String>,
        /// Minimum individual revisions per endpoint; `None` = default.
        min_revs: Option<u32>,
        /// Minimum shared-commit support per pair; `None` = default.
        min_shared_commits: Option<u32>,
        /// Minimum coupling degree ∈ [0, 1]; `None` = default.
        min_degree: Option<f32>,
    },
}
