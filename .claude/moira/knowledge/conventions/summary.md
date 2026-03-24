<!-- moira:freshness init 2026-03-21 -->
<!-- moira:knowledge conventions L1 -->

## 1. Naming Conventions
- Evidence: `src/model/node.rs:8-15` — `FileType::Source`, `FileType::Test`, etc.; `src/model/edge.rs:8-13` — `EdgeType::Imports`, `EdgeType::ReExports`; `src/diagnostic.rs:10-37` — `FatalError::ProjectNotFound`, `FatalError::NotADirectory`
- Evidence: `src/diagnostic.rs:43-59` — `WarningCode::W001ParseFailed`, `WarningCode::W006ImportUnresolved`, `WarningCode::W018BlastRadiusTimeout`
## 2. Import Style
## 3. Export Style
## 4. Error Handling
## 5. Logging
## 6. Code Organization
- `CanonicalPath(String)`, `ContentHash(String)`, `ClusterId(String)`, `Symbol(String)`, `FileSet(BTreeSet<CanonicalPath>)`
- Evidence: `src/model/types.rs:1` — `use std::collections::BTreeSet;` for `FileSet`
- `LanguageParser` and `ImportResolver` traits: `src/parser/traits.rs:31,40`
- `FileWalker` and `FileReader` traits: `src/pipeline/mod.rs` (re-exported from `walk.rs` and `read.rs`)
## 7. Items Searched For But Not Found
