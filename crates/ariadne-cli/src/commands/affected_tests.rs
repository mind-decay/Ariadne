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
use ariadne_core::{DaemonQuery, DaemonResponse, DiffSpec, EdgeKindFilter as CoreEdgeKind};
use ariadne_mcp::Catalog;
use ariadne_mcp::tools;
use ariadne_mcp::types::{AffectedTestsInput, DiffSpecInput, EdgeKindFilter};
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
    println!("{}", route(root, &parse_spec(spec), None, None)?);
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
    )
}

/// Compute the changeset diff, then route the `affected_tests` query to the warm
/// daemon (cold in-process fallback), returning the pretty JSON result. The
/// daemon and cold report payloads mirror each other field-for-field, so the
/// JSON is identical on both paths.
fn route(
    root: &Path,
    spec: &DiffSpec,
    depth: Option<u8>,
    kinds: Option<&[EdgeKindFilter]>,
) -> Result<String> {
    let (hunks, changed_paths) = ariadne_git::diff(root, spec).context("compute git diff")?;

    let query = DaemonQuery::AffectedTests {
        hunks: hunks.clone(),
        changed_paths: changed_paths.clone(),
        depth,
        kinds: to_core_kinds(kinds),
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
    )
    .context("run affected_tests over the cold catalog")?;
    serde_json::to_string_pretty(&out).context("serialize affected_tests output")
}

/// Project a daemon response into the pretty JSON the cold path produces.
fn project(resp: DaemonResponse) -> Result<String> {
    match resp {
        DaemonResponse::AffectedTests(report) => {
            serde_json::to_string_pretty(&report).context("serialize affected_tests report")
        }
        DaemonResponse::Error(msg) => bail!("{msg}"),
        other => bail!("daemon returned an unexpected response: {other:?}"),
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
