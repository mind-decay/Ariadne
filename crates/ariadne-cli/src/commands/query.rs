//! `ariadne query` — run one MCP tool, JSON in, JSON out.
//!
//! Routes each query to the warm daemon over IPC (RD6) with the same
//! cold-path fallback the MCP server uses (tier-09): if no daemon is
//! reachable, build the same [`Catalog`] the MCP server uses and dispatch to
//! the per-tool `handle` function in-process. The daemon's report payloads
//! mirror the MCP output types field-for-field (tier-07), so the pretty JSON
//! is identical on both paths [src: tier-10 step 3].

use std::path::Path;

use anyhow::{Context, Result, bail};
use ariadne_core::{
    DaemonQuery, DaemonResponse, EdgeKindFilter as CoreEdgeKind, Grain as CoreGrain,
};
use ariadne_mcp::Catalog;
use ariadne_mcp::tools;
use ariadne_mcp::types::{
    BlastRadiusInput, CoChangeInput, EdgeKindFilter, FileQuery, Grain, GrainScopeInput,
    ListSymbolsInput, PlanAssistInput, ScopeInput, SymbolQuery,
};
use ariadne_storage::RedbStorage;
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::adapters::daemon_client::DaemonClient;
use crate::domain::index_path;

/// Route `tool` against `args_json` to the warm daemon, falling back to the
/// cold in-process path, and print the pretty JSON result.
///
/// # Errors
/// Fails when the arguments do not parse, the daemon (or cold path) reports a
/// query-level error, or — on the cold path — the index is missing or the tool
/// name is unknown.
pub fn run(root: &Path, tool: &str, args_json: &str) -> Result<()> {
    println!("{}", run_tool(root, tool, args_json)?);
    Ok(())
}

/// Route `tool` against `args_json` to the warm daemon, falling back to the
/// cold in-process path, and return its result as pretty JSON text.
///
/// Serializing each tool's typed output struct directly (rather than through an
/// order-less `serde_json::Value`) keeps the keys in struct-declaration order,
/// so `ariadne query` prints `revision` first as it did before the digest
/// refactor [src: audit/tier-02-report.md F1].
///
/// Shared by `ariadne query` (which prints it verbatim) and `ariadne digest`
/// (which re-parses each result into a `Value` and composes them into bounded
/// Markdown) so both resolve through the identical daemon/cold plumbing
/// [src: tier-02 step 2].
///
/// # Errors
/// Fails when the arguments do not parse, the daemon (or cold path) reports a
/// query-level error, or — on the cold path — the index is missing or the tool
/// name is unknown.
pub fn run_tool(root: &Path, tool: &str, args_json: &str) -> Result<String> {
    // `affected_tests` needs the git diff for its changeset, which runs at the
    // composition root (the daemon never links git, RD7), so it routes through
    // the dedicated command rather than the generic daemon/cold dispatch below.
    if tool == "affected_tests" {
        return crate::commands::affected_tests::run_query(root, args_json);
    }

    if let Some(json) = try_daemon(root, tool, args_json)? {
        return Ok(json);
    }

    // Cold fallback: no daemon reachable (or an unknown tool the daemon
    // protocol has no variant for — the cold dispatcher raises the canonical
    // "unknown tool" error).
    let db_path = index_path(root);
    if !db_path.exists() {
        bail!(
            "no index at {} — run `ariadne index` first",
            db_path.display()
        );
    }
    let storage = RedbStorage::open(&db_path).context("open redb index")?;
    let catalog = Catalog::build(&storage, root.display().to_string()).context("build catalog")?;
    dispatch(&catalog, &storage, tool, args_json)
}

/// Try the warm daemon. Returns the projected JSON text on a daemon answer,
/// `None` when no daemon is reachable or the tool has no daemon-protocol
/// variant (so the caller falls back to the cold path).
///
/// # Errors
/// Propagates an argument-parse failure, or a query-level daemon error
/// (not-found), so the daemon and cold paths surface the same failure.
fn try_daemon(root: &Path, tool: &str, args_json: &str) -> Result<Option<String>> {
    let Some(query) = build_query(tool, args_json)? else {
        return Ok(None);
    };
    let Some(resp) = DaemonClient::new(root).try_query(query) else {
        return Ok(None);
    };
    project(resp).map(Some)
}

/// Map `tool` + JSON arguments to a [`DaemonQuery`]. Returns `None` for a tool
/// the daemon protocol has no variant for, so the caller cold-dispatches it.
///
/// # Errors
/// Propagates a JSON argument-parse failure.
fn build_query(tool: &str, args: &str) -> Result<Option<DaemonQuery>> {
    let query = match tool {
        "list_symbols" => {
            let i = parse::<ListSymbolsInput>(args)?;
            DaemonQuery::ListSymbols {
                query: i.query,
                kind: i.kind,
                limit: i.limit,
            }
        }
        "find_definition" => DaemonQuery::FindDefinition {
            symbol: parse::<SymbolQuery>(args)?.symbol,
        },
        "find_references" => DaemonQuery::FindReferences {
            symbol: parse::<SymbolQuery>(args)?.symbol,
        },
        "blast_radius" => {
            let i = parse::<BlastRadiusInput>(args)?;
            DaemonQuery::BlastRadius {
                symbol: i.symbol,
                depth: i.depth,
                kinds: to_core_kinds(i.kinds.as_deref()),
            }
        }
        "file_summary" => DaemonQuery::FileSummary {
            path: parse::<FileQuery>(args)?.path,
        },
        "plan_assist" => {
            let i = parse::<PlanAssistInput>(args)?;
            DaemonQuery::PlanAssist {
                symbol: i.symbol,
                max_files: i.max_files,
            }
        }
        "coupling_report" => DaemonQuery::CouplingReport {
            prefix: parse::<ScopeInput>(args)?.prefix,
        },
        "weak_spots" => DaemonQuery::WeakSpots {
            prefix: parse::<ScopeInput>(args)?.prefix,
        },
        "doc_for" => DaemonQuery::DocFor {
            symbol: parse::<SymbolQuery>(args)?.symbol,
        },
        "project_status" => DaemonQuery::ProjectStatus,
        "doc_for_module" => DaemonQuery::DocForModule {
            path: parse::<FileQuery>(args)?.path,
        },
        "doc_for_project" => DaemonQuery::DocForProject {
            prefix: parse::<ScopeInput>(args)?.prefix,
        },
        "refactor_suggestions" => DaemonQuery::RefactorSuggestions {
            prefix: parse::<ScopeInput>(args)?.prefix,
        },
        "hotspots" => {
            let i = parse::<GrainScopeInput>(args)?;
            DaemonQuery::Hotspots {
                prefix: i.prefix,
                grain: to_core_grain(i.grain),
            }
        }
        "complexity" => {
            let i = parse::<GrainScopeInput>(args)?;
            DaemonQuery::Complexity {
                prefix: i.prefix,
                grain: to_core_grain(i.grain),
            }
        }
        "co_change" => {
            let i = parse::<CoChangeInput>(args)?;
            DaemonQuery::CoChange {
                prefix: i.prefix,
                min_revs: i.min_revs,
                min_shared_commits: i.min_shared_commits,
                min_degree: i.min_degree,
            }
        }
        _ => return Ok(None),
    };
    Ok(Some(query))
}

/// Project a daemon [`DaemonResponse`] into the same pretty JSON text the cold
/// path produces. Each report payload mirrors the matching MCP output type
/// field-for-field (tier-07), so serializing it yields the same JSON the cold
/// path produces. A query-level [`DaemonResponse::Error`] becomes the same
/// not-found failure the cold path raises.
///
/// # Errors
/// Returns the daemon's query-level error, a serialization failure, or a
/// protocol fault (a `Pong` answer to a tool query).
fn project(resp: DaemonResponse) -> Result<String> {
    match resp {
        DaemonResponse::Symbols(rows) => json(&rows),
        DaemonResponse::Definition(sym) => json(&sym),
        DaemonResponse::References(rows) => json(&rows),
        DaemonResponse::BlastRadius(report) => json(&report),
        DaemonResponse::FileSummary(report) => json(&report),
        DaemonResponse::PlanAssist(report) => json(&report),
        DaemonResponse::Coupling(report) => json(&report),
        DaemonResponse::WeakSpots(report) => json(&report),
        DaemonResponse::DocFor(report) => json(&report),
        DaemonResponse::Doc(report) => json(&report),
        DaemonResponse::ProjectStatus(report) => json(&report),
        DaemonResponse::Refactor(report) => json(&report),
        DaemonResponse::Hotspots(report) => json(&report),
        DaemonResponse::Complexity(report) => json(&report),
        DaemonResponse::CoChange(report) => json(&report),
        // The CLI `query` subcommand does not expose `diff_blast_radius` (it is
        // an MCP tool — tier-15c), so the daemon never returns this for a CLI
        // request; the arm keeps the projection exhaustive against the shared
        // protocol enum.
        DaemonResponse::DiffBlast(report) => json(&report),
        // `query affected_tests` routes through `commands::affected_tests`, which
        // projects the response itself; this generic path never sees it, but the
        // arm keeps the projection exhaustive against the shared protocol enum.
        DaemonResponse::AffectedTests(report) => json(&report),
        DaemonResponse::Error(msg) => bail!("{msg}"),
        DaemonResponse::Pong => bail!("daemon answered Pong to a tool query"),
    }
}

/// Map the MCP-facing grain onto the daemon protocol's grain (mirrors
/// `crate::server::to_core_grain` in `ariadne-mcp`).
fn to_core_grain(grain: Grain) -> CoreGrain {
    match grain {
        Grain::File => CoreGrain::File,
        Grain::Symbol => CoreGrain::Symbol,
    }
}

/// Map the MCP-facing edge-kind filter onto the daemon protocol's filter
/// (mirrors `crate::server::to_core_kinds` in `ariadne-mcp`).
fn to_core_kinds(kinds: Option<&[EdgeKindFilter]>) -> Option<Vec<CoreEdgeKind>> {
    kinds.map(|ks| {
        ks.iter()
            .map(|k| match k {
                EdgeKindFilter::Calls => CoreEdgeKind::Calls,
                EdgeKindFilter::Imports => CoreEdgeKind::Imports,
                EdgeKindFilter::TypeOf => CoreEdgeKind::TypeOf,
                EdgeKindFilter::Defines => CoreEdgeKind::Defines,
                EdgeKindFilter::Overrides => CoreEdgeKind::Overrides,
                EdgeKindFilter::Reads => CoreEdgeKind::Reads,
                EdgeKindFilter::Writes => CoreEdgeKind::Writes,
                EdgeKindFilter::Inherits => CoreEdgeKind::Inherits,
            })
            .collect()
    })
}

/// Route `tool` to its `ariadne_mcp::tools` handler (the cold in-process path).
fn dispatch(cat: &Catalog, storage: &RedbStorage, tool: &str, args: &str) -> Result<String> {
    match tool {
        "list_symbols" => json(&tools::list_symbols::handle(
            cat,
            &parse::<ListSymbolsInput>(args)?,
        )),
        "find_definition" => json(&tools::find_definition::handle(
            cat,
            &parse::<SymbolQuery>(args)?,
        )?),
        "find_references" => json(&tools::find_references::handle(
            cat,
            storage,
            &parse::<SymbolQuery>(args)?,
        )?),
        "blast_radius" => json(&tools::blast_radius::handle(
            cat,
            &parse::<BlastRadiusInput>(args)?,
        )?),
        "file_summary" => json(&tools::file_summary::handle(
            cat,
            storage,
            &parse::<FileQuery>(args)?,
        )?),
        "plan_assist" => json(&tools::plan_assist::handle(
            cat,
            &parse::<PlanAssistInput>(args)?,
        )?),
        "coupling_report" => json(&tools::coupling_report::handle(
            cat,
            &parse::<ScopeInput>(args)?,
        )),
        "weak_spots" => json(&tools::weak_spots::handle(cat, &parse::<ScopeInput>(args)?)),
        "doc_for" => json(&tools::doc_for::handle(cat, &parse::<SymbolQuery>(args)?)?),
        "project_status" => json(&tools::project_status::handle(cat)),
        "doc_for_module" => json(&tools::doc_module::handle(
            cat,
            storage,
            &parse::<FileQuery>(args)?,
        )?),
        "doc_for_project" => json(&tools::doc_project::handle(
            cat,
            storage,
            &parse::<ScopeInput>(args)?,
        )?),
        "refactor_suggestions" => json(&tools::refactor::handle(
            cat,
            storage,
            &parse::<ScopeInput>(args)?,
        )?),
        "hotspots" => json(&tools::hotspots::handle(
            cat,
            &parse::<GrainScopeInput>(args)?,
        )),
        "complexity" => json(&tools::complexity::handle(
            cat,
            &parse::<GrainScopeInput>(args)?,
        )),
        "co_change" => json(&tools::co_change::handle(
            cat,
            &parse::<CoChangeInput>(args)?,
        )),
        other => bail!("unknown tool `{other}`; see `ariadne query --help`"),
    }
}

/// Deserialize the JSON argument object into a tool input type.
fn parse<T: DeserializeOwned>(args: &str) -> Result<T> {
    serde_json::from_str(args).context("parse tool arguments JSON")
}

/// Serialize a tool output struct into pretty JSON text. Serializing the typed
/// struct directly preserves field-declaration order (an order-less
/// `serde_json::Value` round-trip would re-sort the keys alphabetically)
/// [src: audit/tier-02-report.md F1].
fn json<T: Serialize>(value: &T) -> Result<String> {
    serde_json::to_string_pretty(value).context("serialize tool output")
}
