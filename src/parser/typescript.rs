use crate::model::workspace::WorkspaceInfo;
use crate::model::{CanonicalPath, FileSet};
use crate::parser::helpers;
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

    fn tree_sitter_language_for_ext(&self, ext: &str) -> tree_sitter::Language {
        match ext {
            "tsx" | "jsx" => tree_sitter::Language::from(tree_sitter_typescript::LANGUAGE_TSX),
            _ => tree_sitter::Language::from(tree_sitter_typescript::LANGUAGE_TYPESCRIPT),
        }
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
                    if let Some(source_node) = helpers::find_child_by_kind(&node, "string") {
                        if let Some(path) = Self::string_content(&source_node, source) {
                            // Extract symbols from import clause
                            let symbols = if let Some(clause) =
                                helpers::find_child_by_kind(&node, "import_clause")
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
                            if let Some(name_node) =
                                helpers::find_child_by_kind(&child, "identifier")
                            {
                                if let Ok(name) = name_node.utf8_text(source) {
                                    exported_names.push(name.to_string());
                                }
                            }
                        }
                        "class_declaration" => {
                            if let Some(name_node) =
                                helpers::find_child_by_kind(&child, "type_identifier")
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
                                        helpers::find_child_by_kind(&declarator, "identifier")
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
                                helpers::find_child_by_kind(&child, "type_identifier")
                            {
                                if let Ok(name) = name_node.utf8_text(source) {
                                    exported_names.push(name.to_string());
                                }
                            }
                        }
                        "enum_declaration" => {
                            if let Some(name_node) =
                                helpers::find_child_by_kind(&child, "identifier")
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
                    if let Some(func) = helpers::find_child_by_kind(&current, "identifier") {
                        if func.utf8_text(source).ok() == Some("require") {
                            if let Some(args) = helpers::find_child_by_kind(&current, "arguments") {
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
                    if let Some(func) = helpers::find_child_by_kind(&current, "import") {
                        let _ = func; // just checking existence
                        if let Some(args) = helpers::find_child_by_kind(&current, "arguments") {
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
    use crate::parser::traits::{ImportKind, LanguageParser};
    use std::path::PathBuf;

    fn parse_ts(source: &str) -> tree_sitter::Tree {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter::Language::from(
                tree_sitter_typescript::LANGUAGE_TYPESCRIPT,
            ))
            .unwrap();
        parser.parse(source, None).unwrap()
    }

    fn parse_tsx(source: &str) -> tree_sitter::Tree {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter::Language::from(
                tree_sitter_typescript::LANGUAGE_TSX,
            ))
            .unwrap();
        parser.parse(source, None).unwrap()
    }

    fn ts_imports(source: &str) -> Vec<RawImport> {
        let tree = parse_ts(source);
        TypeScriptParser::new().extract_imports(&tree, source.as_bytes())
    }

    fn ts_exports(source: &str) -> Vec<RawExport> {
        let tree = parse_ts(source);
        TypeScriptParser::new().extract_exports(&tree, source.as_bytes())
    }

    // ---- Import tests ----

    #[test]
    fn ts_import_named() {
        let result = ts_imports("import { foo, bar } from './mod';");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].path, "./mod");
        assert!(result[0].symbols.contains(&"foo".to_string()));
        assert!(result[0].symbols.contains(&"bar".to_string()));
    }

    #[test]
    fn ts_import_default() {
        let result = ts_imports("import React from 'react';");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].path, "react");
        assert!(result[0].symbols.contains(&"React".to_string()));
    }

    #[test]
    fn ts_import_namespace() {
        let result = ts_imports("import * as utils from './utils';");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].path, "./utils");
        assert!(result[0].symbols.iter().any(|s| s.contains("utils")));
    }

    #[test]
    fn ts_import_side_effect() {
        let result = ts_imports("import './side-effect';");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].path, "./side-effect");
        assert!(result[0].symbols.is_empty());
    }

    #[test]
    fn ts_require_call() {
        let result = ts_imports("const fs = require('fs');");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].path, "fs");
    }

    #[test]
    fn ts_import_type_only() {
        let result = ts_imports("import type { Foo } from './types';");
        assert_eq!(result.len(), 1);
        assert!(result[0].is_type_only);
        assert_eq!(result[0].path, "./types");
    }

    #[test]
    fn ts_empty_source_no_imports() {
        let result = ts_imports("");
        assert!(result.is_empty());
    }

    #[test]
    fn ts_malformed_no_crash() {
        let result = ts_imports("import from;");
        let _ = result; // Should not panic
    }

    // ---- Export tests ----

    #[test]
    fn ts_export_named() {
        let result = ts_exports("export { foo, bar };");
        assert_eq!(result.len(), 2);
        let names: Vec<&str> = result.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"foo"));
        assert!(names.contains(&"bar"));
        assert!(!result[0].is_re_export);
    }

    #[test]
    fn ts_export_re_export() {
        let result = ts_exports("export { foo } from './other';");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "foo");
        assert!(result[0].is_re_export);
        assert_eq!(result[0].source.as_deref(), Some("./other"));
    }

    #[test]
    fn ts_export_star() {
        let result = ts_exports("export * from './barrel';");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "*");
        assert!(result[0].is_re_export);
    }

    #[test]
    fn ts_export_function() {
        let result = ts_exports("export function hello() {}");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "hello");
    }

    #[test]
    fn ts_export_default() {
        let result = ts_exports("export default function main() {}");
        assert!(result
            .iter()
            .any(|e| e.name == "default" || e.name == "main"));
    }

    #[test]
    fn ts_export_const() {
        let result = ts_exports("export const FOO = 42;");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "FOO");
    }

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

    // ---- TSX-specific tests (requires LANGUAGE_TSX grammar) ----

    #[test]
    fn tsx_implicit_jsx_return_inline() {
        // Bug #1: arrow function with implicit JSX return on same line
        let source = r#"
import React from 'react';
export const A = () => <div>hello</div>;
"#;
        let tree = parse_tsx(source);
        let root = tree.root_node();
        // Should parse without ERROR nodes
        assert!(!root.has_error(), "TSX implicit JSX return should parse cleanly");
        let exports = TypeScriptParser::new().extract_exports(&tree, source.as_bytes());
        assert!(exports.iter().any(|e| e.name == "A"));
    }

    #[test]
    fn tsx_implicit_jsx_return_parens_inline() {
        // Bug #1 variant: => (<JSX>) on one line
        let source = r#"
import React from 'react';
export const A = () => (<div>hello</div>);
"#;
        let tree = parse_tsx(source);
        let root = tree.root_node();
        assert!(!root.has_error(), "TSX parens JSX return should parse cleanly");
        let exports = TypeScriptParser::new().extract_exports(&tree, source.as_bytes());
        assert!(exports.iter().any(|e| e.name == "A"));
    }

    #[test]
    fn tsx_double_braces_with_text_content() {
        // Bug #2: {{ }} in JSX prop + text content on same line
        let source = r#"
import React from 'react';
export const A = () => <div style={{ color: 'red' }}>text</div>;
"#;
        let tree = parse_tsx(source);
        let root = tree.root_node();
        assert!(!root.has_error(), "TSX double braces with text should parse cleanly");
        let imports = TypeScriptParser::new().extract_imports(&tree, source.as_bytes());
        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].path, "react");
    }

    #[test]
    fn tsx_double_braces_with_expression_content() {
        // Bug #2 variant: {{ }} + {expression} on same line
        let source = r#"
import React from 'react';
export const A = () => <div style={{ color: 'red' }}>{42}</div>;
"#;
        let tree = parse_tsx(source);
        let root = tree.root_node();
        assert!(!root.has_error(), "TSX double braces with expression should parse cleanly");
        let imports = TypeScriptParser::new().extract_imports(&tree, source.as_bytes());
        assert_eq!(imports.len(), 1);
    }

    #[test]
    fn tsx_language_for_ext() {
        let parser = TypeScriptParser::new();
        // TSX/JSX extensions should get the TSX grammar
        let tsx_lang = parser.tree_sitter_language_for_ext("tsx");
        let jsx_lang = parser.tree_sitter_language_for_ext("jsx");
        let ts_lang = parser.tree_sitter_language_for_ext("ts");
        // TSX and JSX should use same grammar, different from TS
        assert_eq!(tsx_lang, jsx_lang);
        assert_ne!(ts_lang, tsx_lang);
    }

    // ---- Generic arrow functions ----

    #[test]
    fn tsx_generic_arrow_trailing_comma() {
        // <T,> disambiguates from JSX in TSX files
        let source = r#"
import React from 'react';
export const Box = <T,>(props: { value: T }) => <div>{String(props.value)}</div>;
"#;
        let tree = parse_tsx(source);
        assert!(!tree.root_node().has_error(), "generic <T,> arrow should parse cleanly");
        let exports = TypeScriptParser::new().extract_exports(&tree, source.as_bytes());
        assert!(exports.iter().any(|e| e.name == "Box"));
    }

    #[test]
    fn tsx_generic_multiple_params() {
        let source = r#"
import React from 'react';
export const Pair = <A, B>({ a, b }: { a: A; b: B }) => <span>{String(a)}</span>;
"#;
        let tree = parse_tsx(source);
        assert!(!tree.root_node().has_error(), "generic <A, B> arrow should parse cleanly");
        let exports = TypeScriptParser::new().extract_exports(&tree, source.as_bytes());
        assert!(exports.iter().any(|e| e.name == "Pair"));
    }

    // ---- JSX fragments ----

    #[test]
    fn tsx_fragment() {
        let source = r#"
import React from 'react';
export const Frag = () => (<><span>a</span><span>b</span></>);
"#;
        let tree = parse_tsx(source);
        assert!(!tree.root_node().has_error(), "JSX fragment should parse cleanly");
        let exports = TypeScriptParser::new().extract_exports(&tree, source.as_bytes());
        assert!(exports.iter().any(|e| e.name == "Frag"));
    }

    #[test]
    fn tsx_inline_fragment() {
        let source = r#"
import React from 'react';
export const F = () => <><span>x</span></>;
"#;
        let tree = parse_tsx(source);
        assert!(!tree.root_node().has_error(), "inline fragment should parse cleanly");
    }

    // ---- JSX spread attributes ----

    #[test]
    fn tsx_spread_props() {
        let source = r#"
import React from 'react';
export const S = (props: any) => <div {...props}>spread</div>;
"#;
        let tree = parse_tsx(source);
        assert!(!tree.root_node().has_error(), "spread props should parse cleanly");
        let exports = TypeScriptParser::new().extract_exports(&tree, source.as_bytes());
        assert!(exports.iter().any(|e| e.name == "S"));
    }

    #[test]
    fn tsx_spread_with_inline_style() {
        let source = r#"
import React from 'react';
export const M = (p: any) => <div {...p} style={{ margin: 0 }}>mixed</div>;
"#;
        let tree = parse_tsx(source);
        assert!(!tree.root_node().has_error(), "spread + inline style should parse cleanly");
    }

    // ---- Conditional rendering ----

    #[test]
    fn tsx_conditional_and() {
        let source = r#"
import React from 'react';
export const C = ({ show }: { show: boolean }) => <div>{show && <span>yes</span>}</div>;
"#;
        let tree = parse_tsx(source);
        assert!(!tree.root_node().has_error(), "conditional && should parse cleanly");
    }

    #[test]
    fn tsx_ternary() {
        let source = r#"
import React from 'react';
export const T = ({ ok }: { ok: boolean }) => <div>{ok ? <span>y</span> : <span>n</span>}</div>;
"#;
        let tree = parse_tsx(source);
        assert!(!tree.root_node().has_error(), "ternary in JSX should parse cleanly");
    }

    // ---- .map with arrow returning JSX ----

    #[test]
    fn tsx_map_arrow_jsx() {
        let source = r#"
import React from 'react';
export const L = ({ items }: { items: string[] }) => (
  <ul>{items.map(item => <li key={item}>{item}</li>)}</ul>
);
"#;
        let tree = parse_tsx(source);
        assert!(!tree.root_node().has_error(), "map with arrow JSX should parse cleanly");
        let exports = TypeScriptParser::new().extract_exports(&tree, source.as_bytes());
        assert!(exports.iter().any(|e| e.name == "L"));
    }

    #[test]
    fn tsx_map_with_index_and_inline_style() {
        let source = r#"
import React from 'react';
export const IL = ({ items }: { items: string[] }) => (
  <ol>
    {items.map((item, i) => (
      <li key={i} style={{ fontWeight: i === 0 ? 'bold' : 'normal' }}>{item}</li>
    ))}
  </ol>
);
"#;
        let tree = parse_tsx(source);
        assert!(!tree.root_node().has_error(), "map with index + inline style should parse cleanly");
    }

    // ---- Anonymous default export ----

    #[test]
    fn tsx_anonymous_default_export() {
        let source = r#"
import React from 'react';
export default () => <div>anonymous</div>;
"#;
        let tree = parse_tsx(source);
        assert!(!tree.root_node().has_error(), "anonymous default export should parse cleanly");
        let exports = TypeScriptParser::new().extract_exports(&tree, source.as_bytes());
        assert!(
            exports.iter().any(|e| e.name == "default"),
            "should have default export; got: {:?}",
            exports.iter().map(|e| &e.name).collect::<Vec<_>>()
        );
    }

    // ---- Callback props ----

    #[test]
    fn tsx_multiline_callback_prop() {
        let source = r#"
import React from 'react';
export const CB = () => (
  <button
    onClick={() => {
      console.log('clicked');
    }}
  >
    click
  </button>
);
"#;
        let tree = parse_tsx(source);
        assert!(!tree.root_node().has_error(), "multiline callback prop should parse cleanly");
    }

    #[test]
    fn tsx_inline_callback_prop() {
        let source = r#"
import React from 'react';
export const ICB = () => <button onClick={() => console.log('hi')}>go</button>;
"#;
        let tree = parse_tsx(source);
        assert!(!tree.root_node().has_error(), "inline callback prop should parse cleanly");
    }

    // ---- .jsx extension via reparse ----

    #[test]
    fn jsx_extension_parses_with_tsx_grammar() {
        let source = r#"
import React from 'react';
export const Btn = ({ label }) => <button style={{ padding: '8px' }}>{label}</button>;
"#;
        // Simulate .jsx going through tree_sitter_language_for_ext
        let parser = TypeScriptParser::new();
        let lang = parser.tree_sitter_language_for_ext("jsx");
        let mut ts_parser = tree_sitter::Parser::new();
        ts_parser.set_language(&lang).unwrap();
        let tree = ts_parser.parse(source, None).unwrap();
        assert!(!tree.root_node().has_error(), ".jsx file should parse cleanly with TSX grammar");
        let exports = parser.extract_exports(&tree, source.as_bytes());
        assert!(exports.iter().any(|e| e.name == "Btn"));
    }
}
