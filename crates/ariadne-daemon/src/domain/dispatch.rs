//! Query dispatch: map a [`DaemonQuery`] to the matching warm-graph use
//! case and project its result into a [`DaemonResponse`]. Pure over the
//! [`WarmCatalog`]; the transport adapter feeds requests in and frames the
//! responses out
//! [src: .claude/plans/post-v1-roadmap/tier-07-daemon-warm-graph.md step 5].

use ariadne_core::{DaemonQuery, DaemonResponse, SymbolId, SymbolSummary};

use crate::domain::catalog::WarmCatalog;
use crate::domain::queries::{analytics, docs, health, impact, meta, navigate, refactor};

/// Run `query` against the warm `catalog`, returning the wire response.
pub(crate) fn dispatch(catalog: &WarmCatalog, query: DaemonQuery) -> DaemonResponse {
    match query {
        DaemonQuery::Ping => DaemonResponse::Pong,
        DaemonQuery::ListSymbols { query, kind, limit } => {
            navigate::list_symbols(catalog, &query, kind.as_deref(), limit)
        }
        DaemonQuery::FindDefinition { symbol } => navigate::find_definition(catalog, &symbol),
        DaemonQuery::FindReferences {
            symbol,
            limit,
            cursor,
            verbosity,
        } => navigate::find_references(catalog, &symbol, limit, cursor.as_deref(), verbosity),
        DaemonQuery::BlastRadius {
            symbol,
            depth,
            kinds,
        } => impact::blast_radius(catalog, &symbol, depth, kinds.as_deref()),
        DaemonQuery::FileSummary { path } => impact::file_summary(catalog, &path),
        DaemonQuery::PlanAssist { symbol, max_files } => {
            impact::plan_assist(catalog, &symbol, max_files)
        }
        DaemonQuery::CouplingReport {
            prefix,
            limit,
            cursor,
            verbosity,
        } => health::coupling_report(
            catalog,
            prefix.as_deref(),
            limit,
            cursor.as_deref(),
            verbosity,
        ),
        DaemonQuery::WeakSpots { prefix } => health::weak_spots(catalog, prefix.as_deref()),
        DaemonQuery::DocFor { symbol } => docs::doc_for(catalog, &symbol),
        DaemonQuery::DocForModule { path } => docs::doc_for_module(catalog, &path),
        DaemonQuery::DocForProject { prefix } => docs::doc_for_project(catalog, prefix.as_deref()),
        DaemonQuery::ProjectStatus => meta::project_status(catalog),
        DaemonQuery::RefactorSuggestions { prefix } => {
            refactor::refactor_suggestions(catalog, prefix.as_deref())
        }
        DaemonQuery::Hotspots {
            prefix,
            grain,
            limit,
            cursor,
            verbosity,
        } => analytics::hotspots(
            catalog,
            prefix.as_deref(),
            grain,
            limit,
            cursor.as_deref(),
            verbosity,
        ),
        DaemonQuery::Complexity {
            prefix,
            grain,
            limit,
            cursor,
            verbosity,
        } => analytics::complexity(
            catalog,
            prefix.as_deref(),
            grain,
            limit,
            cursor.as_deref(),
            verbosity,
        ),
        DaemonQuery::CoChange {
            prefix,
            min_revs,
            min_shared_commits,
            min_degree,
            limit,
            cursor,
            verbosity,
        } => analytics::co_change(
            catalog,
            prefix.as_deref(),
            min_revs,
            min_shared_commits,
            min_degree,
            limit,
            cursor.as_deref(),
            verbosity,
        ),
        DaemonQuery::DiffBlast {
            hunks,
            changed_paths,
            depth,
            kinds,
        } => impact::diff_blast(catalog, &hunks, &changed_paths, depth, kinds.as_deref()),
        DaemonQuery::AffectedTests {
            hunks,
            changed_paths,
            depth,
            kinds,
        } => impact::affected_tests(catalog, &hunks, &changed_paths, depth, kinds.as_deref()),
    }
}

/// Project a [`SymbolId`] into the wire [`SymbolSummary`]. Unknown ids
/// collapse into an `<unknown>` placeholder, matching the v1 MCP projector. The
/// cryptic fields are populated (`Some`) — the lossless superset; a concise
/// projection drops them later (Block 1, tier-02 D3).
pub(crate) fn summarize(catalog: &WarmCatalog, id: SymbolId) -> SymbolSummary {
    match catalog.meta_of(id) {
        Some(m) => SymbolSummary {
            id: Some(id.get()),
            name: m.name.clone(),
            kind: m.kind.clone(),
            file: catalog.path_of(m.file).unwrap_or("").to_owned(),
            byte_start: Some(m.byte_start),
            byte_end: Some(m.byte_end),
        },
        None => SymbolSummary {
            id: Some(id.get()),
            name: String::from("<unknown>"),
            kind: String::new(),
            file: String::new(),
            byte_start: Some(0),
            byte_end: Some(0),
        },
    }
}
