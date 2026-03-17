# Path Resolution & Normalization

## Problem

Import paths in source code are strings. They must be resolved to actual files in the project. This is harder than it sounds:

1. **Case sensitivity** — macOS (HFS+/APFS) is case-insensitive by default. `import './Utils'` finds `utils.ts`. Linux is case-sensitive. Same project produces different graphs on different OS.
2. **Path normalization** — `./foo/../bar/baz.ts` and `./bar/baz.ts` are the same file. Without normalization, they produce different node keys.
3. **Monorepos** — A repo with multiple `go.mod`, `package.json`, or `Cargo.toml` files. Import resolution must know about workspace boundaries.
4. **Same file via multiple paths** — Barrel re-exports, index files, and symlinks mean the same logical module can be reached via different import strings.

## Path Normalization

### Canonical Path Format

All paths in the graph use a **canonical relative format**:

```
Rules:
1. Relative to project root (no absolute paths)
2. Forward slashes only (no backslashes, even on Windows)
3. No leading `./`
4. No `.` or `..` segments (fully resolved)
5. No trailing slash
6. No double slashes
7. Preserves filesystem case (NOT lowercased)
```

Examples:
```
src/auth/login.ts           ✓ canonical
./src/auth/login.ts         ✗ leading ./
src/auth/../utils/format.ts ✗ contains ..
src\auth\login.ts           ✗ backslashes
src/auth/login.ts/          ✗ trailing slash
```

### Normalization Function

```rust
/// Normalize a path to canonical format.
/// Input: any relative path from project root
/// Output: canonical path string (forward slashes, no ./.. segments)
fn normalize_path(path: &Path, project_root: &Path) -> String {
    // 1. Make absolute (join with project_root if relative)
    // 2. Canonicalize (resolve .., ., symlinks)
    //    BUT: use dunce::canonicalize on Windows to avoid \\?\ prefix
    // 3. Strip project_root prefix → relative
    // 4. Replace \ with / (Windows)
    // 5. Strip leading ./
}
```

**When normalization happens:** Once, during file walking. Each walked file gets a canonical path. This path becomes the node key in the graph. All edge references use canonical paths.

### Case Sensitivity

**Problem:** On macOS, `src/Utils.ts` and `src/utils.ts` are the same file. On Linux, they're different. If code says `import './Utils'` and the file is `utils.ts`:
- macOS: import resolves, edge created
- Linux: import doesn't resolve, edge missing

**Decision: Follow the filesystem.**

- During **file walking**, we get paths as the filesystem reports them (actual case on disk).
- During **import resolution**, we try the exact path first. If it doesn't match any walked file, we try case-insensitive matching on case-insensitive filesystems.
- **Detection:** Check if the filesystem is case-insensitive by creating a temp file and checking if it's accessible with different casing. Cache this check per build.

```rust
fn is_case_insensitive(root: &Path) -> bool {
    // Create temp file, check if accessible with swapped case
    // Cache result for the build
}
```

- On case-insensitive FS: resolution tries exact match first, then case-insensitive match. If case-insensitive match found, use the canonical (walked) path, not the import string's casing.
- On case-sensitive FS: exact match only. `import './Utils'` won't resolve if the file is `utils.ts`. This is correct — the code has a bug that would fail on Linux.

**This means:** Same project produces the same graph on macOS and Linux IF the code uses correct casing. If code has case bugs (works on macOS but not Linux), the graph will differ — and this is useful information, not a bug.

### Windows Support

- All internal paths use forward slashes.
- `std::path::MAIN_SEPARATOR` is `\` on Windows — normalize on input.
- Use `dunce` crate (or equivalent) to avoid `\\?\` extended path prefix from `fs::canonicalize` on Windows.
- **Dependency:** `dunce` crate for Windows path canonicalization (no-op on Unix).

## Monorepo Support

### What Is a Monorepo

A repository containing multiple independently-buildable projects that may reference each other. Common patterns:

| Type | Indicator file | Example |
|------|---------------|---------|
| Cargo workspace | `Cargo.toml` with `[workspace]` | Rust monorepos |
| Go workspace | `go.work` | Go 1.18+ multi-module repos |
| npm/yarn/pnpm workspace | `package.json` with `workspaces` | JS/TS monorepos |
| Nx | `nx.json` | Angular/React monorepos |
| Turborepo | `turbo.json` | JS/TS monorepos |
| Python | Multiple `pyproject.toml` | Python monorepos |

### Strategy: Single Graph, Workspace-Aware Resolution

Ariadne always produces **one graph per invocation**. No sub-graphs, no per-package splitting. The entire repo is one graph.

But import resolution must understand workspace boundaries:

```
my-monorepo/
├── packages/
│   ├── auth/
│   │   ├── package.json      {"name": "@myapp/auth"}
│   │   └── src/index.ts
│   ├── api/
│   │   ├── package.json      {"name": "@myapp/api"}
│   │   └── src/server.ts     import { login } from "@myapp/auth"
│   └── shared/
│       ├── package.json      {"name": "@myapp/shared"}
│       └── src/utils.ts
└── package.json               {"workspaces": ["packages/*"]}
```

Without workspace awareness: `import "@myapp/auth"` is unresolved (looks like external package). With workspace awareness: resolve `@myapp/auth` → `packages/auth/src/index.ts`.

### Workspace Detection

During the walk phase, before parsing:

1. **Scan for workspace root indicators** in project root:
   - `package.json` with `"workspaces"` field → npm/yarn/pnpm
   - `pnpm-workspace.yaml` → pnpm
   - `Cargo.toml` with `[workspace]` → Cargo
   - `go.work` → Go workspace
   - `nx.json` → Nx
   - `turbo.json` → Turborepo
   - `lerna.json` → Lerna (legacy)

2. **Build workspace member map:**
   ```rust
   struct WorkspaceInfo {
       kind: WorkspaceKind,  // npm | cargo | go | nx | turbo
       members: Vec<WorkspaceMember>,
   }

   struct WorkspaceMember {
       name: String,         // package name ("@myapp/auth", crate name, module path)
       path: PathBuf,        // relative path to member root
       entry_point: PathBuf, // main file (src/index.ts, src/lib.rs, main.go)
   }
   ```

3. **Pass workspace info to parsers** for import resolution.

### Per-Language Workspace Resolution

**TypeScript/JavaScript:**
- Read root `package.json` → `workspaces` glob patterns → resolve to member directories
- For each member: read its `package.json` → extract `name`, `main`/`module`/`exports`
- Build map: `package_name → entry_point_path`
- During resolution: if import matches a workspace package name → resolve to that member's entry point
- `@scope/name` imports checked against workspace map before being classified as external

**Go:**
- Read `go.work` → extract `use` directives → list of module directories
- For each module: read `go.mod` → extract `module` path
- Build map: `module_path → directory`
- During resolution: if import path starts with a workspace module's path → resolve within that module

**Rust:**
- Read root `Cargo.toml` → `[workspace] members` glob patterns
- For each member: read its `Cargo.toml` → extract `[package] name`
- Rust crate dependencies are resolved by Cargo, not by import paths. `use` statements reference crate names, not paths. Workspace awareness helps map crate names to local paths for `extern crate` (deprecated) and `use other_crate::*` patterns.
- Primary benefit: `path` dependencies in `[dependencies]` can be used to create cross-crate edges

**Python:**
- No standard workspace format. Multiple `pyproject.toml` detected by scanning
- Each `pyproject.toml` with `[project] name` → workspace member
- Editable installs (`-e .` in requirements.txt) may reference siblings
- Resolution: if import matches a sibling package name → resolve to that package's source root

### No Workspace → No Problem

If no workspace indicators found, Ariadne behaves as today — single project root, simple resolution. Workspace detection is additive.

### Scope: Phase 1 vs Later

**Phase 1:** Basic workspace detection for TypeScript/JavaScript (most common case — `package.json` workspaces). Store detected workspace info but don't fail if parsing workspace config fails (W008 fallback).

**Later:** Full Go workspace, Cargo workspace, Nx/Turbo support. These can be added incrementally per language — the `WorkspaceInfo` abstraction supports it.

## Import Resolution Pipeline

Putting it all together — the full resolution flow:

```
resolve_import(import_path, source_file, project_root, workspace_info):

  1. CLASSIFY the import:
     - Starts with './' or '../' → RELATIVE
     - Starts with workspace package name → WORKSPACE
     - Matches known standard library → STDLIB → skip
     - Everything else → EXTERNAL → skip

  2. RESOLVE based on classification:

     RELATIVE:
       a. Join source_file directory + import_path
       b. Normalize (remove .., ., collapse //)
       c. Try extensions: [.ts, .tsx, .js, .jsx, .mjs, .cjs] (per language)
       d. Try index files: [index.ts, index.tsx, index.js, index.jsx] (for directory imports)
       e. Case-insensitive fallback if on case-insensitive FS
       f. Match against walked file set → canonical path or None

     WORKSPACE:
       a. Look up package name in workspace member map
       b. Resolve to member's entry point
       c. If import has a subpath (@myapp/auth/utils) → resolve within member directory
       d. Match against walked file set → canonical path or None

     STDLIB / EXTERNAL:
       → None (no edge created)

  3. VALIDATE:
     - Resolved path exists in walked file set?
     - Resolved path is within project root? (security: no path traversal)
     - Resolved path is not the source file itself? (no self-import)

  4. RETURN:
     - Some(canonical_path) → edge created
     - None → unresolved (W006 in verbose mode)
```

### Path Traversal Protection

Import paths like `../../../../../../../etc/passwd` must NEVER resolve outside the project root:

```rust
fn validate_resolved_path(resolved: &Path, project_root: &Path) -> bool {
    // Canonical resolved path must start with canonical project root
    resolved.starts_with(project_root)
}
```

This is checked in step 3 (VALIDATE). If a resolved path escapes the project root, it's treated as unresolved — no edge created, no warning (it's an external reference, same as any other unresolvable path).

## Testing

### Path Normalization Tests

- `./src/auth/login.ts` → `src/auth/login.ts`
- `src/auth/../utils/format.ts` → `src/utils/format.ts`
- `src/auth/./login.ts` → `src/auth/login.ts`
- `src//auth//login.ts` → `src/auth/login.ts`
- Backslash paths on Windows
- Trailing slashes stripped
- Leading `./` stripped

### Case Sensitivity Tests

- Import `'./Utils'` with file `utils.ts` on case-insensitive FS → resolves
- Import `'./Utils'` with file `utils.ts` on case-sensitive FS → does NOT resolve
- Graph uses canonical (filesystem) casing in both cases

### Workspace Resolution Tests

- npm workspace: `import "@myapp/auth"` → resolves to `packages/auth/src/index.ts`
- Workspace subpath: `import "@myapp/auth/utils"` → resolves to `packages/auth/src/utils.ts`
- Non-workspace scoped package: `import "@types/react"` → does NOT resolve (external)
- Fixture: `tests/fixtures/workspace-project/` — npm workspace with 3 packages

### Path Traversal Tests

- `import '../../../etc/passwd'` → does NOT resolve (outside project root)
- `import '../../sibling-project/file'` → does NOT resolve (outside project root)
