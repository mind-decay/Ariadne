//! `ariadne query` — in-process call to one MCP tool, JSON in, JSON out.
//!
//! Builds the same [`Catalog`] the MCP server uses, then dispatches to the
//! per-tool `handle` function directly — no JSON-RPC handshake. Useful for
//! shell / CI debugging [src: tier-10 step 7].

use std::path::Path;

use anyhow::{Context, Result, bail};
use ariadne_mcp::Catalog;
use ariadne_mcp::tools;
use ariadne_mcp::types::{
    BlastRadiusInput, FileQuery, ListSymbolsInput, PlanAssistInput, ScopeInput, SymbolQuery,
};
use ariadne_storage::RedbStorage;
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::domain::index_path;

/// Open the index, build the catalog, run `tool` against `args_json`, and
/// print the pretty JSON result.
///
/// # Errors
/// Fails when the index is missing, the tool name is unknown, the arguments
/// do not parse, or the tool itself returns an error.
pub fn run(root: &Path, tool: &str, args_json: &str) -> Result<()> {
    let db_path = index_path(root);
    if !db_path.exists() {
        bail!(
            "no index at {} — run `ariadne index` first",
            db_path.display()
        );
    }
    let storage = RedbStorage::open(&db_path).context("open redb index")?;
    let catalog = Catalog::build(&storage, root.display().to_string()).context("build catalog")?;
    println!("{}", dispatch(&catalog, &storage, tool, args_json)?);
    Ok(())
}

/// Route `tool` to its `ariadne_mcp::tools` handler.
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
        other => bail!("unknown tool `{other}`; see `ariadne query --help`"),
    }
}

/// Deserialize the JSON argument object into a tool input type.
fn parse<T: DeserializeOwned>(args: &str) -> Result<T> {
    serde_json::from_str(args).context("parse tool arguments JSON")
}

/// Serialize a tool output into pretty JSON.
fn json<T: Serialize>(value: &T) -> Result<String> {
    serde_json::to_string_pretty(value).context("serialize tool output")
}
