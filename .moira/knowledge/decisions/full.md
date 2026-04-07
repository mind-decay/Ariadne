<!-- moira:knowledge decisions L2 -->
# Decisions -- Full

> This file is auto-populated by /moira:init. Manual edits are preserved.
>
> Decision format:
> ## [DATE] Decision title
> CONTEXT: Why this decision was needed
> DECISION: What was chosen
> ALTERNATIVES REJECTED: What was considered and why
> REASONING: Why this choice
> EVIDENCE: Task or source reference (e.g., task-2024-01-15-042)

## [2026-04-05] D-134: roxmltree for pom.xml parsing
CONTEXT: Phase 12 needs pom.xml parsing; roxmltree already a dependency from D-124 (.csproj parsing)
DECISION: Reuse roxmltree 0.21 — zero new dependencies
EVIDENCE: task-2026-04-05-001 architecture.md

## [2026-04-05] D-135: Line-based Gradle DSL subset parsing
CONTEXT: build.gradle (Groovy) and build.gradle.kts (Kotlin DSL) need parsing without full language evaluation
DECISION: Line-based text scanning with brace depth counting; extract dependencies{} and sourceSets{} blocks via regex
NOTE: sourceSets{} extraction was stubbed in Phase 12 implementation — both branches return "src/main/java" (known gap)
EVIDENCE: task-2026-04-05-001 architecture.md, review.md W-1

## [2026-04-05] D-136: settings.gradle for multi-module discovery
CONTEXT: Gradle multi-module projects declare subprojects in settings.gradle
DECISION: Line-based regex extracting include('name') / include("name") directives; map :app → ./app/ by convention
EVIDENCE: task-2026-04-05-001 architecture.md

## [2026-04-05] D-137: GradleConfig and MavenConfig as flat data structs
CONTEXT: Phase 12 needs build config struct representations following Phase 10 pattern (D-125)
DECISION: Two flat structs following CsprojConfig pattern exactly; eager-parsed; no lazy evaluation
EVIDENCE: task-2026-04-05-001 architecture.md

## [2026-04-05] D-138: JavaBuildConfig enum for resolver injection
CONTEXT: JavaResolver needs build config at construction time; Java projects can use Gradle, Maven, or both
DECISION: pub enum JavaBuildConfig { Gradle{...}, Maven{...}, Both{...} }; Gradle preferred in Both variant
EVIDENCE: task-2026-04-05-001 architecture.md

## [2026-04-05] D-139: Nearest-ancestor Gradle/Maven lookup for multi-module
CONTEXT: Java files in subdirectories need to find their nearest build.gradle or pom.xml
DECISION: find_nearest_gradle() and find_nearest_maven() following find_nearest_csproj() / find_nearest_tsconfig() pattern
EVIDENCE: task-2026-04-05-001 architecture.md

## [2026-04-05] D-140: Defer BoundaryKind::DiBinding — use EventChannel with "DI:" prefix
CONTEXT: Java DI extraction needs a BoundaryKind; DiBinding not yet in enum; adding it would touch model+serial+MCP
DECISION: Reuse BoundaryKind::EventChannel with name "DI:ServiceName" — same as .NET DotnetBoundaryExtractor
EVIDENCE: task-2026-04-05-001 architecture.md

## [2026-04-05] D-141: Java framework detection in separate file (java_framework.rs)
CONTEXT: Adding Java framework detection to existing framework.rs (244 lines) would exceed God-object threshold
DECISION: Create src/detect/java_framework.rs; keep framework.rs for .NET only; re-export both from detect/mod.rs
EVIDENCE: task-2026-04-05-001 architecture.md

## [2026-04-05] D-142: Android manifest parsing — minimal scope (PARTIALLY IMPLEMENTED)
CONTEXT: Android projects have AndroidManifest.xml with activity/service/receiver/provider declarations
DECISION: Parse AndroidManifest.xml with roxmltree for component class names only (android:name attribute)
STATUS: W042AndroidManifestParseError defined; parse_android_manifest() not implemented in Phase 12 (deferred)
EVIDENCE: task-2026-04-05-001 review.md S-3

## [2026-04-05] D-143: Warning codes W038-W042 for Java build config errors
CONTEXT: Java build config parsing (Gradle, Maven) can fail; continue from W037 (.NET codes)
DECISION: W038 GradleParseError, W039 MavenParseError, W040 MavenModuleNotFound, W041 GradleSubprojectNotFound, W042 AndroidManifestParseError
EVIDENCE: task-2026-04-05-001 architecture.md

## [2026-04-05] D-144: Classpath-aware resolution strategy for Java
CONTEXT: JavaResolver only tried src/main/java/{path}.java; with build config it can use actual source dirs
DECISION: Resolution order: (1) config source dirs, (2) subproject source dirs, (3) fallback heuristics
EVIDENCE: task-2026-04-05-001 architecture.md
