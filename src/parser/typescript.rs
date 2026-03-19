use crate::model::workspace::WorkspaceInfo;
use crate::model::{CanonicalPath, FileSet};
use crate::parser::traits::{ImportKind, ImportResolver, LanguageParser, RawExport, RawImport};

/// Parser and resolver for TypeScript and JavaScript files.
/// Uses tree-sitter-typescript grammar for all extensions (.ts, .tsx, .js, .jsx, .mjs, .cjs).
pub(crate) struct TypeScriptParser;

impl TypeScriptParser {
    pub fn new() -> Self {
        Self
    }

    /// Extract the string content from a string node, stripping quotes.
    fn string_content<'a>(node: &tree_sitter::Node<'a>, source: &'a [u8]) -> Option<&'a str> {
        let text = node.utf8_text(source).ok()?;
        // Strip surrounding quotes (single, double, or backtick)
        if text.len() >= 2 {
            let first = text.as_bytes()[0];
            let last = text.as_bytes()[text.len() - 1];
            if (first == b'"' || first == b'\'' || first == b'`') && first == last {
                return Some(&text[1..text.len() - 1]);
            }
        }
        Some(text)
    }

    /// Find the first descendant of a given kind.
    fn find_child_by_kind<'a>(
        node: &tree_sitter::Node<'a>,
        kind: &str,
    ) -> Option<tree_sitter::Node<'a>> {
        let mut cursor = node.walk();
        let result = node
            .children(&mut cursor)
            .find(|child| child.kind() == kind);
        result
    }

    /// Extract named import symbols from an import clause.
    fn extract_named_symbols(node: &tree_sitter::Node, source: &[u8]) -> Vec<String> {
        let mut symbols = Vec::new();
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "named_imports" {
                let mut inner_cursor = child.walk();
                for spec in child.children(&mut inner_cursor) {
                    if spec.kind() == "import_specifier" {
                        // The imported name is the first identifier child
                        let mut spec_cursor = spec.walk();
                        for spec_child in spec.children(&mut spec_cursor) {
                            if spec_child.kind() == "identifier"
                                || spec_child.kind() == "type_identifier"
                            {
                                if let Ok(name) = spec_child.utf8_text(source) {
                                    symbols.push(name.to_string());
                                }
                                break;
                            }
                        }
                    }
                }
            } else if child.kind() == "identifier" {
                // Default import
                if let Ok(name) = child.utf8_text(source) {
                    symbols.push(name.to_string());
                }
            } else if child.kind() == "namespace_import" {
                // import * as name
                let mut ns_cursor = child.walk();
                for ns_child in child.children(&mut ns_cursor) {
                    if ns_child.kind() == "identifier" {
                        if let Ok(name) = ns_child.utf8_text(source) {
                            symbols.push(format!("* as {}", name));
                        }
                        break;
                    }
                }
            }
        }
        symbols
    }

    /// Check if an import statement is type-only.
    fn is_type_import(node: &tree_sitter::Node, source: &[u8]) -> bool {
        // Check for `import type` — the node text starts with "import type"
        // In tree-sitter-typescript, type-only imports have "type" as a child token
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "type" {
                return true;
            }
            // Also check for the literal text "type" right after "import"
            if child.kind() == "import" {
                continue;
            }
            if let Ok(text) = child.utf8_text(source) {
                if text == "type" {
                    return true;
                }
            }
            // Stop checking after we've passed the keyword area
            if child.kind() == "import_clause"
                || child.kind() == "string"
                || child.kind() == "named_imports"
            {
                break;
            }
        }
        false
    }
}

impl LanguageParser for TypeScriptParser {
    fn language(&self) -> &str {
        "typescript"
    }

    fn extensions(&self) -> &[&str] {
        &["ts", "tsx", "js", "jsx", "mjs", "cjs"]
    }

    fn tree_sitter_language(&self) -> tree_sitter::Language {
        tree_sitter::Language::from(tree_sitter_typescript::LANGUAGE_TYPESCRIPT)
    }

    fn extract_imports(&self, tree: &tree_sitter::Tree, source: &[u8]) -> Vec<RawImport> {
        let mut imports = Vec::new();
        let root = tree.root_node();
        let mut cursor = root.walk();

        for node in root.children(&mut cursor) {
            match node.kind() {
                // Handles: import { foo } from './bar'
                //          import foo from './bar'
                //          import * as foo from './bar'
                //          import './bar'  (side-effect)
                //          import type { Foo } from './bar'
                "import_statement" => {
                    let is_type_only = Self::is_type_import(&node, source);

                    // Find the source string (the from path)
                    if let Some(source_node) = Self::find_child_by_kind(&node, "string") {
                        if let Some(path) = Self::string_content(&source_node, source) {
                            // Extract symbols from import clause
                            let symbols = if let Some(clause) =
                                Self::find_child_by_kind(&node, "import_clause")
                            {
                                Self::extract_named_symbols(&clause, source)
                            } else {
                                Vec::new()
                            };

                            imports.push(RawImport {
                                path: path.to_string(),
                                symbols,
                                is_type_only,
                                kind: ImportKind::Regular,
                            });
                        }
                    }
                }

                // Handles: expression_statement containing require() or dynamic import()
                "expression_statement" => {
                    Self::extract_require_or_dynamic_import(&node, source, &mut imports);
                }

                // Handles: variable declarations with require() — const foo = require('./bar')
                "lexical_declaration" | "variable_declaration" => {
                    Self::extract_require_from_declaration(&node, source, &mut imports);
                }

                _ => {}
            }
        }

        imports
    }

    fn extract_exports(&self, tree: &tree_sitter::Tree, source: &[u8]) -> Vec<RawExport> {
        let mut exports = Vec::new();
        let root = tree.root_node();
        let mut cursor = root.walk();

        for node in root.children(&mut cursor) {
            // export default function() {} / export default class Foo {}
            // export function foo() {} / export class Foo {}
            // export const foo = ...
            if node.kind() == "export_statement" {
                let mut has_source = false;
                let mut source_path = None;
                let mut is_default = false;
                let mut exported_names = Vec::new();

                let mut child_cursor = node.walk();
                for child in node.children(&mut child_cursor) {
                    match child.kind() {
                        "default" => {
                            is_default = true;
                        }
                        "export_clause" => {
                            // export { foo, bar } or export { foo } from './bar'
                            let mut clause_cursor = child.walk();
                            for spec in child.children(&mut clause_cursor) {
                                if spec.kind() == "export_specifier" {
                                    let mut spec_cursor = spec.walk();
                                    for spec_child in spec.children(&mut spec_cursor) {
                                        if spec_child.kind() == "identifier"
                                            || spec_child.kind() == "type_identifier"
                                        {
                                            if let Ok(name) = spec_child.utf8_text(source) {
                                                exported_names.push(name.to_string());
                                            }
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                        "string" => {
                            // from './bar'
                            if let Some(path) = Self::string_content(&child, source) {
                                has_source = true;
                                source_path = Some(path.to_string());
                            }
                        }
                        "function_declaration" | "generator_function_declaration" => {
                            // export function foo() {}
                            if let Some(name_node) = Self::find_child_by_kind(&child, "identifier")
                            {
                                if let Ok(name) = name_node.utf8_text(source) {
                                    exported_names.push(name.to_string());
                                }
                            }
                        }
                        "class_declaration" => {
                            if let Some(name_node) =
                                Self::find_child_by_kind(&child, "type_identifier")
                            {
                                if let Ok(name) = name_node.utf8_text(source) {
                                    exported_names.push(name.to_string());
                                }
                            }
                        }
                        "lexical_declaration" | "variable_declaration" => {
                            // export const foo = ..., bar = ...
                            let mut decl_cursor = child.walk();
                            for declarator in child.children(&mut decl_cursor) {
                                if declarator.kind() == "variable_declarator" {
                                    if let Some(name_node) =
                                        Self::find_child_by_kind(&declarator, "identifier")
                                    {
                                        if let Ok(name) = name_node.utf8_text(source) {
                                            exported_names.push(name.to_string());
                                        }
                                    }
                                }
                            }
                        }
                        "interface_declaration" | "type_alias_declaration" => {
                            if let Some(name_node) =
                                Self::find_child_by_kind(&child, "type_identifier")
                            {
                                if let Ok(name) = name_node.utf8_text(source) {
                                    exported_names.push(name.to_string());
                                }
                            }
                        }
                        "enum_declaration" => {
                            if let Some(name_node) = Self::find_child_by_kind(&child, "identifier")
                            {
                                if let Ok(name) = name_node.utf8_text(source) {
                                    exported_names.push(name.to_string());
                                }
                            }
                        }
                        "*" => {
                            // export * from './bar'
                            exported_names.push("*".to_string());
                        }
                        "namespace_export" => {
                            // export * as ns from './bar'
                            exported_names.push("*".to_string());
                        }
                        _ => {}
                    }
                }

                if is_default {
                    exports.push(RawExport {
                        name: "default".to_string(),
                        is_re_export: has_source,
                        source: source_path.clone(),
                    });
                } else if exported_names.is_empty() && !is_default {
                    // Side-effect only export or unrecognized pattern
                } else {
                    for name in exported_names {
                        exports.push(RawExport {
                            name,
                            is_re_export: has_source,
                            source: source_path.clone(),
                        });
                    }
                }
            }
        }

        exports
    }
}

impl TypeScriptParser {
    /// Recursively search for require() calls and dynamic import() in a node.
    fn extract_require_or_dynamic_import(
        node: &tree_sitter::Node,
        source: &[u8],
        imports: &mut Vec<RawImport>,
    ) {
        let mut stack = vec![*node];
        while let Some(current) = stack.pop() {
            match current.kind() {
                "call_expression" => {
                    // Check if it's require('...')
                    if let Some(func) = Self::find_child_by_kind(&current, "identifier") {
                        if func.utf8_text(source).ok() == Some("require") {
                            if let Some(args) = Self::find_child_by_kind(&current, "arguments") {
                                let mut args_cursor = args.walk();
                                for arg in args.children(&mut args_cursor) {
                                    if arg.kind() == "string" {
                                        if let Some(path) = Self::string_content(&arg, source) {
                                            imports.push(RawImport {
                                                path: path.to_string(),
                                                symbols: Vec::new(),
                                                is_type_only: false,
                                                kind: ImportKind::Regular,
                                            });
                                        }
                                    }
                                }
                            }
                        }
                    }
                    // Check if it's import('...')  — dynamic import
                    if let Some(func) = Self::find_child_by_kind(&current, "import") {
                        let _ = func; // just checking existence
                        if let Some(args) = Self::find_child_by_kind(&current, "arguments") {
                            let mut args_cursor = args.walk();
                            for arg in args.children(&mut args_cursor) {
                                if arg.kind() == "string" {
                                    if let Some(path) = Self::string_content(&arg, source) {
                                        imports.push(RawImport {
                                            path: path.to_string(),
                                            symbols: Vec::new(),
                                            is_type_only: false,
                                            kind: ImportKind::Regular,
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
                _ => {
                    let mut cursor = current.walk();
                    for child in current.children(&mut cursor) {
                        stack.push(child);
                    }
                }
            }
        }
    }

    /// Extract require() calls from variable declarations.
    fn extract_require_from_declaration(
        node: &tree_sitter::Node,
        source: &[u8],
        imports: &mut Vec<RawImport>,
    ) {
        Self::extract_require_or_dynamic_import(node, source, imports);
    }
}

/// TypeScript/JavaScript import resolver.
pub(crate) struct TypeScriptResolver;

impl TypeScriptResolver {
    pub fn new() -> Self {
        Self
    }

    /// Check if a specifier is a bare module specifier (not relative).
    fn is_bare_specifier(path: &str) -> bool {
        !path.starts_with('.') && !path.starts_with('/')
    }
}

impl TypeScriptResolver {
    /// Try to resolve an import as a workspace package reference.
    /// Handles both direct package imports (`@myapp/auth`) and
    /// subpath imports (`@myapp/auth/utils`).
    fn resolve_workspace_import(
        &self,
        import_path: &str,
        _from_file: &CanonicalPath,
        known_files: &FileSet,
        workspace: &WorkspaceInfo,
    ) -> Option<CanonicalPath> {
        for member in &workspace.members {
            if import_path == member.name {
                // Direct package import -> resolve to entry point
                let entry = member.entry_point.to_string_lossy();
                // Convert to forward slashes, strip leading ./
                let entry_canonical = entry.replace('\\', "/");
                let entry_canonical = entry_canonical
                    .strip_prefix("./")
                    .unwrap_or(&entry_canonical);
                let candidate = CanonicalPath::new(entry_canonical);
                if known_files.contains(&candidate) {
                    return Some(candidate);
                }
                // Also probe with extensions if entry point has no extension
                let base_str = candidate.as_str();
                let extensions = &["ts", "tsx", "js", "jsx", "mjs", "cjs"];
                for ext in extensions {
                    let with_ext = CanonicalPath::new(format!("{}.{}", base_str, ext));
                    if known_files.contains(&with_ext) {
                        return Some(with_ext);
                    }
                }
                // Try index file in entry point directory
                let index_extensions = &["ts", "tsx", "js", "jsx"];
                for ext in index_extensions {
                    let index = CanonicalPath::new(format!("{}/index.{}", base_str, ext));
                    if known_files.contains(&index) {
                        return Some(index);
                    }
                }
            } else if import_path.starts_with(&format!("{}/", member.name)) {
                // Subpath import: @myapp/auth/utils -> strip package name, resolve within member dir
                let subpath = &import_path[member.name.len() + 1..];
                let member_dir = member.path.to_string_lossy();
                let member_dir = member_dir.replace('\\', "/");
                let member_dir = member_dir.strip_prefix("./").unwrap_or(&member_dir);
                let base = format!("{}/{}", member_dir, subpath);
                let base_canonical = CanonicalPath::new(&base);
                let base_str = base_canonical.as_str();

                // Exact match
                if known_files.contains(&base_canonical) {
                    return Some(base_canonical);
                }

                // Extension probing
                let extensions = &["ts", "tsx", "js", "jsx", "mjs", "cjs"];
                for ext in extensions {
                    let candidate = CanonicalPath::new(format!("{}.{}", base_str, ext));
                    if known_files.contains(&candidate) {
                        return Some(candidate);
                    }
                }

                // Index file probing
                let index_extensions = &["ts", "tsx", "js", "jsx"];
                for ext in index_extensions {
                    let candidate = CanonicalPath::new(format!("{}/index.{}", base_str, ext));
                    if known_files.contains(&candidate) {
                        return Some(candidate);
                    }
                }
            }
        }
        None
    }
}

impl ImportResolver for TypeScriptResolver {
    fn resolve(
        &self,
        import: &RawImport,
        from_file: &CanonicalPath,
        known_files: &FileSet,
        workspace: Option<&WorkspaceInfo>,
    ) -> Option<CanonicalPath> {
        let specifier = &import.path;

        // Skip empty imports
        if specifier.is_empty() {
            return None;
        }

        // Check workspace packages first (before relative/external classification)
        if let Some(ws) = workspace {
            if let Some(resolved) =
                self.resolve_workspace_import(specifier, from_file, known_files, ws)
            {
                return Some(resolved);
            }
        }

        // Skip bare specifiers (npm packages) and scoped packages
        if Self::is_bare_specifier(specifier) {
            return None;
        }

        // Get the directory of the source file
        let base_dir = from_file.parent().unwrap_or("");

        // Join the relative path with the base directory
        let joined = if base_dir.is_empty() {
            specifier.to_string()
        } else {
            format!("{}/{}", base_dir, specifier)
        };

        let candidate_base = CanonicalPath::new(&joined);
        let base_str = candidate_base.as_str();

        // 1. Try exact match (file already has extension)
        let exact = CanonicalPath::new(base_str);
        if known_files.contains(&exact) {
            return Some(exact);
        }

        // 2. Extension probing
        let extensions = &["ts", "tsx", "js", "jsx", "mjs", "cjs"];
        for ext in extensions {
            let candidate = CanonicalPath::new(format!("{}.{}", base_str, ext));
            if known_files.contains(&candidate) {
                return Some(candidate);
            }
        }

        // 3. Index file probing (directory import)
        let index_extensions = &["ts", "tsx", "js", "jsx"];
        for ext in index_extensions {
            let candidate = CanonicalPath::new(format!("{}/index.{}", base_str, ext));
            if known_files.contains(&candidate) {
                return Some(candidate);
            }
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::workspace::{WorkspaceKind, WorkspaceMember};
    use crate::parser::traits::ImportKind;
    use std::path::PathBuf;

    fn make_import(path: &str) -> RawImport {
        RawImport {
            path: path.to_string(),
            symbols: Vec::new(),
            is_type_only: false,
            kind: ImportKind::Regular,
        }
    }

    fn make_workspace() -> WorkspaceInfo {
        WorkspaceInfo {
            kind: WorkspaceKind::Npm,
            members: vec![
                WorkspaceMember {
                    name: "@myapp/auth".to_string(),
                    path: PathBuf::from("packages/auth"),
                    entry_point: PathBuf::from("packages/auth/src/index.ts"),
                },
                WorkspaceMember {
                    name: "@myapp/utils".to_string(),
                    path: PathBuf::from("packages/utils"),
                    entry_point: PathBuf::from("packages/utils/src/index.ts"),
                },
            ],
        }
    }

    fn make_files(paths: &[&str]) -> FileSet {
        FileSet::from_iter(paths.iter().map(|p| CanonicalPath::new(*p)))
    }

    #[test]
    fn workspace_direct_package_import() {
        let resolver = TypeScriptResolver::new();
        let ws = make_workspace();
        let files = make_files(&[
            "packages/auth/src/index.ts",
            "packages/utils/src/index.ts",
            "apps/web/src/app.ts",
        ]);
        let from = CanonicalPath::new("apps/web/src/app.ts");
        let import = make_import("@myapp/auth");

        let result = resolver.resolve(&import, &from, &files, Some(&ws));
        assert_eq!(result.unwrap().as_str(), "packages/auth/src/index.ts");
    }

    #[test]
    fn workspace_subpath_import() {
        let resolver = TypeScriptResolver::new();
        let ws = make_workspace();
        let files = make_files(&[
            "packages/auth/src/index.ts",
            "packages/auth/utils.ts",
            "apps/web/src/app.ts",
        ]);
        let from = CanonicalPath::new("apps/web/src/app.ts");
        let import = make_import("@myapp/auth/utils");

        let result = resolver.resolve(&import, &from, &files, Some(&ws));
        assert_eq!(result.unwrap().as_str(), "packages/auth/utils.ts");
    }

    #[test]
    fn workspace_subpath_import_with_extension_probing() {
        let resolver = TypeScriptResolver::new();
        let ws = make_workspace();
        let files = make_files(&[
            "packages/auth/src/index.ts",
            "packages/auth/helpers/validate.ts",
            "apps/web/src/app.ts",
        ]);
        let from = CanonicalPath::new("apps/web/src/app.ts");
        let import = make_import("@myapp/auth/helpers/validate");

        let result = resolver.resolve(&import, &from, &files, Some(&ws));
        assert_eq!(
            result.unwrap().as_str(),
            "packages/auth/helpers/validate.ts"
        );
    }

    #[test]
    fn non_workspace_scoped_package_returns_none() {
        let resolver = TypeScriptResolver::new();
        let ws = make_workspace();
        let files = make_files(&["apps/web/src/app.ts"]);
        let from = CanonicalPath::new("apps/web/src/app.ts");
        let import = make_import("@types/react");

        let result = resolver.resolve(&import, &from, &files, Some(&ws));
        assert!(result.is_none());
    }

    #[test]
    fn bare_specifier_returns_none_with_workspace() {
        let resolver = TypeScriptResolver::new();
        let ws = make_workspace();
        let files = make_files(&["apps/web/src/app.ts"]);
        let from = CanonicalPath::new("apps/web/src/app.ts");
        let import = make_import("lodash");

        let result = resolver.resolve(&import, &from, &files, Some(&ws));
        assert!(result.is_none());
    }

    #[test]
    fn relative_import_unchanged_with_workspace() {
        let resolver = TypeScriptResolver::new();
        let ws = make_workspace();
        let files = make_files(&["apps/web/src/app.ts", "apps/web/src/utils.ts"]);
        let from = CanonicalPath::new("apps/web/src/app.ts");
        let import = make_import("./utils");

        let result = resolver.resolve(&import, &from, &files, Some(&ws));
        assert_eq!(result.unwrap().as_str(), "apps/web/src/utils.ts");
    }

    #[test]
    fn workspace_none_keeps_existing_behavior() {
        let resolver = TypeScriptResolver::new();
        let files = make_files(&["src/app.ts", "src/utils.ts"]);
        let from = CanonicalPath::new("src/app.ts");
        let import = make_import("./utils");

        let result = resolver.resolve(&import, &from, &files, None);
        assert_eq!(result.unwrap().as_str(), "src/utils.ts");
    }

    #[test]
    fn workspace_none_bare_specifier_returns_none() {
        let resolver = TypeScriptResolver::new();
        let files = make_files(&["src/app.ts"]);
        let from = CanonicalPath::new("src/app.ts");
        let import = make_import("lodash");

        let result = resolver.resolve(&import, &from, &files, None);
        assert!(result.is_none());
    }

    #[test]
    fn workspace_subpath_index_file() {
        let resolver = TypeScriptResolver::new();
        let ws = make_workspace();
        let files = make_files(&[
            "packages/auth/src/index.ts",
            "packages/auth/helpers/index.ts",
            "apps/web/src/app.ts",
        ]);
        let from = CanonicalPath::new("apps/web/src/app.ts");
        let import = make_import("@myapp/auth/helpers");

        let result = resolver.resolve(&import, &from, &files, Some(&ws));
        assert_eq!(result.unwrap().as_str(), "packages/auth/helpers/index.ts");
    }
}
