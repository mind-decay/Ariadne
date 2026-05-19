//! `AriadneServer` — rmcp `#[tool_router]` host wiring the 10 Ariadne
//! analytics into MCP. Each `#[tool]` method delegates to the per-tool
//! module under [`crate::tools`].
//!
//! Concurrency model — tier-08 step 7: the [`Catalog`] is built once,
//! held behind an [`Arc`], and read-only for the server lifetime; many
//! `#[tool]` futures run in parallel without contention
//! (`8 simultaneous tool calls in <100ms p95` per the tier's
//! `<exit_criteria>`). Tier-10 orchestration will swap the catalog
//! `Arc` atomically when the watcher pipeline commits a revision; this
//! crate ships only the read path.
//!
//! `AriadneDb` is deliberately omitted from the server state. The plan
//! letter sketches `db: Arc<RwLock<AriadneDb>>`, but salsa's database
//! handle is `!Sync` (its ingredient store wraps `UnsafeCell`), and
//! tier-04 explicitly stubs the derived queries the MCP surface would
//! call — the field would carry no functional payload in tier-08 and
//! would prevent every `#[tool]` future from being `Send`. Tier-10
//! revisits the integration alongside the watcher pipeline.

use std::sync::Arc;

use ariadne_storage::RedbStorage;
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{
    CallToolResult, Content, Implementation, ProtocolVersion, ServerCapabilities, ServerInfo,
};
use rmcp::{ErrorData, ServerHandler, tool, tool_handler, tool_router};

use crate::catalog::Catalog;
use crate::errors::McpError;
use crate::tools;
use crate::types::{
    BlastRadiusInput, FileQuery, ListSymbolsInput, PlanAssistInput, ScopeInput, SymbolQuery,
};

/// MCP server backing the Ariadne analytics tools. Clone-friendly so the
/// rmcp service layer can hand it across tasks.
#[derive(Clone)]
pub struct AriadneServer {
    storage: Arc<RedbStorage>,
    catalog: Arc<Catalog>,
    // Populated by the `#[tool_router]` macro and consumed by the
    // `#[tool_handler]` macro expansion; the rustc reachability pass
    // can't see through the macro path so it warns. The field is load-
    // bearing: removing it breaks rmcp's tool dispatch.
    #[allow(dead_code)]
    tool_router: ToolRouter<Self>,
}

impl std::fmt::Debug for AriadneServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AriadneServer")
            .field("storage", &"<RedbStorage>")
            .finish_non_exhaustive()
    }
}

#[tool_router]
impl AriadneServer {
    /// Build a fresh server around an opened storage + an already-built
    /// in-RAM [`Catalog`].
    #[must_use]
    pub fn new(storage: Arc<RedbStorage>, catalog: Catalog) -> Self {
        Self {
            storage,
            catalog: Arc::new(catalog),
            tool_router: Self::tool_router(),
        }
    }

    /// Surface the catalog for in-process tests and benches.
    #[must_use]
    pub fn catalog_arc(&self) -> Arc<Catalog> {
        Arc::clone(&self.catalog)
    }

    /// Surface the storage handle for in-process tests and benches.
    #[must_use]
    pub fn storage(&self) -> Arc<RedbStorage> {
        Arc::clone(&self.storage)
    }

    #[tool(description = "List symbols matching an optional substring + kind filter")]
    async fn list_symbols(
        &self,
        Parameters(input): Parameters<ListSymbolsInput>,
    ) -> Result<CallToolResult, ErrorData> {
        let cat = &*self.catalog;
        let out = tools::list_symbols::handle(cat, &input);
        wire(&out)
    }

    #[tool(description = "Find the defining symbol record by canonical name")]
    async fn find_definition(
        &self,
        Parameters(input): Parameters<SymbolQuery>,
    ) -> Result<CallToolResult, ErrorData> {
        let cat = &*self.catalog;
        let out = tools::find_definition::handle(cat, &input).map_err(McpError::into_rmcp)?;
        wire(&out)
    }

    #[tool(description = "List references to a symbol with source spans")]
    async fn find_references(
        &self,
        Parameters(input): Parameters<SymbolQuery>,
    ) -> Result<CallToolResult, ErrorData> {
        let cat = &*self.catalog;
        let out = tools::find_references::handle(cat, &*self.storage, &input)
            .map_err(McpError::into_rmcp)?;
        wire(&out)
    }

    #[tool(description = "Compute the blast radius (must-touch + may-touch) of a symbol")]
    async fn blast_radius(
        &self,
        Parameters(input): Parameters<BlastRadiusInput>,
    ) -> Result<CallToolResult, ErrorData> {
        let cat = &*self.catalog;
        let out = tools::blast_radius::handle(cat, &input).map_err(McpError::into_rmcp)?;
        wire(&out)
    }

    #[tool(description = "Summarize a file: symbols, fan-in/out, top dependencies")]
    async fn file_summary(
        &self,
        Parameters(input): Parameters<FileQuery>,
    ) -> Result<CallToolResult, ErrorData> {
        let cat = &*self.catalog;
        let out = tools::file_summary::handle(cat, &*self.storage, &input)
            .map_err(McpError::into_rmcp)?;
        wire(&out)
    }

    #[tool(description = "Ranked plan-assist file list implicated by a symbol change")]
    async fn plan_assist(
        &self,
        Parameters(input): Parameters<PlanAssistInput>,
    ) -> Result<CallToolResult, ErrorData> {
        let cat = &*self.catalog;
        let out = tools::plan_assist::handle(cat, &input).map_err(McpError::into_rmcp)?;
        wire(&out)
    }

    #[tool(description = "Per-file Ca/Ce/I/A/Distance Martin coupling metrics")]
    async fn coupling_report(
        &self,
        Parameters(input): Parameters<ScopeInput>,
    ) -> Result<CallToolResult, ErrorData> {
        let cat = &*self.catalog;
        let out = tools::coupling_report::handle(cat, &input);
        wire(&out)
    }

    #[tool(description = "Cycles, god modules, and dead-code candidates with reasons")]
    async fn weak_spots(
        &self,
        Parameters(input): Parameters<ScopeInput>,
    ) -> Result<CallToolResult, ErrorData> {
        let cat = &*self.catalog;
        let out = tools::weak_spots::handle(cat, &input);
        wire(&out)
    }

    #[tool(description = "Doc-like structured summary for one symbol")]
    async fn doc_for(
        &self,
        Parameters(input): Parameters<SymbolQuery>,
    ) -> Result<CallToolResult, ErrorData> {
        let cat = &*self.catalog;
        let out = tools::doc_for::handle(cat, &input).map_err(McpError::into_rmcp)?;
        wire(&out)
    }

    #[tool(description = "Project-wide counts, revision, and root")]
    async fn project_status(&self) -> Result<CallToolResult, ErrorData> {
        let cat = &*self.catalog;
        let out = tools::project_status::handle(cat);
        wire(&out)
    }
}

#[tool_handler]
impl ServerHandler for AriadneServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::from_build_env())
            .with_protocol_version(ProtocolVersion::V_2024_11_05)
            .with_instructions(
                "Ariadne code-intelligence tools: read-only graph analytics over the local \
project. Use list_symbols / find_definition to navigate, blast_radius and \
plan_assist for impact analysis, coupling_report and weak_spots for \
architecture health, and project_status for index freshness.",
            )
    }
}

fn wire<T: serde::Serialize>(value: &T) -> Result<CallToolResult, ErrorData> {
    let json =
        serde_json::to_string(value).map_err(|e| ErrorData::internal_error(e.to_string(), None))?;
    Ok(CallToolResult::success(vec![Content::text(json)]))
}
