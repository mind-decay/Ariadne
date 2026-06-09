//! `ariadne affected-tests <spec>` — the tests a change reaches (Block A, A1).
//!
//! Unlike the generic `query` route, `affected_tests` needs the git diff for
//! its changeset, so — like the MCP `affected_tests` tool — the diff
//! (`ariadne_git::diff`) runs here at the composition root (the daemon never
//! links git, RD7) and only the pre-computed hunks travel to the warm daemon.
//! Shared by the `affected-tests` subcommand (parses the CLI `<spec>` string)
//! and the `query affected_tests` route (parses the JSON `AffectedTestsInput`),
//! both threading the identical warm→cold plumbing the other tools use.

use std::path::Path;

use anyhow::{Context, Result, bail};
use ariadne_core::{
    DaemonQuery, DaemonResponse, DiffSpec, EdgeKindFilter as CoreEdgeKind,
    Verbosity as CoreVerbosity,
};
use ariadne_mcp::Catalog;
use ariadne_mcp::tools;
use ariadne_mcp::types::{
    AffectedTestsInput, AffectedTestsOutput, DiffSpecInput, EdgeKindFilter, Verbosity,
};
use ariadne_storage::RedbStorage;

use crate::adapters::daemon_client::DaemonClient;
use crate::domain::index_path;

/// Run the `affected-tests <spec>` subcommand: parse the CLI spec string, route
/// it warm→cold, and print the pretty JSON result.
///
/// # Errors
/// Fails when the git diff, the daemon, or the cold path errors, or — on the
/// cold path — the index is missing.
pub fn run(root: &Path, spec: &str) -> Result<()> {
    println!(
        "{}",
        route(
            root,
            &parse_spec(spec),
            None,
            None,
            None,
            None,
            Verbosity::default(),
        )?
    );
    Ok(())
}

/// Run the `query affected_tests '{json}'` route: parse the JSON tool input and
/// route it warm→cold, returning the pretty JSON result (so `ariadne query`
/// prints it verbatim).
///
/// # Errors
/// Fails when the arguments do not parse, or as [`run`] does.
pub fn run_query(root: &Path, args_json: &str) -> Result<String> {
    let input: AffectedTestsInput =
        serde_json::from_str(args_json).context("parse affected_tests arguments JSON")?;
    route(
        root,
        &to_core_spec(&input.spec),
        input.depth,
        input.kinds.as_deref(),
        input.limit,
        input.cursor.as_deref(),
        input.verbosity,
    )
}

/// Compute the changeset diff, then route the `affected_tests` query to the warm
/// daemon (cold in-process fallback), returning the pretty JSON result. The
/// daemon and cold report payloads `From`-project to the identical MCP wire
/// output (`AffectedTestsOutput`), so the JSON — including the tier-04 economy
/// cap / cursor / concise projection — is byte-identical on both paths.
// Each parameter is a distinct facet of the query; the tier-04 economy controls
// thread through to both serving paths, like the generic `query` route.
#[allow(clippy::too_many_arguments)]
fn route(
    root: &Path,
    spec: &DiffSpec,
    depth: Option<u8>,
    kinds: Option<&[EdgeKindFilter]>,
    limit: Option<u32>,
    cursor: Option<&str>,
    verbosity: Verbosity,
) -> Result<String> {
    let (hunks, changed_paths) = ariadne_git::diff(root, spec).context("compute git diff")?;

    let query = DaemonQuery::AffectedTests {
        hunks: hunks.clone(),
        changed_paths: changed_paths.clone(),
        depth,
        kinds: to_core_kinds(kinds),
        limit,
        cursor: cursor.map(ToOwned::to_owned),
        verbosity: to_core_verbosity(verbosity),
    };
    if let Some(resp) = DaemonClient::new(root).try_query(query) {
        return project(resp);
    }

    // Cold fallback: no daemon reachable — build the same catalog the MCP server
    // uses and run the per-tool handler in-process.
    let db_path = index_path(root);
    if !db_path.exists() {
        bail!(
            "no index at {} — run `ariadne index` first",
            db_path.display()
        );
    }
    let storage = RedbStorage::open(&db_path).context("open redb index")?;
    let catalog = Catalog::build(&storage, root.display().to_string()).context("build catalog")?;
    let out = tools::affected_tests::handle(
        &catalog,
        &storage,
        root,
        &hunks,
        &changed_paths,
        depth,
        kinds,
        limit,
        cursor,
        verbosity,
    )
    .context("run affected_tests over the cold catalog")?;
    serde_json::to_string_pretty(&out).context("serialize affected_tests output")
}

/// Project a daemon response into the pretty JSON the cold path produces. The
/// daemon report `From`-projects to the MCP wire output so the concise omission +
/// cursor/steer match the cold path byte-for-byte (parity).
fn project(resp: DaemonResponse) -> Result<String> {
    match resp {
        DaemonResponse::AffectedTests(report) => {
            serde_json::to_string_pretty(&AffectedTestsOutput::from(report))
                .context("serialize affected_tests report")
        }
        // `Error` is a query-level fault, `InvalidInput` a malformed / stale
        // cursor; the CLI has no JSON-RPC envelope (unlike the MCP path, which
        // maps them to distinct codes), so both just surface the message.
        DaemonResponse::Error(msg) | DaemonResponse::InvalidInput(msg) => bail!("{msg}"),
        other => bail!("daemon returned an unexpected response: {other:?}"),
    }
}

/// Map the MCP-facing verbosity onto the daemon protocol's verbosity (mirrors
/// `crate::commands::query::to_core_verbosity`).
fn to_core_verbosity(verbosity: Verbosity) -> CoreVerbosity {
    match verbosity {
        Verbosity::Concise => CoreVerbosity::Concise,
        Verbosity::Detailed => CoreVerbosity::Detailed,
    }
}

/// Parse a CLI `<spec>` string into a core [`DiffSpec`]: `working_tree` (or
/// empty) → working tree; `<from>..<to>` → ref range; anything else → a single
/// commit-ish.
fn parse_spec(spec: &str) -> DiffSpec {
    let s = spec.trim();
    if s.is_empty() || s == "working_tree" {
        DiffSpec::WorkingTree
    } else if let Some((from, to)) = s.split_once("..") {
        DiffSpec::RefRange {
            from: from.to_owned(),
            to: to.to_owned(),
        }
    } else {
        DiffSpec::Commit(s.to_owned())
    }
}

/// Map the MCP-facing diff spec onto the core [`DiffSpec`] the git adapter
/// resolves (mirrors `ariadne_mcp::server::to_core_spec`).
fn to_core_spec(spec: &DiffSpecInput) -> DiffSpec {
    match spec {
        DiffSpecInput::WorkingTree => DiffSpec::WorkingTree,
        DiffSpecInput::Commit(rev) => DiffSpec::Commit(rev.clone()),
        DiffSpecInput::RefRange { from, to } => DiffSpec::RefRange {
            from: from.clone(),
            to: to.clone(),
        },
    }
}

/// Map the MCP-facing edge-kind filter onto the daemon protocol's filter
/// (mirrors `crate::commands::query::to_core_kinds`).
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
