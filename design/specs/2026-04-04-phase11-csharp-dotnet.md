# Phase 11: Deep Language Support — C# / .NET — Specification

## Goal

Add full C# and .NET project understanding to Ariadne. The Phase 1a C# parser handles `using` directives and public declarations but resolves namespaces via naive path mapping. Phase 11 adds `.csproj` and `.sln` awareness, enabling accurate namespace-to-file resolution, project reference tracking, NuGet/framework assembly filtering, .NET framework detection, and semantic boundary extraction.

## Dependencies

| Phase | Status | What It Provides |
|-------|--------|-----------------|
| Phase 1a (MVP) | DONE | CSharpParser + CSharpResolver with basic namespace-to-path heuristic |
| Phase 10 (Config-Aware Resolution) | DONE | ProjectConfig, ConfigDiscovery, construction-time injection pattern (D-118) |
| Phase 8 (Semantic Boundaries) | DONE | BoundaryExtractor trait, SemanticState, boundary persistence |

## Risk Classification

**Overall: GREEN** — Follows established Phase 10 pattern exactly. New dependency (roxmltree) is minimal and well-proven.

| # | Deliverable | Risk | Rationale |
|---|-------------|------|-----------|
| D1 | .csproj parsing | GREEN | roxmltree is mature; .csproj structure is well-documented |
| D2 | .sln parsing | GREEN | Line-oriented format, simple string matching |
| D3 | Config-aware CSharpResolver | GREEN | Follows D-118 pattern exactly |
| D4 | Framework detection | GREEN | Heuristic-based, graceful degradation |
| D5 | Semantic boundary extractors | GREEN | Follows existing BoundaryExtractor pattern |
| D6 | .razor support | GREEN | Regex for line-oriented directives |

## Design Sources

| Decision | Description | Source |
|----------|-------------|--------|
| D-124 | roxmltree for .csproj XML parsing | Phase 11 architecture doc |
| D-125 | CsprojConfig as flat data struct | Phase 10 pattern (D-118) |
| D-126 | Line-based .sln parsing | go.mod precedent |
| D-127 | DotnetSolutionInfo separate from WorkspaceInfo | Type safety |
| D-128 | Construction-time .csproj injection into CSharpResolver | D-118 pattern |
| D-129 | Framework detection split: detect/ + semantic/ | Separation of concerns |
| D-130 | .razor directive extraction via regex | No tree-sitter grammar |
| D-131 | Warning codes W034-W037 | D-003 graceful degradation |
| D-132 | ImportKind::ProjectReference for cross-project edges | Edge type semantics |
| D-133 | MSBuild Condition attributes ignored | Over-approximation principle |

## Deliverables

### D1: .csproj and .sln Parsing (`src/parser/config/csproj.rs`)

- `CsprojConfig` struct: target_framework, root_namespace, assembly_name, project_references, package_references
- `DotnetSolutionInfo` struct: solution path, project entries with type GUIDs
- `parse_csproj()` using roxmltree 0.21
- `parse_sln()` using line-based string matching
- Warning codes W034 (CsprojParseError), W035 (SlnParseError)

### D2: Config Discovery Extension (`src/parser/config/mod.rs`)

- `ProjectConfig` extended with `csproj_configs: BTreeMap<PathBuf, CsprojConfig>` and `dotnet_solution: Option<DotnetSolutionInfo>`
- Discovery functions scan known_files for .csproj/.sln extensions
- ProjectReference path resolution (relative to .csproj directory, normalized to project-relative)
- Warning codes W036 (ProjectRefNotFound), W037 (MultipleSlnFiles)

### D3: Config-Aware CSharpResolver (`src/parser/csharp.rs`)

- `with_csproj_configs()` construction-time injection
- Resolution algorithm: framework filter -> NuGet filter -> nearest-project lookup -> namespace-to-path mapping (with root namespace stripping) -> cross-project resolution -> file set scan fallback
- Registry integration in `with_project_config()`

### D4: Framework Detection (`src/detect/framework.rs`)

- `DotnetFrameworkHints` struct with boolean flags for ASP.NET Controller, Middleware, MinimalAPI, EF DbContext, EF Migration, Blazor Component, MAUI Page, DI Registration
- `detect_dotnet_framework()` function using tree-sitter AST analysis

### D5: Semantic Boundary Extractors (`src/semantic/dotnet.rs`)

- EF DbContext boundary extractor (entity types from DbSet<T> properties)
- DI registration boundary extractor (service registrations from builder.Services.Add*() calls)

### D6: .razor File Support (`src/parser/csharp.rs`)

- `"razor"` added to CSharpParser extensions
- Regex-based directive extraction: @using, @inject, @inherits, @page
- @page directives produce Boundary entries (BoundaryKind::HttpRoute)

### D7: ImportKind::ProjectReference (`src/parser/traits.rs`, `src/serial/output.rs`)

- New ImportKind variant for .csproj ProjectReference entries
- Serializes as EdgeType::ProjectRef ("project_ref") in graph.json

## File Impact Summary

| File | Change Type | Description |
|------|------------|-------------|
| `Cargo.toml` | Modify | Add `roxmltree = "0.21"` |
| `src/parser/config/csproj.rs` | **New** | CsprojConfig, DotnetSolutionInfo, parse_csproj, parse_sln |
| `src/parser/config/mod.rs` | Modify | Add csproj/sln discovery, extend ProjectConfig |
| `src/parser/csharp.rs` | Modify | Config-aware CSharpResolver, .razor support |
| `src/parser/registry.rs` | Modify | Inject csproj config in with_project_config() |
| `src/parser/traits.rs` | Modify | Add ImportKind::ProjectReference |
| `src/detect/framework.rs` | **New** | DotnetFrameworkHints, detect_dotnet_framework() |
| `src/detect/mod.rs` | Modify | Declare framework module |
| `src/semantic/dotnet.rs` | **New** | EF, DI boundary extractors |
| `src/semantic/mod.rs` | Modify | Declare dotnet module |
| `src/diagnostic.rs` | Modify | Add W034-W037 warning codes |
| `src/serial/output.rs` | Modify | Serialize ImportKind::ProjectReference as "project_ref" |
| `tests/fixtures/dotnet-webapi/` | **New** | ASP.NET Web API test fixture |
| `tests/fixtures/dotnet-blazor/` | **New** | Blazor test fixture |
| `tests/fixtures/dotnet-efcore/` | **New** | Entity Framework Core test fixture |
| `tests/fixtures/dotnet-maui/` | **New** | MAUI test fixture |
| `design/decisions/log.md` | Modify | Add D-124 through D-133 |
| `design/ROADMAP.md` | Modify | Mark Phase 11 as DONE |
| `design/architecture.md` | Modify | Add Phase 11 features |

## Success Criteria

1. `cargo test` passes all 1013 tests (no regressions)
2. .csproj files parsed correctly with roxmltree — ProjectReferences, PackageReferences, TargetFramework extracted
3. .sln files parsed correctly — project entries with paths and type GUIDs
4. CSharpResolver filters framework assemblies (System.*, Microsoft.*) and NuGet packages
5. Namespace-to-path resolution works with root namespace stripping
6. Cross-project resolution follows ProjectReference edges
7. Framework detection identifies ASP.NET, EF, Blazor, MAUI, MinimalAPI patterns
8. .razor directives (@using, @inject, @page, @inherits) extracted correctly
9. ProjectReference edges serialized as "project_ref" in graph.json
10. Warning codes W034-W037 emitted for .NET config errors
11. Test fixtures cover all four framework patterns (webapi, blazor, efcore, maui)

## Testing

- Unit tests for .csproj parsing (valid, malformed, conditional groups)
- Unit tests for .sln parsing (valid, malformed, multiple solutions)
- Unit tests for CSharpResolver with config injection
- Unit tests for framework detection heuristics
- Unit tests for .razor directive extraction
- Integration tests with dotnet-webapi, dotnet-blazor, dotnet-efcore, dotnet-maui fixtures
- Snapshot tests for serialized graph output with project_ref edges
