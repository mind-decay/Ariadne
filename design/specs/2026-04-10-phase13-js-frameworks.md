# Phase 13: Deep Language Support — TypeScript/JavaScript Frameworks — Specification

## Scope Split (D-149)

Phase 13 is split into two sub-phases:
- **Phase 13a** (this spec): Bundler resolution + React/Next.js + `raw_parse` infrastructure
- **Phase 13b** (separate spec): SFC formats (Vue, Svelte, Astro) + Angular + Remix

## Phase 13a Goal

Bundler-aware alias resolution (Vite, Webpack, Turbopack) and framework-aware dependency extraction for React/Next.js — the highest-share segment of the JS/TS ecosystem. Also delivers the `raw_parse` LanguageParser trait extension (D-145) that Phase 13b depends on, and migrates .razor to use it (fixing a latent pipeline bug).

## Dependencies

| Phase | Status | What It Provides |
|-------|--------|-----------------|
| Phase 1a (MVP) | DONE | TypeScriptParser + TypeScriptResolver with relative path + extension probing |
| Phase 10 (Config-Aware Resolution) | DONE | tsconfig.json parsing with paths/baseUrl/extends, ProjectConfig, ConfigDiscovery, construction-time injection pattern (D-118) |
| Phase 8 (Semantic Boundaries) | DONE | BoundaryExtractor trait, SemanticState, boundary persistence |

## Risk Classification

**Overall: YELLOW** — All deliverables use existing tree-sitter-typescript grammar. Bundler idiom recognition (D-147) introduces AST pattern matching complexity but is well-scoped. `raw_parse` trait extension (D-145) is backward compatible. No new tree-sitter grammars or external dependencies required.

### Per-Deliverable Risk

| # | Deliverable | Risk | Rationale |
|---|-------------|------|-----------|
| D1 | `raw_parse` trait extension + pipeline update | GREEN | Minimal trait addition, backward compatible, one pipeline change |
| D2 | .razor migration to `raw_parse` | GREEN | Moves existing logic, fixes latent bug, no new behavior |
| D3 | Bundler config idiom recognition (Vite) | YELLOW | AST pattern matching for 3 idioms (literal, fileURLToPath, array form), tree-sitter based |
| D4 | Bundler config idiom recognition (Webpack) | YELLOW | AST pattern matching for 3 idioms (path.resolve, path.join, concatenation), tree-sitter based |
| D5 | Turbopack pipeline parsing | GREEN | Standard JSON parsing, same as tsconfig |
| D6 | Next.js filesystem routing | GREEN | Directory scanning + convention matching, no AST needed |
| D7 | React framework detection + context boundaries | YELLOW | tree-sitter AST analysis for JSX, hooks, createContext/useContext |
| D8 | Next.js framework patterns (directives, routes) | GREEN | String literal detection + filesystem convention |
| D9 | TypeScriptResolver bundler integration | GREEN | Follows D-118 construction-time injection, extends existing resolver |
| D10 | Config discovery extension | GREEN | Follows existing discover_config pattern exactly |

## Design Sources

| Decision | Description | Source |
|----------|-------------|--------|
| D-145 | `raw_parse` override for container formats | Phase 13 architectural audit |
| D-146 | Container format exception to D-001 | Phase 13 architectural audit |
| D-147 | Bundler config idiom recognition via AST | Phase 13 architectural audit |
| D-148 | Warning codes W044-W048 | Sequential allocation pattern |
| D-149 | Phase 13 scope split (13a/13b) | Phase 13 scope analysis |
| D-150 | Bundler alias resolution priority (tsconfig > bundler > relative) | Phase 13 architectural audit |
| D-151 | React context boundaries as EventChannel | Phase 13 architectural audit |
| D-152 | Next.js directive and route boundary mapping | Phase 13 architectural audit |
| D-118 | Construction-time config injection | Phase 10 |
| D-129 | Framework detection split (detect/ + semantic/) | Phase 11 |
| D-130 | .razor directive extraction via regex | Phase 11 |

## Deliverables

### D1: `raw_parse` Trait Extension (`src/parser/traits.rs` + `src/parser/registry.rs`)

Extend `LanguageParser` with optional `raw_parse` method (D-145):

```rust
fn raw_parse(&self, source: &[u8], extension: &str, path: &CanonicalPath)
    -> Option<ParseOutcome> { None }
```

Update `parse_source()` in `ParserRegistry`:
- Check `parser.raw_parse()` first
- If `Some(outcome)`, return it directly — bypass tree-sitter top-level parse, error rate check, AND separate symbol/boundary extraction (since `ParseOutcome` already contains all four components: imports, exports, symbols, boundaries)
- If `None`, proceed with existing tree-sitter pipeline (unchanged)

Update `reparse_imports()` similarly — check raw_parse first. If `raw_parse` returns `Some`, extract imports from the `ParseOutcome`.

Import `ParseOutcome` into traits.rs or re-export from registry. The `ParseOutcome` type may need to be moved to a shared location if it creates a circular dependency.

### D2: .razor Migration to `raw_parse` (`src/parser/csharp.rs`)

Migrate CSharpParser's .razor handling from the current "detect inside extract_imports" approach to `raw_parse`:

- Implement `raw_parse()` on CSharpParser
- For `.razor` extension: call existing `extract_razor_imports()`, return `ParseOutcome::Ok`
- For `.cs` extension: return `None` (use standard tree-sitter pipeline)
- Remove `is_razor_content()` detection from `extract_imports()`

This fixes the latent bug where .razor files with >50% ERROR nodes (when parsed as C#) are silently skipped.

### D3: Vite Config Parsing (`src/parser/config/bundler.rs`)

Parse `vite.config.ts` / `vite.config.js` / `vite.config.mjs` using tree-sitter-typescript AST pattern matching (D-147).

`BundlerConfig` struct:
```rust
pub struct BundlerConfig {
    pub config_dir: PathBuf,
    pub aliases: BTreeMap<String, String>, // alias prefix -> target path (relative to config_dir)
}
```

AST pattern recognition for `resolve.alias` values:
1. Navigate AST: find `defineConfig(...)` call → object argument → `resolve` property → `alias` property
2. Also handle: direct `export default { resolve: { alias: ... } }` without defineConfig wrapper
3. For each alias entry, recognize value patterns:
   - String literal `'./src'` → resolve relative to config_dir
   - `fileURLToPath(new URL('./src', import.meta.url))` → resolve relative to config_dir
   - Array form `[{ find: '@', replacement: './src' }]` → extract find/replacement string literals
   - Unrecognized pattern → emit W045 with source text

Discovery: scan `known_files` for files named `vite.config.{ts,js,mjs}`.

### D4: Webpack Config Parsing (`src/parser/config/bundler.rs`)

Parse `webpack.config.js` / `webpack.config.ts` using tree-sitter AST pattern matching (D-147).

AST pattern recognition for `resolve.alias` values:
1. Navigate AST: find `module.exports = { ... }` → `resolve` property → `alias` property
2. Also handle: `export default { resolve: { alias: ... } }` (ESM webpack config)
3. For each alias entry, recognize value patterns:
   - String literal `'./src'` → resolve relative to config_dir
   - `path.resolve(__dirname, 'src')` → join(config_dir, literal argument)
   - `path.join(__dirname, 'src')` → join(config_dir, literal argument)
   - `__dirname + '/src'` binary expression → join(config_dir, literal)
   - Unrecognized pattern → emit W045 with source text

Also extract `resolve.modules` if present (array of string literals) — these extend the module search path.

Discovery: scan `known_files` for files named `webpack.config.{js,ts}`.

### D5: Turbopack Pipeline Parsing (`src/parser/config/turbo.rs`)

Parse `turbo.json` using standard JSON parsing (same pattern as tsconfig).

`TurboConfig` struct:
```rust
pub struct TurboConfig {
    pub config_dir: PathBuf,
    pub pipeline: BTreeMap<String, TurboPipelineEntry>,
}

pub struct TurboPipelineEntry {
    pub depends_on: Vec<String>,
    pub outputs: Vec<String>,
}
```

Discovery: scan `known_files` for files named `turbo.json`.
Warning: W047 for parse failures.

**Consumer:** `TurboConfig` is stored in `ProjectConfig` for downstream access by MCP tools and views (e.g., displaying workspace task dependencies). Phase 13a does not produce graph edges from turbo pipeline data — it is informational data for consumers, not an import resolution input.

### D6: Next.js Filesystem Routing (`src/parser/config/nextjs.rs`)

Discover Next.js route structure from filesystem conventions. No config file parsing needed — Next.js uses tsconfig paths for aliases (already covered by Phase 10).

`NextRouteInfo` struct:
```rust
pub struct NextRouteInfo {
    pub routes: Vec<NextRoute>,
    pub router_type: NextRouterType, // AppRouter | PagesRouter
}

pub struct NextRoute {
    pub path: String,            // e.g., "/dashboard"
    pub file: CanonicalPath,     // e.g., "app/dashboard/page.tsx"
    pub kind: NextRouteKind,     // Page | Layout | Loading | Error | ApiRoute | Middleware
}

pub enum NextRouterType { AppRouter, PagesRouter, Both }
pub enum NextRouteKind { Page, Layout, Loading, Error, Template, NotFound, ApiRoute, Middleware }
```

Discovery logic:
- If `app/` directory exists with `page.tsx`/`page.jsx`/`page.ts`/`page.js` files → App Router
- If `pages/` directory exists with `.tsx`/`.jsx`/`.ts`/`.js` files → Pages Router
- Convention files: `layout`, `loading`, `error`, `template`, `not-found` in app router
- API routes: `app/api/**/route.{ts,js}` or `pages/api/**/*.{ts,js}`
- Middleware: `middleware.{ts,js}` at project root

Warning: W048 for discovery failures.

**Consumer:** `NextRouteInfo` is stored in `ProjectConfig` for downstream access by MCP tools and views (e.g., route listing, route-to-file mapping). `NextBoundaryExtractor` (D8) does NOT depend on `NextRouteInfo` — it derives route paths independently from the file's `CanonicalPath` via convention matching (D-152). This keeps the boundary extractor self-contained and consistent with the `BoundaryExtractor` trait interface.

### D7: React Framework Detection + Context Boundaries (`src/detect/js_framework.rs` + `src/semantic/react.rs`)

**File-level detection** (D-129 pattern, in `src/detect/js_framework.rs`):

```rust
pub struct JsFrameworkHints {
    pub react_component: bool,         // function returning JSX
    pub react_hook: bool,              // function named use*()
    pub context_provider: Vec<String>, // context names in <X.Provider>
    pub context_consumer: Vec<String>, // context names in useContext(X)
    pub server_component: bool,        // no "use client" in App Router context
    pub client_component: bool,        // "use client" directive
    pub route_convention: Option<RouteConvention>,
}

pub enum RouteConvention {
    NextPage,       // app/**/page.tsx or pages/**/*.tsx
    NextLayout,     // app/**/layout.tsx
    NextApiRoute,   // app/api/**/route.ts or pages/api/**/*.ts
    NextMiddleware, // middleware.ts
    NextLoading,    // app/**/loading.tsx
    NextError,      // app/**/error.tsx
    // Phase 13b will add: RemixLoader, SvelteKitPage, etc.
}
```

**Purpose:** `JsFrameworkHints` is a detection utility consumed by boundary extractors (`ReactBoundaryExtractor`, `NextBoundaryExtractor`) and available for future MCP tools / view generation. Detection flags (`react_component`, `react_hook`, `server_component`, `client_component`) are NOT stored in graph output — they follow the same pattern as `DotnetFrameworkHints` and `JavaFrameworkHints`, which exist as classification utilities.

**ROADMAP coverage rationale:**
- "component tree extraction" — React component dependencies are already captured by standard import edges (File A imports Component from File B → edge A→B). The "component tree" (which components render which via JSX) is runtime render behavior, not a static structural dependency. No additional extraction needed beyond existing import analysis.
- "hook dependency tracking" — Custom hooks import other hooks via standard `import` statements, already captured as dependency edges. The `react_hook` flag provides classification metadata.
- "context provider/consumer graph" — NEW information not available from imports. Delivered via `ReactBoundaryExtractor` mapping `createContext`/`useContext` to `EventChannel` boundaries (D-151).

`detect_js_framework()` function: takes tree-sitter AST + source + file `CanonicalPath`, returns `JsFrameworkHints`. File path is needed for `server_component` detection (requires knowing if file is under `app/` directory) and `route_convention` classification. Detection via AST node inspection:
- JSX elements → `react_component`
- Function name starts with `use` + hook call patterns → `react_hook`
- `createContext()` call → extract variable name → `context_provider`
- `useContext(X)` call → extract argument name → `context_consumer`
- `"use client"` string literal as first statement → `client_component`
- `"use server"` string literal as first statement → explicitly marks server actions
- `server_component` = file is under `app/` directory AND does NOT have `"use client"` directive (Next.js App Router defaults to server components). Requires file path context.
- Filepath convention matching → `route_convention`

**Boundary extraction** (D-129 pattern, D-151, in `src/semantic/react.rs`):

`ReactBoundaryExtractor` implementing `BoundaryExtractor`:
- `createContext()` → `Boundary { kind: EventChannel, name: "Context:<variable_name>", role: Producer }` (D-151)
- `useContext(X)` → `Boundary { kind: EventChannel, name: "Context:<variable_name>", role: Consumer }` (D-151)
- `<X.Provider>` JSX → confirms producer role for context X

Register for extensions: `["ts", "tsx", "js", "jsx"]`.

### D8: Next.js Semantic Boundaries (`src/semantic/nextjs.rs`)

`NextBoundaryExtractor` implementing `BoundaryExtractor` (D-152):
- Files matching Next.js page conventions → `Boundary { kind: HttpRoute, name: "<route_path>", ... }`
- Files matching API route conventions → `Boundary { kind: HttpRoute, name: "API:<route_path>", ... }`
- `"use client"` directive → `Boundary { kind: EventChannel, name: "ClientBoundary", role: Producer }`

Route path derived from file's `CanonicalPath` via convention matching (D-152) — self-contained, does not depend on `NextRouteInfo`. Example: `app/dashboard/settings/page.tsx` → `/dashboard/settings`.

Register for extensions: `["ts", "tsx", "js", "jsx"]`.

### D9: TypeScriptResolver Bundler Integration (`src/parser/typescript.rs`)

Add `with_bundler_configs()` construction-time injection method (D-118 pattern):

```rust
impl TypeScriptResolver {
    pub fn with_bundler_configs(mut self, configs: BTreeMap<PathBuf, BundlerConfig>) -> Self {
        self.bundler_configs = configs;
        self
    }
}
```

Updated resolution order in `resolve()` (D-150):
0. Workspace import match (existing, unchanged)
1. tsconfig `paths` alias match (existing, unchanged)
2. Bundler alias match (new — find nearest bundler config, check alias prefix match)
3. Bare specifier skip (existing, unchanged)
4. Relative path with extension probing (existing, unchanged)
5. tsconfig `baseUrl` fallback (existing, unchanged)

tsconfig paths take priority over bundler aliases because tsconfig is the language-level standard (D-150). Bundler alias matching: for each alias in nearest bundler config, check if specifier starts with alias prefix. If match, substitute and probe extensions.

### D10: Config Discovery Extension (`src/parser/config/mod.rs`)

Extend `ProjectConfig`:
```rust
pub struct ProjectConfig {
    // ... existing fields ...
    pub bundler_configs: BTreeMap<PathBuf, BundlerConfig>,
    pub turbo_config: Option<TurboConfig>,
    pub next_routes: Option<NextRouteInfo>,
}
```

Extend `discover_config()`:
- `discover_bundler_configs()` — scan for vite.config.*/webpack.config.* files, parse each
- `discover_turbo_config()` — scan for turbo.json
- `discover_next_routes()` — scan for app/ and pages/ directories with Next.js conventions

Extend `with_project_config()` in `ParserRegistry`:
- Pass `bundler_configs` to `TypeScriptResolver::with_bundler_configs()`
- Register `ReactBoundaryExtractor` and `NextBoundaryExtractor`

## Success Criteria

### Infrastructure (D1-D2)
1. GIVEN any existing parser, WHEN `raw_parse` is not overridden, THEN behavior is identical to current pipeline (no regression)
2. GIVEN a `.razor` file with mostly HTML content (>50% ERROR nodes as C#), WHEN parsed, THEN imports are correctly extracted via `raw_parse` (latent bug fixed)
3. GIVEN a `.cs` file, WHEN parsed, THEN standard tree-sitter pipeline is used (raw_parse returns None)

### Bundler Resolution (D3-D5, D9-D10)
4. GIVEN a Vite project with `resolve.alias: { '@': './src' }` (string literal), WHEN Ariadne builds the graph, THEN `import Foo from '@/components/Foo'` resolves to `src/components/Foo.{ts,tsx,js,jsx}`
5. GIVEN a Vite project with `resolve.alias: { '@': fileURLToPath(new URL('./src', import.meta.url)) }`, WHEN Ariadne builds the graph, THEN the alias resolves correctly (idiom recognized)
6. GIVEN a Vite project with array alias form `[{ find: '@', replacement: './src' }]`, WHEN parsed, THEN alias is correctly extracted
7. GIVEN a Webpack project with `resolve.alias: { '@': path.resolve(__dirname, 'src') }`, WHEN Ariadne builds the graph, THEN the alias resolves correctly (path.resolve idiom recognized)
8. GIVEN a Webpack project with `resolve.alias: { '@': path.join(__dirname, 'src') }`, WHEN parsed, THEN alias resolves correctly
9. GIVEN a Webpack project with `resolve.alias: { '@': getAliases() }` (dynamic), WHEN parsed, THEN W045 warning is emitted with the source text `getAliases()`, and alias is skipped
10. GIVEN a project with both tsconfig paths AND bundler aliases for the same prefix, WHEN resolving, THEN tsconfig paths take priority
11. GIVEN turbo.json with `{ "pipeline": { "build": { "dependsOn": ["^build"] } } }`, WHEN parsed, THEN pipeline dependencies are correctly extracted
12. GIVEN a project with no bundler config files, WHEN building graph, THEN behavior is identical to current (no regression, no warnings)

### Framework Detection (D7)
13. GIVEN a React file with `function Button() { return <button>Click</button> }`, WHEN detecting, THEN `react_component: true`
14. GIVEN a file with `function useAuth() { ... }`, WHEN detecting, THEN `react_hook: true`
15. GIVEN a file with `const ThemeContext = createContext(defaultTheme)`, WHEN detecting, THEN `context_provider: ["ThemeContext"]`
16. GIVEN a file with `const theme = useContext(ThemeContext)`, WHEN detecting, THEN `context_consumer: ["ThemeContext"]`
17. GIVEN a file starting with `"use client"`, WHEN detecting, THEN `client_component: true`
18. GIVEN a `.tsx` file under `app/` directory without `"use client"` directive, WHEN detecting with file path context, THEN `server_component: true` (App Router defaults to server components)

### Next.js Routes (D6, D8)
19. GIVEN `app/dashboard/page.tsx`, WHEN discovering routes, THEN route `/dashboard` is extracted with kind Page
20. GIVEN `app/api/users/route.ts`, WHEN discovering routes, THEN route `/api/users` is extracted with kind ApiRoute
21. GIVEN `app/dashboard/layout.tsx`, WHEN discovering routes, THEN route `/dashboard` is extracted with kind Layout
22. GIVEN `middleware.ts` at project root, WHEN discovering routes, THEN middleware is detected with kind Middleware
23. GIVEN `pages/about.tsx` (Pages Router), WHEN discovering routes, THEN route `/about` is extracted
24. GIVEN both `app/` and `pages/` directories, WHEN discovering, THEN `router_type: Both` and both routers' routes are collected

### Semantic Boundaries (D7-D8)
25. GIVEN `createContext()` in file A and `useContext()` in file B, WHEN extracting boundaries, THEN EventChannel boundaries connect them with producer/consumer roles
26. GIVEN `app/dashboard/page.tsx`, WHEN extracting boundaries, THEN HttpRoute boundary with path `/dashboard` is created
27. GIVEN `app/api/users/route.ts`, WHEN extracting boundaries, THEN HttpRoute boundary with path `API:/api/users` is created

### General
28. All existing tests continue to pass (no regression)
29. `cargo test` passes
30. E2E verification on at least one real React/Next.js project with Vite or Webpack aliases

## Testing Requirements

Per `design/testing.md`:

### L1: Parser Snapshot Tests
- Vite config: literal alias, fileURLToPath alias, array form alias, mixed (some recognized, some W045)
- Webpack config: path.resolve alias, path.join alias, concatenation alias, dynamic alias (W045)
- Turbo.json: pipeline with dependencies, empty pipeline, missing file
- .razor via raw_parse: verify same output as current implementation (migration test)

### L2: Resolver Integration Tests
- Bundler alias resolution with fixture projects (Vite aliases, Webpack aliases)
- Priority ordering: tsconfig paths > bundler aliases > relative path
- Edge cases: no bundler config, empty aliases, conflicting aliases between tsconfig and bundler
- Nearest bundler config lookup (monorepo with multiple configs)

### L3: Framework Detection Tests
- React: JSX component, hook function, non-hook `use*` (e.g., `user`), createContext, useContext
- Next.js: "use client" directive, "use server" directive, no directive (server by default)
- Route detection: app router pages, layouts, loading, error, API routes, middleware
- Pages router: file-based routes in pages/ directory

### L4: E2E on Real Projects
- Run on a real React/Next.js project with Vite config
- Verify: alias resolution produces correct edges, framework hints are plausible, route boundaries extracted
- Check edge counts and compare with/without bundler config support

## File Size Planning

| File | Estimated lines | Action if >300 |
|------|----------------|----------------|
| `src/parser/config/bundler.rs` | ~250-280 | Split to `bundler/vite.rs` + `bundler/webpack.rs` + `bundler/mod.rs` |
| `src/parser/config/nextjs.rs` | ~150-180 | OK |
| `src/parser/config/turbo.rs` | ~80-100 | OK |
| `src/detect/js_framework.rs` | ~200-250 | OK (follows D-141 pattern) |
| `src/semantic/react.rs` | ~100-130 | OK |
| `src/semantic/nextjs.rs` | ~100-130 | OK |

## Architecture Compliance

| Constraint | Status | Notes |
|-----------|--------|-------|
| Tree-sitter for AST extraction (D-001) | OK | Bundler configs parsed with tree-sitter-typescript. D-146 clarifies container format exception (Phase 13b). |
| No regex-based parsing (D-001) | OK | All Phase 13a parsing is tree-sitter based. No regex in this phase. |
| Determinism (D-006) | OK | BTreeMap for aliases, sorted discovery, deterministic AST walking |
| Consumer-agnostic (D-004) | OK | Framework detection = source code knowledge, not consumer knowledge |
| Module boundaries (D-033) | OK | Config in parser/config/, detection in detect/, boundaries in semantic/ |
| File size ≤300 lines | PLANNED | bundler.rs may need split (see table above) |
| ImportResolver unchanged (D-118) | OK | with_bundler_configs() construction-time injection |
| No HashMap in model/serial | OK | All new code in parser/, detect/, semantic/ |
| LanguageParser trait extension (D-145) | OK | raw_parse with backward-compatible default |
| Warning codes (D-148) | OK | W044-W048 allocated |

## Phase 13b Preview (Separate Spec)

Phase 13b depends on D-145 (`raw_parse`) delivered in 13a. Scope:
- SFC state machine block extractor (shared infrastructure)
- VueParser (`.vue` — script + style extraction via state machine, D-146)
- SvelteParser (`.svelte` — same approach)
- AstroParser (`.astro` — frontmatter `---` extraction)
- CSS `@import`/`@use` extraction from `<style>` blocks
- Angular DI/module parsing (decorators via tree-sitter-typescript)
- Remix loader/action patterns (standard TS exports)
