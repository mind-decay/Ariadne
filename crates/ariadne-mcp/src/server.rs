//! `AriadneServer` — rmcp `#[tool_router]` host wiring the 17 Ariadne
//! analytics into MCP. Each `#[tool]` method routes its query to the warm
//! daemon over IPC (RD6) and projects the [`DaemonResponse`] into the v1 tool
//! output shape; when no daemon is reachable it falls back to the per-tool
//! module under [`crate::tools`] reading the cold [`Catalog`]. The output
//! shape is identical on both paths, so the v1 goldens hold unchanged
//! [src: .claude/plans/post-v1-roadmap/tier-09-mcp-daemon-client.md].
//!
//! Concurrency model — tier-08 step 7: the [`Catalog`] is built at most
//! once, held behind an [`Arc`], and read-only for the server lifetime; many
//! `#[tool]` futures run in parallel without contention. The build itself is
//! deferred to the first cold-fallback miss (see `AriadneServer::catalog`),
//! so a session whose tools all route to the warm daemon never pays it. The
//! [`DaemonClient`] is a stateless connector (one short-lived socket per
//! query), but its socket IO and auto-spawn wait are synchronous, so each
//! handler routes through [`DaemonClient::try_query_async`], which offloads
//! the round-trip to `tokio::task::spawn_blocking`. A slow or missing daemon
//! therefore parks a blocking-pool thread, never a runtime worker, and the
//! executor stays free to drive other tool futures — so the same parallelism
//! holds for the daemon path.
//!
//! `AriadneDb` is deliberately omitted from the server state. The plan
//! letter sketches `db: Arc<RwLock<AriadneDb>>`, but salsa's database
//! handle is `!Sync` (its ingredient store wraps `UnsafeCell`), and
//! tier-04 explicitly stubs the derived queries the MCP surface would
//! call — the field would carry no functional payload here and would
//! prevent every `#[tool]` future from being `Send`.

use std::path::PathBuf;
use std::sync::Arc;

use ariadne_core::{
    DaemonQuery, DaemonResponse, EdgeKindFilter as CoreEdgeKind, Grain as CoreGrain,
};
use ariadne_storage::RedbStorage;
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{
    CallToolResult, Content, Implementation, ProtocolVersion, ServerCapabilities, ServerInfo,
};
use rmcp::{ErrorData, ServerHandler, tool, tool_handler, tool_router};
use tokio::sync::OnceCell;

use crate::DaemonClient;
use crate::catalog::Catalog;
use crate::errors::McpError;
use crate::tools;
use crate::types::{
    BlastRadiusInput, CoChangeInput, DiffBlastInput, DiffSpecInput, EdgeKindFilter, FileQuery,
    Grain, GrainScopeInput, ListSymbolsInput, PlanAssistInput, ReadSymbolInput, ScopeInput,
    SearchCodeInput, SymbolQuery,
};

/// MCP server backing the Ariadne analytics tools. Clone-friendly so the
/// rmcp service layer can hand it across tasks.
#[derive(Clone)]
pub struct AriadneServer {
    /// Path to `<root>/.ariadne/index.redb`. The server deliberately does
    /// **not** hold an open redb handle: a held handle takes redb's
    /// single-open lock for the server's lifetime, which would stop the daemon
    /// it auto-spawns — and any running daemon's staleness refresh — from
    /// opening the same index, deadlocking the warm path. Cold-fallback arms
    /// open redb transiently via [`Self::open_storage`] only when the daemon is
    /// unreachable [src: tier-10 build — fixes the `mcp_session` autospawn
    /// deadlock surfaced by the workspace SLO gate].
    db_path: PathBuf,
    /// Project root the daemon client targets and the catalog is built from.
    /// Held directly (not read off `catalog.root`) because the catalog may be
    /// unbuilt for the whole session.
    root: PathBuf,
    /// Last-observed redb revision, read cheaply at startup (single
    /// `KEY_REVISION` lookup, no graph build) and sent in every daemon
    /// handshake so a daemon behind the client refreshes before answering.
    revision: u64,
    /// In-RAM cold-fallback [`Catalog`], built lazily on the first cold miss
    /// via [`Self::catalog`]. Empty while the daemon answers, so a warm
    /// session never reads the full index or allocates the petgraph. The
    /// `Arc<OnceCell<…>>` is shared across server clones, so concurrent first
    /// misses build it once and the rest await that build.
    catalog: Arc<OnceCell<Arc<Catalog>>>,
    daemon: DaemonClient,
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
            .field("db_path", &self.db_path)
            .finish_non_exhaustive()
    }
}

#[tool_router]
impl AriadneServer {
    /// Build a fresh server around the on-disk index path, project `root`, and
    /// the startup-read redb `revision`. The cold-fallback [`Catalog`] is
    /// **not** built here — it is constructed lazily on the first cold miss via
    /// `Self::catalog` — so session-open holds no open redb lock and does no
    /// graph work. The daemon client targets `root`.
    #[must_use]
    pub fn new(db_path: PathBuf, root: PathBuf, revision: u64) -> Self {
        let daemon = DaemonClient::new(root.clone());
        Self {
            db_path,
            root,
            revision,
            catalog: Arc::new(OnceCell::new()),
            daemon,
            tool_router: Self::tool_router(),
        }
    }

    /// Surface the catalog for in-process tests and benches, forcing the lazy
    /// build if it has not happened yet by routing through the race-free
    /// async `catalog` accessor. Async (every caller already runs inside a
    /// tokio runtime) so concurrent first callers share the single
    /// `OnceCell::get_or_try_init` build instead of each running
    /// `Catalog::build` — the sync variant this replaced let the losers'
    /// build be wasted CPU.
    ///
    /// # Panics
    /// Panics if the on-disk index cannot be opened or the catalog cannot be
    /// built — acceptable in the test/bench callers, which seed a valid index.
    #[must_use]
    pub async fn catalog_arc(&self) -> Arc<Catalog> {
        self.catalog()
            .await
            .expect("build cold catalog for test/bench")
    }

    /// Whether the cold-fallback catalog has been built. Lets tests assert the
    /// lazy contract — unbuilt after `build_server`, built only after a cold
    /// miss — without forcing a build.
    #[must_use]
    pub fn catalog_built(&self) -> bool {
        self.catalog.get().is_some()
    }

    /// Lazily build (once) and return the cold-fallback [`Catalog`]. Called by
    /// every cold-fallback tool arm when the daemon is unreachable.
    ///
    /// [`Catalog::build`] is a synchronous, CPU-bound full-index read, so it
    /// runs inside [`tokio::task::spawn_blocking`] to keep runtime workers
    /// free. [`OnceCell::get_or_try_init`] guarantees concurrent first misses
    /// build it once; the rest await that build.
    ///
    /// # Errors
    /// Surfaces storage-open / catalog-build failures as the same rmcp wire
    /// error the cold path already raises.
    async fn catalog(&self) -> Result<Arc<Catalog>, ErrorData> {
        let db_path = self.db_path.clone();
        let root = self.root.to_string_lossy().into_owned();
        let catalog = self
            .catalog
            .get_or_try_init(|| async move {
                let built = tokio::task::spawn_blocking(move || {
                    let storage = RedbStorage::open(&db_path).map_err(McpError::Storage)?;
                    Catalog::build(&storage, root)
                })
                .await
                .map_err(|e| McpError::Other(format!("catalog build task join: {e}")))??;
                Ok::<Arc<Catalog>, McpError>(Arc::new(built))
            })
            .await
            .map_err(McpError::into_rmcp)?;
        Ok(Arc::clone(catalog))
    }

    /// Open the on-disk index transiently for a cold-fallback read, then let
    /// the caller drop it. Never held past the call, so the redb single-open
    /// lock stays free for the daemon (see [`Self::db_path`]).
    fn open_storage(&self) -> Result<RedbStorage, ErrorData> {
        RedbStorage::open(&self.db_path).map_err(|e| McpError::Storage(e).into_rmcp())
    }

    /// The client's last-observed redb revision, sent in every daemon
    /// handshake so a daemon behind the client refreshes before answering.
    fn revision(&self) -> u64 {
        self.revision
    }

    #[tool(
        description = "List symbols matching an optional substring + kind filter. Use \
when locating a symbol by name or kind before opening files; triggers: \"where is the X \
function\", \"list the structs in\".",
        meta = always_load_meta(),
    )]
    async fn list_symbols(
        &self,
        Parameters(input): Parameters<ListSymbolsInput>,
    ) -> Result<CallToolResult, ErrorData> {
        let query = DaemonQuery::ListSymbols {
            query: input.query.clone(),
            kind: input.kind.clone(),
            limit: input.limit,
        };
        if let Some(resp) = self.daemon.try_query_async(self.revision(), query).await {
            return project_daemon(resp);
        }
        let catalog = self.catalog().await?;
        let out = tools::list_symbols::handle(&catalog, &input);
        wire(&out)
    }

    #[tool(
        description = "Search the codebase for symbols by name pattern — case-insensitive \
substring, or a regular expression when `regex` is true — optionally narrowed by a path \
glob, kind, language, or visibility. Use when finding symbols whose exact name you do not \
know, or scoping a name search to a path / kind, instead of grepping; triggers: \"search \
for functions matching\", \"find symbols named like\", \"grep the codebase for\".",
        meta = always_load_meta(),
    )]
    async fn search_code(
        &self,
        Parameters(input): Parameters<SearchCodeInput>,
    ) -> Result<CallToolResult, ErrorData> {
        // Pure read projection over the in-RAM catalog (D8): no daemon query
        // variant exists for it, so it always answers from the cold catalog,
        // built lazily on first use like every other cold-path arm.
        let catalog = self.catalog().await?;
        let out = tools::search_code::handle(&catalog, &input).map_err(McpError::into_rmcp)?;
        wire(&out)
    }

    #[tool(
        description = "Read a symbol's source straight from disk by name, in `signature`, \
`full` (default), or `context` (±N lines) mode — without reading the whole file. Returns \
the file, 1-based line range, byte range, and the index `revision` (with `stale: true` if \
the file changed since indexing). Use when you need the actual code of a symbol you can \
name, instead of opening the file with Read; triggers: \"show me the source of X\", \"read \
the body of X\", \"what does X look like\".",
        meta = always_load_meta(),
    )]
    async fn read_symbol(
        &self,
        Parameters(input): Parameters<ReadSymbolInput>,
    ) -> Result<CallToolResult, ErrorData> {
        // Reads the live file under the catalog root (D9): no daemon query
        // variant exists, so it always answers from the cold catalog, built
        // lazily on first use like every other cold-path arm.
        let catalog = self.catalog().await?;
        let out = tools::read_symbol::handle(&catalog, &input).map_err(McpError::into_rmcp)?;
        wire(&out)
    }

    #[tool(
        description = "Find the defining symbol record by canonical name. Use when you \
need the canonical definition site of a named symbol; triggers: \"where is X defined\", \
\"go to definition of\".",
        meta = always_load_meta(),
    )]
    async fn find_definition(
        &self,
        Parameters(input): Parameters<SymbolQuery>,
    ) -> Result<CallToolResult, ErrorData> {
        let query = DaemonQuery::FindDefinition {
            symbol: input.symbol.clone(),
        };
        if let Some(resp) = self.daemon.try_query_async(self.revision(), query).await {
            return project_daemon(resp);
        }
        let catalog = self.catalog().await?;
        let out = tools::find_definition::handle(&catalog, &input).map_err(McpError::into_rmcp)?;
        wire(&out)
    }

    #[tool(
        description = "List references to a symbol with source spans. Use when you need \
every use site of a symbol; triggers: \"who calls X\", \"where is X used\", \"find usages \
of\".",
        meta = always_load_meta(),
    )]
    async fn find_references(
        &self,
        Parameters(input): Parameters<SymbolQuery>,
    ) -> Result<CallToolResult, ErrorData> {
        let query = DaemonQuery::FindReferences {
            symbol: input.symbol.clone(),
        };
        if let Some(resp) = self.daemon.try_query_async(self.revision(), query).await {
            return project_daemon(resp);
        }
        let catalog = self.catalog().await?;
        let storage = self.open_storage()?;
        let out = tools::find_references::handle(&catalog, &storage, &input)
            .map_err(McpError::into_rmcp)?;
        wire(&out)
    }

    #[tool(
        description = "Compute the blast radius (must-touch + may-touch) of a symbol. \
Use when assessing what a change to a symbol could break; triggers: \"what breaks if I \
change X\", \"impact of changing\", \"is it safe to edit\".",
        meta = always_load_meta(),
    )]
    async fn blast_radius(
        &self,
        Parameters(input): Parameters<BlastRadiusInput>,
    ) -> Result<CallToolResult, ErrorData> {
        let query = DaemonQuery::BlastRadius {
            symbol: input.symbol.clone(),
            depth: input.depth,
            kinds: to_core_kinds(input.kinds.as_deref()),
        };
        if let Some(resp) = self.daemon.try_query_async(self.revision(), query).await {
            return project_daemon(resp);
        }
        let catalog = self.catalog().await?;
        let out = tools::blast_radius::handle(&catalog, &input).map_err(McpError::into_rmcp)?;
        wire(&out)
    }

    #[tool(
        description = "Summarize a file: symbols, fan-in/out, top dependencies. Use when \
orienting in an unfamiliar file before reading it; triggers: \"what is in this file\", \
\"summarize src/X.rs\".",
        meta = always_load_meta(),
    )]
    async fn file_summary(
        &self,
        Parameters(input): Parameters<FileQuery>,
    ) -> Result<CallToolResult, ErrorData> {
        let query = DaemonQuery::FileSummary {
            path: input.path.clone(),
        };
        if let Some(resp) = self.daemon.try_query_async(self.revision(), query).await {
            return project_daemon(resp);
        }
        let catalog = self.catalog().await?;
        let storage = self.open_storage()?;
        let out =
            tools::file_summary::handle(&catalog, &storage, &input).map_err(McpError::into_rmcp)?;
        wire(&out)
    }

    #[tool(
        description = "Ranked plan-assist file list implicated by a symbol change. Use \
when scoping which files a change touches before editing; triggers: \"what files do I \
touch for X\", \"where do I start to change\".",
        meta = always_load_meta(),
    )]
    async fn plan_assist(
        &self,
        Parameters(input): Parameters<PlanAssistInput>,
    ) -> Result<CallToolResult, ErrorData> {
        let query = DaemonQuery::PlanAssist {
            symbol: input.symbol.clone(),
            max_files: input.max_files,
        };
        if let Some(resp) = self.daemon.try_query_async(self.revision(), query).await {
            return project_daemon(resp);
        }
        let catalog = self.catalog().await?;
        let out = tools::plan_assist::handle(&catalog, &input).map_err(McpError::into_rmcp)?;
        wire(&out)
    }

    #[tool(
        description = "Per-file Ca/Ce/I/A/Distance Martin coupling metrics. Use when \
assessing module dependency or architecture health; triggers: \"how coupled is\", \
\"Martin coupling metrics for\".",
        meta = always_load_meta(),
    )]
    async fn coupling_report(
        &self,
        Parameters(input): Parameters<ScopeInput>,
    ) -> Result<CallToolResult, ErrorData> {
        let query = DaemonQuery::CouplingReport {
            prefix: input.prefix.clone(),
        };
        if let Some(resp) = self.daemon.try_query_async(self.revision(), query).await {
            return project_daemon(resp);
        }
        let catalog = self.catalog().await?;
        let out = tools::coupling_report::handle(&catalog, &input);
        wire(&out)
    }

    #[tool(
        description = "Cycles, god modules, and dead-code candidates with reasons. Use \
when hunting cycles, god modules, or dead code; triggers: \"what is wrong with this \
codebase\", \"find tech debt\", \"any cycles\".",
        meta = always_load_meta(),
    )]
    async fn weak_spots(
        &self,
        Parameters(input): Parameters<ScopeInput>,
    ) -> Result<CallToolResult, ErrorData> {
        let query = DaemonQuery::WeakSpots {
            prefix: input.prefix.clone(),
        };
        if let Some(resp) = self.daemon.try_query_async(self.revision(), query).await {
            return project_daemon(resp);
        }
        let catalog = self.catalog().await?;
        let out = tools::weak_spots::handle(&catalog, &input);
        wire(&out)
    }

    #[tool(
        description = "Doc-like structured summary for one symbol. Use when you need a \
structured explanation of one symbol; triggers: \"what does X do\", \"explain the symbol \
X\".",
        meta = always_load_meta(),
    )]
    async fn doc_for(
        &self,
        Parameters(input): Parameters<SymbolQuery>,
    ) -> Result<CallToolResult, ErrorData> {
        let query = DaemonQuery::DocFor {
            symbol: input.symbol.clone(),
        };
        if let Some(resp) = self.daemon.try_query_async(self.revision(), query).await {
            return project_daemon(resp);
        }
        let catalog = self.catalog().await?;
        let out = tools::doc_for::handle(&catalog, &input).map_err(McpError::into_rmcp)?;
        wire(&out)
    }

    #[tool(
        description = "Project-wide counts, revision, and root. Use when checking index \
freshness or coverage before trusting results; triggers: \"is the index current\", \"how \
big is the project\".",
        meta = always_load_meta(),
    )]
    async fn project_status(&self) -> Result<CallToolResult, ErrorData> {
        if let Some(resp) = self
            .daemon
            .try_query_async(self.revision(), DaemonQuery::ProjectStatus)
            .await
        {
            return project_daemon(resp);
        }
        let catalog = self.catalog().await?;
        let out = tools::project_status::handle(&catalog);
        wire(&out)
    }

    #[tool(
        description = "Markdown documentation summary for one module (file path). Use \
when you need a doc-style summary of a file or module; triggers: \"document this \
module\", \"overview of src/X.rs\".",
        meta = always_load_meta(),
    )]
    async fn doc_for_module(
        &self,
        Parameters(input): Parameters<FileQuery>,
    ) -> Result<CallToolResult, ErrorData> {
        let query = DaemonQuery::DocForModule {
            path: input.path.clone(),
        };
        if let Some(resp) = self.daemon.try_query_async(self.revision(), query).await {
            return project_daemon(resp);
        }
        let catalog = self.catalog().await?;
        let storage = self.open_storage()?;
        let out =
            tools::doc_module::handle(&catalog, &storage, &input).map_err(McpError::into_rmcp)?;
        wire(&out)
    }

    #[tool(
        description = "Markdown architecture overview for the whole project. Use when \
you need a whole-project architecture overview; triggers: \"explain the architecture\", \
\"how is this project structured\".",
        meta = always_load_meta(),
    )]
    async fn doc_for_project(
        &self,
        Parameters(input): Parameters<ScopeInput>,
    ) -> Result<CallToolResult, ErrorData> {
        let query = DaemonQuery::DocForProject {
            prefix: input.prefix.clone(),
        };
        if let Some(resp) = self.daemon.try_query_async(self.revision(), query).await {
            return project_daemon(resp);
        }
        let catalog = self.catalog().await?;
        let storage = self.open_storage()?;
        let out =
            tools::doc_project::handle(&catalog, &storage, &input).map_err(McpError::into_rmcp)?;
        wire(&out)
    }

    #[tool(
        description = "Static refactor suggestions (god modules, cycle breaks, misplaced \
symbols). Use when you want concrete static refactor candidates; triggers: \"how should \
I refactor\", \"cleanup suggestions for\".",
        meta = always_load_meta(),
    )]
    async fn refactor_suggestions(
        &self,
        Parameters(input): Parameters<ScopeInput>,
    ) -> Result<CallToolResult, ErrorData> {
        let query = DaemonQuery::RefactorSuggestions {
            prefix: input.prefix.clone(),
        };
        if let Some(resp) = self.daemon.try_query_async(self.revision(), query).await {
            return project_daemon(resp);
        }
        let catalog = self.catalog().await?;
        let storage = self.open_storage()?;
        let out =
            tools::refactor::handle(&catalog, &storage, &input).map_err(McpError::into_rmcp)?;
        wire(&out)
    }

    #[tool(
        description = "Rank files or symbols by churn × complexity (the Git change-frequency \
× McCabe hotspot product). Use when finding the riskiest code to review or refactor first; \
triggers: \"what are the hotspots\", \"which files change most and are most complex\", \
\"where is the riskiest code\".",
        meta = always_load_meta(),
    )]
    async fn hotspots(
        &self,
        Parameters(input): Parameters<GrainScopeInput>,
    ) -> Result<CallToolResult, ErrorData> {
        let query = DaemonQuery::Hotspots {
            prefix: input.prefix.clone(),
            grain: to_core_grain(input.grain),
        };
        if let Some(resp) = self.daemon.try_query_async(self.revision(), query).await {
            return project_daemon(resp);
        }
        let catalog = self.catalog().await?;
        let out = tools::hotspots::handle(&catalog, &input);
        wire(&out)
    }

    #[tool(
        description = "Rank files (Σ) or symbols by McCabe cyclomatic complexity, descending. \
Use when finding the most complex code to simplify or add tests to; triggers: \"what is the \
most complex code\", \"cyclomatic complexity of\", \"which functions are hardest to follow\".",
        meta = always_load_meta(),
    )]
    async fn complexity(
        &self,
        Parameters(input): Parameters<GrainScopeInput>,
    ) -> Result<CallToolResult, ErrorData> {
        let query = DaemonQuery::Complexity {
            prefix: input.prefix.clone(),
            grain: to_core_grain(input.grain),
        };
        if let Some(resp) = self.daemon.try_query_async(self.revision(), query).await {
            return project_daemon(resp);
        }
        let catalog = self.catalog().await?;
        let out = tools::complexity::handle(&catalog, &input);
        wire(&out)
    }

    #[tool(
        description = "List file pairs that change together in Git history (logical coupling) \
above the configured thresholds. Use when finding hidden dependencies that move together \
despite no static edge; triggers: \"what changes together with\", \"co-change coupling\", \
\"which files are logically coupled\".",
        meta = always_load_meta(),
    )]
    async fn co_change(
        &self,
        Parameters(input): Parameters<CoChangeInput>,
    ) -> Result<CallToolResult, ErrorData> {
        let query = DaemonQuery::CoChange {
            prefix: input.prefix.clone(),
            min_revs: input.min_revs,
            min_shared_commits: input.min_shared_commits,
            min_degree: input.min_degree,
        };
        if let Some(resp) = self.daemon.try_query_async(self.revision(), query).await {
            return project_daemon(resp);
        }
        let catalog = self.catalog().await?;
        let out = tools::co_change::handle(&catalog, &input);
        wire(&out)
    }

    #[tool(
        description = "Compute the blast radius of a code change: the must-touch ∪ may-touch \
impact of every symbol a diff touches (uncommitted working-tree changes by default, or a \
commit / ref range). Use when scoping what your current changes affect before committing or \
in review; triggers: \"what does my diff affect\", \"blast radius of my changes\", \"impact \
of this commit\", \"what does this PR touch\".",
        meta = always_load_meta(),
    )]
    async fn diff_blast_radius(
        &self,
        Parameters(input): Parameters<DiffBlastInput>,
    ) -> Result<CallToolResult, ErrorData> {
        // Run the git diff first — both the daemon and cold paths need its hunks
        // + changed paths, and it must run here in the MCP process where
        // `ariadne-git` is linked; the daemon never links git (RD7 / ADR-0023),
        // so only the pre-computed hunks travel over the wire.
        let spec = to_core_spec(&input.spec);
        let (hunks, changed_paths) = ariadne_git::diff(&self.root, &spec)
            .map_err(|e| McpError::Other(format!("git diff failed: {e}")).into_rmcp())?;

        let query = DaemonQuery::DiffBlast {
            hunks: hunks.clone(),
            changed_paths: changed_paths.clone(),
            depth: input.depth,
            kinds: to_core_kinds(input.kinds.as_deref()),
        };
        if let Some(resp) = self.daemon.try_query_async(self.revision(), query).await {
            return project_daemon(resp);
        }
        let catalog = self.catalog().await?;
        let storage = self.open_storage()?;
        let out = tools::diff_blast::handle(
            &catalog,
            &storage,
            &self.root,
            &hunks,
            &changed_paths,
            input.depth,
            input.kinds.as_deref(),
        )
        .map_err(McpError::into_rmcp)?;
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
                "For any question about symbols, references, impact, or architecture, \
search for and call these Ariadne tools instead of grep, Read, or file-walking — the \
graph answers in one call where text search needs many and misses cross-file edges. \
Ariadne is a read-only semantic graph of the local project (symbols, references, and \
dependency edges), kept current with the code. Navigate with list_symbols, \
find_definition, find_references, and file_summary. Scope impact with blast_radius, \
plan_assist, and diff_blast_radius. Assess architecture health with coupling_report, \
weak_spots, and refactor_suggestions. Triage risk from Git history with hotspots, \
complexity, and co_change. Read generated docs with doc_for, doc_for_module, and \
doc_for_project. Verify index freshness with project_status before trusting results. \
Call these even when the answer seems known — the graph reflects the current code, and \
assumptions may be stale.",
            )
    }
}

/// Project a daemon [`DaemonResponse`] into the v1 tool output wire shape.
/// Each report payload mirrors the matching MCP output type field-for-field
/// (tier-07), so serializing it yields the byte-identical JSON the cold path
/// produces. A query-level [`DaemonResponse::Error`] becomes the same wire
/// error the cold path raises for a missing symbol / path.
fn project_daemon(resp: DaemonResponse) -> Result<CallToolResult, ErrorData> {
    match resp {
        DaemonResponse::Symbols(rows) => wire(&rows),
        DaemonResponse::Definition(sym) => wire(&sym),
        DaemonResponse::References(rows) => wire(&rows),
        DaemonResponse::BlastRadius(report) => wire(&report),
        DaemonResponse::FileSummary(report) => wire(&report),
        DaemonResponse::PlanAssist(report) => wire(&report),
        DaemonResponse::Coupling(report) => wire(&report),
        DaemonResponse::WeakSpots(report) => wire(&report),
        DaemonResponse::DocFor(report) => wire(&report),
        DaemonResponse::Doc(report) => wire(&report),
        DaemonResponse::ProjectStatus(report) => wire(&report),
        DaemonResponse::Refactor(report) => wire(&report),
        DaemonResponse::Hotspots(report) => wire(&report),
        DaemonResponse::Complexity(report) => wire(&report),
        DaemonResponse::CoChange(report) => wire(&report),
        DaemonResponse::DiffBlast(report) => wire(&report),
        DaemonResponse::Error(msg) => Err(ErrorData::internal_error(msg, None)),
        DaemonResponse::Pong => Err(ErrorData::internal_error(
            "daemon answered Pong to a tool query",
            None,
        )),
    }
}

/// Map the MCP-facing diff spec onto the `ariadne-core` `DiffSpec` the git
/// adapter resolves. The core type stays a wire/domain type free of `schemars`
/// (tier-15c D4).
fn to_core_spec(spec: &DiffSpecInput) -> ariadne_core::DiffSpec {
    match spec {
        DiffSpecInput::WorkingTree => ariadne_core::DiffSpec::WorkingTree,
        DiffSpecInput::Commit(rev) => ariadne_core::DiffSpec::Commit(rev.clone()),
        DiffSpecInput::RefRange { from, to } => ariadne_core::DiffSpec::RefRange {
            from: from.clone(),
            to: to.clone(),
        },
    }
}

/// Map the MCP-facing grain onto the daemon protocol's grain.
fn to_core_grain(grain: Grain) -> CoreGrain {
    match grain {
        Grain::File => CoreGrain::File,
        Grain::Symbol => CoreGrain::Symbol,
    }
}

/// Map the MCP-facing edge-kind filter onto the daemon protocol's filter.
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

/// The `_meta` marker exempting each Ariadne tool from MCP Tool Search
/// deferral. Claude Code reads `anthropic/alwaysLoad` off each tool's `_meta`
/// in the `tools/list` response and keeps that tool always-loaded — so its
/// trigger-phrase description reaches the agent every session even when the
/// consumer's `.mcp.json` carries no server-level `alwaysLoad` flag (plan D2,
/// belt-and-suspenders with the `setup`-written server flag). Each `#[tool]`
/// attaches it via `meta = always_load_meta()`, which rmcp expands to
/// `Tool::with_meta(..)` [src: <https://code.claude.com/docs/en/mcp> "mark
/// individual tools as always-loaded … anthropic/alwaysLoad"; rmcp 1.7.0
/// `Tool::with_meta`].
fn always_load_meta() -> rmcp::model::Meta {
    let mut map = serde_json::Map::new();
    map.insert(
        "anthropic/alwaysLoad".to_owned(),
        serde_json::Value::Bool(true),
    );
    rmcp::model::Meta(map)
}

fn wire<T: serde::Serialize>(value: &T) -> Result<CallToolResult, ErrorData> {
    let json =
        serde_json::to_string(value).map_err(|e| ErrorData::internal_error(e.to_string(), None))?;
    Ok(CallToolResult::success(vec![Content::text(json)]))
}

#[cfg(test)]
mod tests {
    //! Daemon/cold projection parity (audit INFO-2, INFO-3).
    //!
    //! `project_daemon` claims the daemon report types (`ariadne_core::*Report`)
    //! serialize byte-identically to the cold MCP output types
    //! (`crate::types::*Output`) — "correct by construction" because the row
    //! types are mirrored field-for-field. These tests drive `project_daemon`
    //! for every `DaemonResponse` arm and assert the projected JSON equals the
    //! cold output's JSON for equivalent data, so any field rename, reorder, or
    //! serde-attr drift between the two type families fails loudly.

    use super::*;

    /// Daemon-side symbol row.
    fn c_sym(id: u64, name: &str) -> ariadne_core::SymbolSummary {
        ariadne_core::SymbolSummary {
            id,
            name: name.into(),
            kind: "function".into(),
            file: "src/x.rs".into(),
            byte_start: 1,
            byte_end: 9,
        }
    }

    /// Cold-side symbol row carrying field-identical data.
    fn t_sym(id: u64, name: &str) -> crate::types::SymbolSummary {
        crate::types::SymbolSummary {
            id,
            name: name.into(),
            kind: "function".into(),
            file: "src/x.rs".into(),
            byte_start: 1,
            byte_end: 9,
        }
    }

    /// Extract the single JSON text block `project_daemon` wraps a report in.
    fn projected_text(resp: DaemonResponse) -> String {
        let result = project_daemon(resp).expect("variant must project to Ok");
        match &result.content.first().expect("one content block").raw {
            rmcp::model::RawContent::Text(t) => t.text.clone(),
            other => panic!("expected text content, got {other:?}"),
        }
    }

    /// Assert the daemon-path projection of `resp` equals the cold-path JSON
    /// of `cold` byte-for-byte.
    fn assert_parity<T: serde::Serialize>(label: &str, resp: DaemonResponse, cold: &T) {
        let got = projected_text(resp);
        let want = serde_json::to_string(cold).expect("serialize cold output");
        assert_eq!(got, want, "daemon/cold JSON parity for {label}");
    }

    #[test]
    fn symbols_arm_matches_cold_list_symbols() {
        let cold: Vec<crate::types::SymbolSummary> = vec![t_sym(1, "a"), t_sym(2, "b")];
        assert_parity(
            "list_symbols",
            DaemonResponse::Symbols(vec![c_sym(1, "a"), c_sym(2, "b")]),
            &cold,
        );
    }

    #[test]
    fn definition_arm_matches_cold_find_definition() {
        assert_parity(
            "find_definition",
            DaemonResponse::Definition(c_sym(7, "crate::f")),
            &t_sym(7, "crate::f"),
        );
    }

    #[test]
    fn references_arm_matches_cold_find_references() {
        let c = ariadne_core::ReferenceSite {
            caller: 3,
            caller_name: "crate::caller".into(),
            file: "src/y.rs".into(),
            byte_start: 4,
            byte_end: 12,
        };
        let t = crate::types::ReferenceSite {
            caller: 3,
            caller_name: "crate::caller".into(),
            file: "src/y.rs".into(),
            byte_start: 4,
            byte_end: 12,
        };
        let cold: Vec<crate::types::ReferenceSite> = vec![t];
        assert_parity(
            "find_references",
            DaemonResponse::References(vec![c]),
            &cold,
        );
    }

    #[test]
    fn blast_radius_arm_matches_cold_output() {
        let c = ariadne_core::BlastRadiusReport {
            symbol: c_sym(1, "t"),
            must_touch: vec![c_sym(2, "m")],
            may_touch: vec![c_sym(3, "y")],
            depth_used: 2,
        };
        let t = crate::types::BlastRadiusOutput {
            symbol: t_sym(1, "t"),
            must_touch: vec![t_sym(2, "m")],
            may_touch: vec![t_sym(3, "y")],
            depth_used: 2,
        };
        assert_parity("blast_radius", DaemonResponse::BlastRadius(c), &t);
    }

    #[test]
    fn file_summary_arm_matches_cold_output() {
        let c = ariadne_core::FileSummaryReport {
            path: "src/f.rs".into(),
            symbols: vec![c_sym(1, "a")],
            fan_in: 3,
            fan_out: 4,
            top_dependencies: vec![ariadne_core::DependencyRow {
                file: "src/dep.rs".into(),
                edges: 5,
            }],
            components: vec![ariadne_core::ComponentRow {
                component: "App".into(),
                renders: vec!["Card".into()],
                hooks: vec!["useX".into()],
            }],
        };
        let t = crate::types::FileSummaryOutput {
            path: "src/f.rs".into(),
            symbols: vec![t_sym(1, "a")],
            fan_in: 3,
            fan_out: 4,
            top_dependencies: vec![crate::types::DependencyRow {
                file: "src/dep.rs".into(),
                edges: 5,
            }],
            components: vec![crate::types::ComponentRow {
                component: "App".into(),
                renders: vec!["Card".into()],
                hooks: vec!["useX".into()],
            }],
        };
        assert_parity("file_summary", DaemonResponse::FileSummary(c), &t);
    }

    #[test]
    fn plan_assist_arm_matches_cold_output() {
        let c = ariadne_core::PlanAssistReport {
            files: vec![ariadne_core::PlanFileRow {
                file: "src/f.rs".into(),
                why: vec!["reason".into()],
                certainty: 0.5,
            }],
        };
        let t = crate::types::PlanAssistOutput {
            files: vec![crate::types::PlanFileRow {
                file: "src/f.rs".into(),
                why: vec!["reason".into()],
                certainty: 0.5,
            }],
        };
        assert_parity("plan_assist", DaemonResponse::PlanAssist(c), &t);
    }

    #[test]
    fn coupling_arm_matches_cold_output() {
        let c = ariadne_core::CouplingReport {
            rows: vec![ariadne_core::CouplingRow {
                module: "src/m.rs".into(),
                afferent: 2,
                efferent: 3,
                instability: 0.5,
                abstractness: 0.0,
                distance: 0.25,
            }],
        };
        let t = crate::types::CouplingOutput {
            rows: vec![crate::types::CouplingRow {
                module: "src/m.rs".into(),
                afferent: 2,
                efferent: 3,
                instability: 0.5,
                abstractness: 0.0,
                distance: 0.25,
            }],
        };
        assert_parity("coupling_report", DaemonResponse::Coupling(c), &t);
    }

    #[test]
    fn weak_spots_arm_matches_cold_output() {
        let c = ariadne_core::WeakSpotsReport {
            cycles: vec![ariadne_core::CycleRow {
                members: vec!["a".into(), "b".into()],
            }],
            god_modules: vec![ariadne_core::CouplingRow {
                module: "src/g.rs".into(),
                afferent: 0,
                efferent: 9,
                instability: 1.0,
                abstractness: 0.0,
                distance: 0.0,
            }],
            dead_symbols: vec![c_sym(9, "dead")],
        };
        let t = crate::types::WeakSpotsOutput {
            cycles: vec![crate::types::CycleRow {
                members: vec!["a".into(), "b".into()],
            }],
            god_modules: vec![crate::types::CouplingRow {
                module: "src/g.rs".into(),
                afferent: 0,
                efferent: 9,
                instability: 1.0,
                abstractness: 0.0,
                distance: 0.0,
            }],
            dead_symbols: vec![t_sym(9, "dead")],
        };
        assert_parity("weak_spots", DaemonResponse::WeakSpots(c), &t);
    }

    #[test]
    fn doc_for_arm_matches_cold_output() {
        let c = ariadne_core::DocForReport {
            signature: "fn f()".into(),
            kind: "function".into(),
            file: "src/f.rs".into(),
            brief: "brief".into(),
            public_refs: vec![c_sym(1, "r")],
        };
        let t = crate::types::DocForOutput {
            signature: "fn f()".into(),
            kind: "function".into(),
            file: "src/f.rs".into(),
            brief: "brief".into(),
            public_refs: vec![t_sym(1, "r")],
        };
        assert_parity("doc_for", DaemonResponse::DocFor(c), &t);
    }

    #[test]
    fn doc_arm_matches_cold_output() {
        assert_parity(
            "doc_for_module/doc_for_project",
            DaemonResponse::Doc(ariadne_core::DocReport {
                markdown: "# Doc".into(),
            }),
            &crate::types::DocOutput {
                markdown: "# Doc".into(),
            },
        );
    }

    #[test]
    fn project_status_arm_matches_cold_output() {
        let c = ariadne_core::ProjectStatusReport {
            revision: 11,
            file_count: 4,
            symbol_count: 7,
            edge_count: 6,
            root: "/p".into(),
        };
        let t = crate::types::ProjectStatusOutput {
            revision: 11,
            file_count: 4,
            symbol_count: 7,
            edge_count: 6,
            root: "/p".into(),
        };
        assert_parity("project_status", DaemonResponse::ProjectStatus(c), &t);
    }

    #[test]
    fn refactor_arm_matches_cold_output() {
        let c = ariadne_core::RefactorReport {
            god_modules: vec![ariadne_core::GodModuleRow {
                module: "src/m.rs".into(),
                efferent: 9,
                cohesion: 0.25,
                top_outbound: vec![ariadne_core::OutboundRow {
                    symbol: "s".into(),
                    edges: 3,
                }],
                suggestion: "split".into(),
            }],
            cycle_breaks: vec![ariadne_core::CycleBreakRow {
                from: "a".into(),
                to: "b".into(),
                score: 0.5,
                rationale: "rationale".into(),
            }],
            misplaced_symbols: vec![ariadne_core::MisplacedRow {
                symbol: "s".into(),
                current_module: "src/a.rs".into(),
                target_module: "src/b.rs".into(),
                ratio: 0.75,
            }],
        };
        let t = crate::types::RefactorOutput {
            god_modules: vec![crate::types::GodModuleRow {
                module: "src/m.rs".into(),
                efferent: 9,
                cohesion: 0.25,
                top_outbound: vec![crate::types::OutboundRow {
                    symbol: "s".into(),
                    edges: 3,
                }],
                suggestion: "split".into(),
            }],
            cycle_breaks: vec![crate::types::CycleBreakRow {
                from: "a".into(),
                to: "b".into(),
                score: 0.5,
                rationale: "rationale".into(),
            }],
            misplaced_symbols: vec![crate::types::MisplacedRow {
                symbol: "s".into(),
                current_module: "src/a.rs".into(),
                target_module: "src/b.rs".into(),
                ratio: 0.75,
            }],
        };
        assert_parity("refactor_suggestions", DaemonResponse::Refactor(c), &t);
    }

    #[test]
    fn hotspots_arm_matches_cold_output() {
        // File-grain and symbol-grain rows in one report exercise both the
        // `file`-set and `symbol`-set projections.
        let c = ariadne_core::HotspotReport {
            rows: vec![
                ariadne_core::HotspotRow {
                    file: "src/a.rs".into(),
                    symbol: None,
                    churn: 9,
                    complexity: 7,
                    score: 1.0,
                },
                ariadne_core::HotspotRow {
                    file: String::new(),
                    symbol: Some(c_sym(1, "crate::f")),
                    churn: 5,
                    complexity: 7,
                    score: 0.5,
                },
            ],
        };
        let t = crate::types::HotspotOutput {
            rows: vec![
                crate::types::HotspotRow {
                    file: "src/a.rs".into(),
                    symbol: None,
                    churn: 9,
                    complexity: 7,
                    score: 1.0,
                },
                crate::types::HotspotRow {
                    file: String::new(),
                    symbol: Some(t_sym(1, "crate::f")),
                    churn: 5,
                    complexity: 7,
                    score: 0.5,
                },
            ],
        };
        assert_parity("hotspots", DaemonResponse::Hotspots(c), &t);
    }

    #[test]
    fn complexity_arm_matches_cold_output() {
        let c = ariadne_core::ComplexityReport {
            rows: vec![
                ariadne_core::ComplexityRow {
                    file: "src/a.rs".into(),
                    symbol: None,
                    complexity: 12,
                },
                ariadne_core::ComplexityRow {
                    file: String::new(),
                    symbol: Some(c_sym(2, "crate::g")),
                    complexity: 4,
                },
            ],
        };
        let t = crate::types::ComplexityOutput {
            rows: vec![
                crate::types::ComplexityRow {
                    file: "src/a.rs".into(),
                    symbol: None,
                    complexity: 12,
                },
                crate::types::ComplexityRow {
                    file: String::new(),
                    symbol: Some(t_sym(2, "crate::g")),
                    complexity: 4,
                },
            ],
        };
        assert_parity("complexity", DaemonResponse::Complexity(c), &t);
    }

    #[test]
    fn co_change_arm_matches_cold_output() {
        let c = ariadne_core::CoChangeReport {
            edges: vec![ariadne_core::CoChangeEdge {
                a: "src/a.rs".into(),
                b: "src/b.rs".into(),
                shared_commits: 3,
                degree: 0.461_538_46,
            }],
        };
        let t = crate::types::CoChangeOutput {
            edges: vec![crate::types::CoChangeEdge {
                a: "src/a.rs".into(),
                b: "src/b.rs".into(),
                shared_commits: 3,
                degree: 0.461_538_46,
            }],
        };
        assert_parity("co_change", DaemonResponse::CoChange(c), &t);
    }

    #[test]
    fn diff_blast_arm_matches_cold_output() {
        // A seed (`t`) plus an unresolved changed path exercises every field of
        // the mirrored report families.
        let c = ariadne_core::DiffBlastReport {
            seeds: vec![ariadne_core::DiffSeed {
                symbol: c_sym(1, "t"),
                must_touch: vec![c_sym(2, "m")],
                may_touch: vec![c_sym(3, "y")],
                depth_used: 2,
            }],
            must_touch: vec![c_sym(2, "m")],
            may_touch: vec![c_sym(3, "y")],
            unresolved: vec!["src/new.rs".into()],
        };
        let t = crate::types::DiffBlastOutput {
            seeds: vec![crate::types::DiffSeedRow {
                symbol: t_sym(1, "t"),
                must_touch: vec![t_sym(2, "m")],
                may_touch: vec![t_sym(3, "y")],
                depth_used: 2,
            }],
            must_touch: vec![t_sym(2, "m")],
            may_touch: vec![t_sym(3, "y")],
            unresolved: vec!["src/new.rs".into()],
        };
        assert_parity("diff_blast_radius", DaemonResponse::DiffBlast(c), &t);
    }

    #[test]
    fn error_arm_shares_the_cold_not_found_contract() {
        // The daemon phrases not-found as "symbol X not found"; the cold path's
        // `McpError::NotFound` renders "not found: symbol X". The wording
        // differs, but both map to the same JSON-RPC code and both carry the
        // "not found" substring the `find_definition` error test asserts — so
        // the daemon Error arm satisfies the cold path's error contract
        // (audit INFO-3).
        let daemon =
            project_daemon(DaemonResponse::Error("symbol crate::x not found".into())).unwrap_err();
        let cold = crate::errors::McpError::NotFound("symbol crate::x".into()).into_rmcp();
        assert_eq!(daemon.code, cold.code, "same JSON-RPC error code");
        assert_eq!(daemon.code, rmcp::model::ErrorCode::INTERNAL_ERROR);
        assert!(
            daemon.message.contains("not found"),
            "daemon message: {}",
            daemon.message
        );
        assert!(
            cold.message.contains("not found"),
            "cold message: {}",
            cold.message
        );
    }

    #[test]
    fn pong_arm_is_a_protocol_error_not_a_tool_result() {
        // `Pong` answers a liveness probe; receiving it for a tool query is a
        // protocol fault, surfaced as an internal error rather than a result.
        let err = project_daemon(DaemonResponse::Pong).unwrap_err();
        assert_eq!(err.code, rmcp::model::ErrorCode::INTERNAL_ERROR);
        assert!(err.message.contains("Pong"), "got {}", err.message);
    }
}
