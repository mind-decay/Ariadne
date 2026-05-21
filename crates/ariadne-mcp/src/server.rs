//! `AriadneServer` — rmcp `#[tool_router]` host wiring the 13 Ariadne
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

    #[tool(
        description = "List symbols matching an optional substring + kind filter. Use \
when locating a symbol by name or kind before opening files; triggers: \"where is the X \
function\", \"list the structs in\"."
    )]
    async fn list_symbols(
        &self,
        Parameters(input): Parameters<ListSymbolsInput>,
    ) -> Result<CallToolResult, ErrorData> {
        let cat = &*self.catalog;
        let out = tools::list_symbols::handle(cat, &input);
        wire(&out)
    }

    #[tool(
        description = "Find the defining symbol record by canonical name. Use when you \
need the canonical definition site of a named symbol; triggers: \"where is X defined\", \
\"go to definition of\"."
    )]
    async fn find_definition(
        &self,
        Parameters(input): Parameters<SymbolQuery>,
    ) -> Result<CallToolResult, ErrorData> {
        let cat = &*self.catalog;
        let out = tools::find_definition::handle(cat, &input).map_err(McpError::into_rmcp)?;
        wire(&out)
    }

    #[tool(
        description = "List references to a symbol with source spans. Use when you need \
every use site of a symbol; triggers: \"who calls X\", \"where is X used\", \"find usages \
of\"."
    )]
    async fn find_references(
        &self,
        Parameters(input): Parameters<SymbolQuery>,
    ) -> Result<CallToolResult, ErrorData> {
        let cat = &*self.catalog;
        let out = tools::find_references::handle(cat, &*self.storage, &input)
            .map_err(McpError::into_rmcp)?;
        wire(&out)
    }

    #[tool(
        description = "Compute the blast radius (must-touch + may-touch) of a symbol. \
Use when assessing what a change to a symbol could break; triggers: \"what breaks if I \
change X\", \"impact of changing\", \"is it safe to edit\"."
    )]
    async fn blast_radius(
        &self,
        Parameters(input): Parameters<BlastRadiusInput>,
    ) -> Result<CallToolResult, ErrorData> {
        let cat = &*self.catalog;
        let out = tools::blast_radius::handle(cat, &input).map_err(McpError::into_rmcp)?;
        wire(&out)
    }

    #[tool(
        description = "Summarize a file: symbols, fan-in/out, top dependencies. Use when \
orienting in an unfamiliar file before reading it; triggers: \"what is in this file\", \
\"summarize src/X.rs\"."
    )]
    async fn file_summary(
        &self,
        Parameters(input): Parameters<FileQuery>,
    ) -> Result<CallToolResult, ErrorData> {
        let cat = &*self.catalog;
        let out = tools::file_summary::handle(cat, &*self.storage, &input)
            .map_err(McpError::into_rmcp)?;
        wire(&out)
    }

    #[tool(
        description = "Ranked plan-assist file list implicated by a symbol change. Use \
when scoping which files a change touches before editing; triggers: \"what files do I \
touch for X\", \"where do I start to change\"."
    )]
    async fn plan_assist(
        &self,
        Parameters(input): Parameters<PlanAssistInput>,
    ) -> Result<CallToolResult, ErrorData> {
        let cat = &*self.catalog;
        let out = tools::plan_assist::handle(cat, &input).map_err(McpError::into_rmcp)?;
        wire(&out)
    }

    #[tool(
        description = "Per-file Ca/Ce/I/A/Distance Martin coupling metrics. Use when \
assessing module dependency or architecture health; triggers: \"how coupled is\", \
\"Martin coupling metrics for\"."
    )]
    async fn coupling_report(
        &self,
        Parameters(input): Parameters<ScopeInput>,
    ) -> Result<CallToolResult, ErrorData> {
        let cat = &*self.catalog;
        let out = tools::coupling_report::handle(cat, &input);
        wire(&out)
    }

    #[tool(
        description = "Cycles, god modules, and dead-code candidates with reasons. Use \
when hunting cycles, god modules, or dead code; triggers: \"what is wrong with this \
codebase\", \"find tech debt\", \"any cycles\"."
    )]
    async fn weak_spots(
        &self,
        Parameters(input): Parameters<ScopeInput>,
    ) -> Result<CallToolResult, ErrorData> {
        let cat = &*self.catalog;
        let out = tools::weak_spots::handle(cat, &input);
        wire(&out)
    }

    #[tool(
        description = "Doc-like structured summary for one symbol. Use when you need a \
structured explanation of one symbol; triggers: \"what does X do\", \"explain the symbol \
X\"."
    )]
    async fn doc_for(
        &self,
        Parameters(input): Parameters<SymbolQuery>,
    ) -> Result<CallToolResult, ErrorData> {
        let cat = &*self.catalog;
        let out = tools::doc_for::handle(cat, &input).map_err(McpError::into_rmcp)?;
        wire(&out)
    }

    #[tool(
        description = "Project-wide counts, revision, and root. Use when checking index \
freshness or coverage before trusting results; triggers: \"is the index current\", \"how \
big is the project\"."
    )]
    async fn project_status(&self) -> Result<CallToolResult, ErrorData> {
        let cat = &*self.catalog;
        let out = tools::project_status::handle(cat);
        wire(&out)
    }

    #[tool(
        description = "Markdown documentation summary for one module (file path). Use \
when you need a doc-style summary of a file or module; triggers: \"document this \
module\", \"overview of src/X.rs\"."
    )]
    async fn doc_for_module(
        &self,
        Parameters(input): Parameters<FileQuery>,
    ) -> Result<CallToolResult, ErrorData> {
        let cat = &*self.catalog;
        let out =
            tools::doc_module::handle(cat, &*self.storage, &input).map_err(McpError::into_rmcp)?;
        wire(&out)
    }

    #[tool(
        description = "Markdown architecture overview for the whole project. Use when \
you need a whole-project architecture overview; triggers: \"explain the architecture\", \
\"how is this project structured\"."
    )]
    async fn doc_for_project(
        &self,
        Parameters(input): Parameters<ScopeInput>,
    ) -> Result<CallToolResult, ErrorData> {
        let cat = &*self.catalog;
        let out =
            tools::doc_project::handle(cat, &*self.storage, &input).map_err(McpError::into_rmcp)?;
        wire(&out)
    }

    #[tool(
        description = "Static refactor suggestions (god modules, cycle breaks, misplaced \
symbols). Use when you want concrete static refactor candidates; triggers: \"how should \
I refactor\", \"cleanup suggestions for\"."
    )]
    async fn refactor_suggestions(
        &self,
        Parameters(input): Parameters<ScopeInput>,
    ) -> Result<CallToolResult, ErrorData> {
        let cat = &*self.catalog;
        let out =
            tools::refactor::handle(cat, &*self.storage, &input).map_err(McpError::into_rmcp)?;
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
                "Ariadne is a read-only semantic graph of the local project: symbols, \
references, and dependency edges, kept current with the code. Prefer these tools over \
grep, Read, or file-walking for any question about symbols, references, impact, or \
architecture — the graph answers in one call where text search needs many and misses \
cross-file edges. Workflow: navigate with list_symbols, find_definition, and \
find_references; assess impact with blast_radius and plan_assist; check architecture \
health with coupling_report, weak_spots, and refactor_suggestions; read generated docs \
with doc_for, doc_for_module, and doc_for_project; verify index freshness with \
project_status. Call these even when the answer seems known — the graph reflects the \
current code, and assumptions may be stale.",
            )
    }
}

fn wire<T: serde::Serialize>(value: &T) -> Result<CallToolResult, ErrorData> {
    let json =
        serde_json::to_string(value).map_err(|e| ErrorData::internal_error(e.to_string(), None))?;
    Ok(CallToolResult::success(vec![Content::text(json)]))
}
