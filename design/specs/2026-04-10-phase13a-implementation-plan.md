# Phase 13a: Implementation Plan

**Spec:** `design/specs/2026-04-10-phase13-js-frameworks.md`

## Chunk Dependency Graph

```
C0 (warning codes)
├── C1 → C2 (.razor migration)
├── C3 (Vite config) ──┐
├── C4 (Webpack config) ┼── C6 (resolver bundler integration)
├── C5 (React detection) ┬── C7 (React boundaries)
│                        └── C9 (Next.js boundaries)
├── C10 (Next.js routes + Turbo)
│
└── C8 (config discovery wiring) ← depends on C3, C4, C6, C7, C9, C10
    └── C11 (integration + E2E)
```

Parallelism: C1, C3, C4, C5, C10 are independent of each other (all depend only on C0).
C7 + C9 are independent of C6. C2 is an isolated branch (C1 → C2, no downstream).

## Chunk 0: Warning Codes (D-148)

**Files:** `src/diagnostic.rs`

**What:**
Add warning code variants to `WarningCode` enum and the corresponding match arms in `code()` and `emit()` (exhaustive match at ~line 384).

Note: W043 is not allocated — the sequential gap between W042 (Phase 12) and W044 (Phase 13) follows the pattern of per-phase code blocks. W046 is reserved for Phase 13b (`SfcBlockExtractionError`) per D-148.

Phase 13a codes (per D-148):

| Code | Variant name | Meaning |
|------|-------------|---------|
| W044 | `W044BundlerConfigParseError` | Bundler config file failed to parse |
| W045 | `W045DynamicAliasSkipped` | Alias value doesn't match recognized idiom (D-147) |
| W047 | `W047TurboConfigParseError` | turbo.json failed to parse |
| W048 | `W048NextConfigParseError` | next.config.js parse / route discovery failure |

W046 (`SfcBlockExtractionError`) is Phase 13b — do NOT add it in this phase.

**Depends on:** nothing
**Criteria:** `cargo test` passes, no new warnings

---

## Chunk 1: `raw_parse` Trait Extension (D-145)

**Files:** `src/parser/traits.rs`, `src/parser/registry.rs`

**What:**
1. **Resolve `ParseOutcome` location first.** `ParseOutcome` currently lives in `registry.rs` (~line 22). `traits.rs` cannot import from `registry.rs` (registry imports traits → circular). Move `ParseOutcome` enum definition into `traits.rs` (alongside `LanguageParser`). Update `registry.rs` to import `ParseOutcome` from `traits.rs`. This is a prerequisite — without it, step 2 won't compile.
2. Add `use crate::model::CanonicalPath` to `traits.rs` (not currently imported there).
3. Add default method to `LanguageParser`:
   ```rust
   fn raw_parse(&self, source: &[u8], extension: &str, path: &CanonicalPath)
       -> Option<ParseOutcome> { None }
   ```
4. In `parse_source()` (registry.rs): before tree-sitter parse, call `parser.raw_parse(source, extension, path)`. If `Some(outcome)`, return it directly — bypass tree-sitter top-level parse, error-rate check, AND symbol/boundary extraction.
5. In `reparse_imports()`: same early-return check. Extract imports from the `ParseOutcome`.

**Depends on:** C0
**Criteria:** SC-1 (existing parsers unaffected), `cargo test` green

---

## Chunk 2: .razor Migration to `raw_parse` (D2)

**Files:** `src/parser/csharp.rs`

**What:**
1. Implement `raw_parse()` on `CSharpParser`:
   - `.razor` extension → call existing `extract_razor_imports()`, return `ParseOutcome::Ok(imports, vec![], vec![], vec![])`
   - Other extensions → return `None`
2. Remove `is_razor_content()` check from `extract_imports()` (it's now handled by `raw_parse`)
3. Verify that .razor files with >50% ERROR nodes are now correctly handled (the latent bug fix)

**Depends on:** C1
**Criteria:** SC-2, SC-3, existing csharp tests pass

---

## Chunk 3: Vite Config Parsing (D3)

**Files:** `src/parser/config/bundler.rs` (new)

**What:**
1. Define `BundlerConfig` struct:
   ```rust
   pub struct BundlerConfig {
       pub config_dir: PathBuf,
       pub aliases: BTreeMap<String, String>,
       pub modules: Vec<String>, // Webpack resolve.modules (empty for Vite)
   }
   ```
2. Implement `parse_vite_config(source: &[u8], config_path: &Path, diag: &DiagnosticCollector) -> Option<BundlerConfig>`:
   - Parse with tree-sitter-typescript
   - Navigate AST: `defineConfig(...)` or direct `export default` → `resolve` → `alias`
   - Recognize 3 value patterns: string literal, `fileURLToPath(new URL(...))`, array form `[{find, replacement}]`
   - Unrecognized → emit W045
3. `discover_vite_configs(known_files, project_root, diag) -> BTreeMap<PathBuf, BundlerConfig>`
   - Scan for `vite.config.{ts,js,mjs}`

**Depends on:** C0
**Criteria:** SC-4, SC-5, SC-6, snapshot tests for each idiom

---

## Chunk 4: Webpack Config Parsing (D4)

**Files:** `src/parser/config/bundler.rs` (append)

**What:**
1. Implement `parse_webpack_config(source: &[u8], config_path: &Path, diag: &DiagnosticCollector) -> Option<BundlerConfig>`:
   - Parse with tree-sitter-typescript
   - Navigate AST: `module.exports` or `export default` → `resolve` → `alias`
   - Recognize value patterns: string literal, `path.resolve(__dirname, ...)`, `path.join(__dirname, ...)`, `__dirname + '/...'`
   - Unrecognized → emit W045
   - Extract `resolve.modules` if present (array of string literals) → store in `BundlerConfig.modules`
2. `discover_webpack_configs(known_files, project_root, diag) -> BTreeMap<PathBuf, BundlerConfig>`
   - Scan for `webpack.config.{js,ts}`

**Depends on:** C0
**Criteria:** SC-7, SC-8, SC-9, snapshot tests for each idiom

---

## Chunk 5: React Framework Detection (D7 — detection part, D-129)

**Files:** `src/detect/js_framework.rs` (new)

**What:**
1. Define `JsFrameworkHints` struct (all fields from spec)
2. Define `RouteConvention` enum (pub — exported for use by C7/C9 boundary extractors)
3. Implement `detect_js_framework(tree: &Tree, source: &[u8], path: &CanonicalPath) -> JsFrameworkHints`:
   - JSX elements → `react_component`
   - Function name `use*` + hook patterns → `react_hook`
   - `createContext(...)` → extract name → `context_provider`
   - `useContext(X)` → extract name → `context_consumer`
   - `"use client"` first statement → `client_component`
   - `"use server"` first statement → detect server actions
   - Path under `app/` without `"use client"` → `server_component`
   - Filepath convention matching → `route_convention`
4. Extract reusable AST helpers as `pub` functions for C7/C9 to import:
   - `find_create_context_calls(tree, source) -> Vec<String>` (context names)
   - `find_use_context_calls(tree, source) -> Vec<String>` (context names)
   - `has_use_client_directive(tree, source) -> bool`
   - `classify_route_convention(path: &CanonicalPath) -> Option<RouteConvention>`
5. Register in `src/detect/mod.rs` (pub mod + re-export, same pattern as `java_framework`)

**Depends on:** C0
**Criteria:** SC-13 through SC-18, snapshot tests for each detection case

---

## Chunk 6: TypeScriptResolver Bundler Integration (D-150, D-118)

**Files:** `src/parser/typescript.rs`

**What:**
1. Add `bundler_configs: Option<BTreeMap<PathBuf, BundlerConfig>>` field to `TypeScriptResolver`
2. Add `with_bundler_configs(mut self, configs: BTreeMap<PathBuf, BundlerConfig>) -> Self` builder method
3. In `resolve()`, insert bundler alias matching at priority 2 (after tsconfig paths, before bare specifier skip) per D-150:
   - Find nearest bundler config (walk parent dirs of the importing file)
   - Check if specifier starts with any alias prefix
   - If match: substitute prefix, probe extensions
4. Update `resolve()` docs to reflect new priority order

**Depends on:** C3 (needs `BundlerConfig` type defined in `bundler.rs`)
**Criteria:** SC-10 (tsconfig > bundler priority), integration tests with fixture projects

---

## Chunk 7: React Boundary Extraction (D-151)

**Files:** `src/semantic/react.rs` (new)

**What:**
1. Implement `ReactBoundaryExtractor` with `BoundaryExtractor` trait:
   - `extensions()` → `["ts", "tsx", "js", "jsx"]`
   - `extract()` — imports and calls helpers from `crate::detect::js_framework`:
     - `find_create_context_calls()` → `Boundary { kind: EventChannel, name: "Context:<var>", role: Producer }`
     - `find_use_context_calls()` → `Boundary { kind: EventChannel, name: "Context:<var>", role: Consumer }`
     - `<X.Provider>` JSX → confirms producer role
2. Register in `src/semantic/mod.rs`

**Depends on:** C5 (imports `find_create_context_calls`, `find_use_context_calls` from `detect::js_framework`)
**Criteria:** SC-25

---

## Chunk 8: Config Discovery Extension (D10, D-118)

**Files:** `src/parser/config/mod.rs`, `src/parser/registry.rs`

**What:**
1. Add fields to `ProjectConfig`:
   ```rust
   pub bundler_configs: BTreeMap<PathBuf, BundlerConfig>,
   pub turbo_config: Option<TurboConfig>,
   pub next_routes: Option<NextRouteInfo>,
   ```
2. In `discover_config()`: call `discover_bundler_configs()`, `discover_turbo_config()`, `discover_next_routes()`
3. In `with_project_config()`:
   - Pass `bundler_configs` to `TypeScriptResolver::with_bundler_configs()`
   - Register `ReactBoundaryExtractor` and `NextBoundaryExtractor`
4. **Update `empty_config()` test helper** (~line 575 in `config/mod.rs`) — add the 3 new fields with default values (`BTreeMap::new()`, `None`, `None`). Without this, 6+ existing tests will fail to compile.

**Depends on:** C3, C4, C6, C7, C9, C10 (all new config types + extractors)
**Criteria:** SC-12 (no bundler config = no regression)

---

## Chunk 9: Next.js Boundary Extraction (D-152)

**Files:** `src/semantic/nextjs.rs` (new)

**What:**
1. Implement `NextBoundaryExtractor` with `BoundaryExtractor` trait:
   - `extensions()` → `["ts", "tsx", "js", "jsx"]`
   - `extract()` — imports helpers from `crate::detect::js_framework`:
     - `classify_route_convention(path)` → match on `RouteConvention` variants:
       - `NextPage` → `Boundary { kind: HttpRoute, name: "<route_path>" }`
       - `NextApiRoute` → `Boundary { kind: HttpRoute, name: "API:<route_path>" }`
     - `has_use_client_directive()` → `Boundary { kind: EventChannel, name: "ClientBoundary", role: Producer }`
   - Route path derived from `CanonicalPath` (self-contained, no `NextRouteInfo` dependency per D-152)
2. Register in `src/semantic/mod.rs`

**Depends on:** C5 (imports `RouteConvention`, `classify_route_convention`, `has_use_client_directive` from `detect::js_framework`)
**Criteria:** SC-26, SC-27

---

## Chunk 10: Next.js Filesystem Routing + Turbopack (D5, D6)

**Files:** `src/parser/config/nextjs.rs` (new), `src/parser/config/turbo.rs` (new)

**What:**

**Next.js routes:**
1. Define `NextRouteInfo`, `NextRoute`, `NextRouterType`, `NextRouteKind` structs/enums
2. Implement `discover_next_routes(project_root: &Path, known_files: &FileSet, diag: &DiagnosticCollector) -> Option<NextRouteInfo>`:
   - Detect App Router (`app/` with page files) and Pages Router (`pages/` with route files)
   - Scan convention files: page, layout, loading, error, template, not-found
   - API routes: `app/api/**/route.*` or `pages/api/**/*.*`
   - Middleware: `middleware.{ts,js}` at root

**Turbopack:**
1. Define `TurboConfig`, `TurboPipelineEntry` structs
2. Implement `parse_turbo_config(source: &[u8], config_path: &Path, diag: &DiagnosticCollector) -> Option<TurboConfig>`
   - Sort `depends_on` and `outputs` Vecs after parsing for determinism (D-006)
3. `discover_turbo_config(known_files, project_root, diag) -> Option<TurboConfig>`

**Depends on:** C0
**Criteria:** SC-11 (turbo), SC-19 through SC-24 (Next.js routes)

---

## Chunk 11: Integration + E2E

**Files:** tests, no production code changes

**What:**
1. Integration test: fixture project with Vite aliases → verify edges resolve correctly
2. Integration test: fixture project with Webpack aliases → verify edges resolve correctly
3. Integration test: tsconfig paths + bundler alias conflict → tsconfig wins
4. Run `cargo test` — all 30 success criteria
5. E2E on a real React/Next.js project (SC-30)
6. Review all file sizes against 300-line limit

**Depends on:** C8 (everything wired together)
**Criteria:** SC-28, SC-29, SC-30

---

## Implementation Order (Sequential)

| Step | Chunk | Deliverable | Can parallelize with |
|------|-------|-------------|---------------------|
| 1 | C0 | Warning codes | — |
| 2 | C1 | raw_parse trait (+ ParseOutcome move) | C3, C4, C5, C10 |
| 3 | C2 | .razor migration | C3, C4, C5, C10 |
| 4 | C3 | Vite config | C1, C2, C4, C5, C10 |
| 5 | C4 | Webpack config | C1, C2, C3, C5, C10 |
| 6 | C5 | React detection | C1, C2, C3, C4, C10 |
| 7 | C10 | Next.js routes + Turbo | C1, C2, C3, C4, C5 |
| 8 | C6 | Resolver bundler integration | C7, C9 |
| 9 | C7 | React boundaries | C6 |
| 10 | C9 | Next.js boundaries | C6 |
| 11 | C8 | Config discovery wiring | — |
| 12 | C11 | Integration + E2E | — |

**Recommended serial path:** C0 → C1 → C2 → C3 → C4 → C5 → C10 → C6 → C7 → C9 → C8 → C11

**Critical path (5 steps):** C0 → C3 → C6 → C8 → C11 (or equivalently C0 → C5 → C7/C9 → C8 → C11). C1 → C2 is an isolated 2-step branch off the critical path.

The parallelism column shows what *could* run concurrently in theory — in practice, serial execution with `cargo test` after each chunk is safer per the project's development process.

## File Size Risk

`src/parser/config/bundler.rs` is the only file at risk (~250-280 estimated). If it exceeds 300 lines after C4, immediately split into `bundler/mod.rs` + `bundler/vite.rs` + `bundler/webpack.rs`.
