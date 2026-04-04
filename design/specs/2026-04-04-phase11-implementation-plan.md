# Phase 11: C# / .NET — Implementation Plan

## Overview

This plan describes the implementation as executed. Phase 11 was implemented in 8 chunks following the architecture document's ordering.

## Chunk 1: Foundation — Cargo.toml + csproj.rs + Warning Codes

**Dependencies:** None

**Files:**
- `Cargo.toml` — add `roxmltree = "0.21"`
- `src/parser/config/csproj.rs` — new file: `CsprojConfig`, `DotnetSolutionInfo`, `ProjectRef`, `PackageRef`, `SlnProjectEntry`, `parse_csproj()`, `parse_sln()`
- `src/diagnostic.rs` — add W034-W037 warning codes

**What:**
- Add roxmltree dependency
- Implement .csproj XML parsing: extract PropertyGroup fields (TargetFramework, RootNamespace, AssemblyName), ItemGroup entries (ProjectReference, PackageReference). Process all elements regardless of MSBuild Condition attributes (D-133).
- Implement .sln line-based parsing: regex-match `Project("GUID") = "Name", "Path", "GUID"` lines
- Add four warning codes for .NET config parse errors

## Chunk 2: Config Discovery — config/mod.rs Extensions

**Dependencies:** Chunk 1

**Files:**
- `src/parser/config/mod.rs` — extend `ProjectConfig` with `csproj_configs` and `dotnet_solution` fields; add `discover_csproj_configs()` and `discover_dotnet_solution()` functions

**What:**
- Scan known_files for .csproj extensions, parse each, store in BTreeMap keyed by project-relative path
- Scan known_files for .sln extensions at project root; warn W037 if multiple found
- Resolve ProjectReference paths relative to .csproj directory, normalize to project-relative

## Chunk 3: Config-Aware Resolver — CSharpResolver Rewrite

**Dependencies:** Chunks 1-2

**Files:**
- `src/parser/csharp.rs` — rewrite CSharpResolver with config injection
- `src/parser/registry.rs` — inject csproj config in `with_project_config()`

**What:**
- Add `with_csproj_configs()` builder method
- Implement resolution algorithm: framework prefix filter, NuGet package filter, nearest-project lookup, namespace-to-path mapping with root namespace stripping, cross-project resolution, file set scan fallback
- Wire into registry: if csproj_configs non-empty, construct config-aware resolver

## Chunk 4: ImportKind::ProjectReference

**Dependencies:** Chunk 3

**Files:**
- `src/parser/traits.rs` — add `ImportKind::ProjectReference` variant
- `src/model/edge.rs` — add `EdgeType::ProjectRef` variant
- `src/serial/output.rs` — serialize as `"project_ref"`

**What:**
- New enum variant for cross-project reference edges
- Serialization mapping in JSON output

## Chunk 5: Framework Detection

**Dependencies:** Chunk 1

**Files:**
- `src/detect/framework.rs` — new file: `DotnetFrameworkHints`, `detect_dotnet_framework()`
- `src/detect/mod.rs` — declare framework module

**What:**
- AST-based detection heuristics for ASP.NET Controller, Middleware, MinimalAPI, EF DbContext, EF Migration, Blazor Component, MAUI Page, DI Registration
- Boolean flags in `DotnetFrameworkHints` struct

## Chunk 6: Semantic Boundary Extractors

**Dependencies:** None (parallel with Chunks 3-5)

**Files:**
- `src/semantic/dotnet.rs` — new file: EF DbContext extractor, DI registration extractor
- `src/semantic/mod.rs` — declare dotnet module

**What:**
- EF: extract entity types from `DbSet<T>` properties
- DI: extract service registrations from `builder.Services.Add*()` calls
- Produce `Boundary` entries with appropriate `BoundaryKind` values

## Chunk 7: .razor File Support

**Dependencies:** Chunks 3, 6

**Files:**
- `src/parser/csharp.rs` — add "razor" to extensions, regex directive extraction

**What:**
- `@using Namespace` -> RawImport
- `@inject ServiceType PropertyName` -> RawImport
- `@page "/route"` -> Boundary (HttpRoute, Producer)
- `@inherits BaseComponent` -> RawImport
- Extension check dispatches to tree-sitter (.cs) or regex (.razor) path

## Chunk 8: Test Fixtures

**Dependencies:** All previous chunks

**Files:**
- `tests/fixtures/dotnet-webapi/` — ASP.NET Web API fixture
- `tests/fixtures/dotnet-blazor/` — Blazor fixture with .razor files
- `tests/fixtures/dotnet-efcore/` — Entity Framework Core fixture
- `tests/fixtures/dotnet-maui/` — MAUI fixture

**What:**
- Each fixture contains .csproj, .cs source files, and expected graph output
- Tests verify: correct parsing, resolution, framework detection, boundary extraction, warning emission

## Chunk 9: Documentation

**Dependencies:** Implementation stabilization

**Files:**
- `design/decisions/log.md` — D-124 through D-133
- `design/ROADMAP.md` — mark Phase 11 as DONE
- `design/architecture.md` — add Phase 11 features
- `design/specs/2026-04-04-phase11-csharp-dotnet.md` — retrospective spec

## Verification

All 1013 tests pass after implementation. No regressions in existing language support.
