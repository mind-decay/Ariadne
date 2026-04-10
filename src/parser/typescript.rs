use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::model::symbol::{LineSpan, SymbolDef, SymbolKind, Visibility};
use crate::model::workspace::WorkspaceInfo;
use crate::model::{CanonicalPath, FileSet};
use crate::parser::helpers;
use crate::parser::config::{BundlerConfig, TsConfig};
use crate::parser::symbols::SymbolExtractor;
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

impl SymbolExtractor for TypeScriptParser {
    fn extract_symbols(&self, tree: &tree_sitter::Tree, source: &[u8]) -> Vec<SymbolDef> {
        let mut symbols = Vec::new();
        let root = tree.root_node();
        self.extract_symbols_from_node(&root, source, &mut symbols, None, false);
        symbols
    }
}

impl TypeScriptParser {
    /// Recursively extract symbol definitions from a node and its children.
    fn extract_symbols_from_node(
        &self,
        node: &tree_sitter::Node,
        source: &[u8],
        symbols: &mut Vec<SymbolDef>,
        parent_name: Option<&str>,
        is_exported: bool,
    ) {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            match child.kind() {
                "export_statement" => {
                    // Children of export_statement are the actual declarations
                    self.extract_symbols_from_node(&child, source, symbols, parent_name, true);
                }
                "function_declaration" | "generator_function_declaration" => {
                    if let Some(name_node) = helpers::find_child_by_kind(&child, "identifier") {
                        if let Ok(name) = name_node.utf8_text(source) {
                            let visibility = if is_exported {
                                Visibility::Public
                            } else {
                                Visibility::Private
                            };
                            let kind = if parent_name.is_some() {
                                SymbolKind::Method
                            } else {
                                SymbolKind::Function
                            };
                            symbols.push(SymbolDef {
                                name: name.to_string(),
                                kind,
                                visibility,
                                span: node_span(&child),
                                signature: truncate_signature(child.utf8_text(source).ok(), 200),
                                parent: parent_name.map(|s| s.to_string()),
                            });
                        }
                    }
                }
                "class_declaration" => {
                    let class_name = helpers::find_child_by_kind(&child, "type_identifier")
                        .and_then(|n| n.utf8_text(source).ok())
                        .map(|s| s.to_string());
                    if let Some(ref name) = class_name {
                        let visibility = if is_exported {
                            Visibility::Public
                        } else {
                            Visibility::Private
                        };
                        symbols.push(SymbolDef {
                            name: name.clone(),
                            kind: SymbolKind::Class,
                            visibility,
                            span: node_span(&child),
                            signature: truncate_signature(
                                first_line_signature(&child, source).as_deref(),
                                200,
                            ),
                            parent: parent_name.map(|s| s.to_string()),
                        });
                    }
                    // Extract methods inside the class body
                    if let Some(body) = helpers::find_child_by_kind(&child, "class_body") {
                        self.extract_class_members(
                            &body,
                            source,
                            symbols,
                            class_name.as_deref(),
                        );
                    }
                }
                "interface_declaration" => {
                    if let Some(name_node) = helpers::find_child_by_kind(&child, "type_identifier")
                    {
                        if let Ok(name) = name_node.utf8_text(source) {
                            let visibility = if is_exported {
                                Visibility::Public
                            } else {
                                Visibility::Private
                            };
                            symbols.push(SymbolDef {
                                name: name.to_string(),
                                kind: SymbolKind::Interface,
                                visibility,
                                span: node_span(&child),
                                signature: truncate_signature(
                                    first_line_signature(&child, source).as_deref(),
                                    200,
                                ),
                                parent: parent_name.map(|s| s.to_string()),
                            });
                        }
                    }
                }
                "type_alias_declaration" => {
                    if let Some(name_node) = helpers::find_child_by_kind(&child, "type_identifier")
                    {
                        if let Ok(name) = name_node.utf8_text(source) {
                            let visibility = if is_exported {
                                Visibility::Public
                            } else {
                                Visibility::Private
                            };
                            symbols.push(SymbolDef {
                                name: name.to_string(),
                                kind: SymbolKind::Type,
                                visibility,
                                span: node_span(&child),
                                signature: truncate_signature(child.utf8_text(source).ok(), 200),
                                parent: parent_name.map(|s| s.to_string()),
                            });
                        }
                    }
                }
                "enum_declaration" => {
                    if let Some(name_node) = helpers::find_child_by_kind(&child, "identifier") {
                        if let Ok(name) = name_node.utf8_text(source) {
                            let visibility = if is_exported {
                                Visibility::Public
                            } else {
                                Visibility::Private
                            };
                            symbols.push(SymbolDef {
                                name: name.to_string(),
                                kind: SymbolKind::Enum,
                                visibility,
                                span: node_span(&child),
                                signature: truncate_signature(
                                    first_line_signature(&child, source).as_deref(),
                                    200,
                                ),
                                parent: parent_name.map(|s| s.to_string()),
                            });
                        }
                    }
                }
                "lexical_declaration" | "variable_declaration" => {
                    // const FOO = ... or const foo = () => {}
                    let mut decl_cursor = child.walk();
                    for declarator in child.children(&mut decl_cursor) {
                        if declarator.kind() == "variable_declarator" {
                            if let Some(name_node) =
                                helpers::find_child_by_kind(&declarator, "identifier")
                            {
                                if let Ok(name) = name_node.utf8_text(source) {
                                    let visibility = if is_exported {
                                        Visibility::Public
                                    } else {
                                        Visibility::Private
                                    };
                                    // Determine kind: named arrow function or const
                                    let kind = if is_arrow_function(&declarator) {
                                        SymbolKind::Function
                                    } else if is_upper_case_const(name) {
                                        SymbolKind::Const
                                    } else {
                                        SymbolKind::Variable
                                    };
                                    symbols.push(SymbolDef {
                                        name: name.to_string(),
                                        kind,
                                        visibility,
                                        span: node_span(&child),
                                        signature: truncate_signature(
                                            child.utf8_text(source).ok(),
                                            200,
                                        ),
                                        parent: parent_name.map(|s| s.to_string()),
                                    });
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }

    /// Extract methods and properties from a class body.
    fn extract_class_members(
        &self,
        body: &tree_sitter::Node,
        source: &[u8],
        symbols: &mut Vec<SymbolDef>,
        class_name: Option<&str>,
    ) {
        let mut cursor = body.walk();
        for member in body.children(&mut cursor) {
            match member.kind() {
                "method_definition" | "public_field_definition" => {
                    if let Some(name_node) = helpers::find_child_by_kind(&member, "property_identifier") {
                        if let Ok(name) = name_node.utf8_text(source) {
                            let visibility = extract_member_visibility(&member, source);
                            symbols.push(SymbolDef {
                                name: name.to_string(),
                                kind: SymbolKind::Method,
                                visibility,
                                span: node_span(&member),
                                signature: truncate_signature(
                                    first_line_signature(&member, source).as_deref(),
                                    200,
                                ),
                                parent: class_name.map(|s| s.to_string()),
                            });
                        }
                    }
                }
                _ => {}
            }
        }
    }
}

/// Check if a variable_declarator contains an arrow_function or function value.
fn is_arrow_function(declarator: &tree_sitter::Node) -> bool {
    let mut cursor = declarator.walk();
    for child in declarator.children(&mut cursor) {
        if child.kind() == "arrow_function" || child.kind() == "function" {
            return true;
        }
    }
    false
}

/// Check if a name matches UPPER_CASE const pattern: ^[A-Z][A-Z0-9_]*$
fn is_upper_case_const(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    let bytes = name.as_bytes();
    bytes[0].is_ascii_uppercase()
        && bytes
            .iter()
            .all(|&b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_')
}

/// Extract visibility from a TS/JS class member by checking for accessibility_modifier.
/// Defaults to Public if no modifier is present (TypeScript default).
fn extract_member_visibility(member: &tree_sitter::Node, source: &[u8]) -> Visibility {
    let mut cursor = member.walk();
    for child in member.children(&mut cursor) {
        if child.kind() == "accessibility_modifier" {
            if let Ok(text) = child.utf8_text(source) {
                return match text {
                    "private" => Visibility::Private,
                    "protected" => Visibility::Internal, // map TS protected → Internal
                    _ => Visibility::Public,
                };
            }
        }
    }
    Visibility::Public
}

/// Get the LineSpan from a tree-sitter node (0-based rows → 1-based lines).
fn node_span(node: &tree_sitter::Node) -> LineSpan {
    LineSpan {
        start: node.start_position().row as u32 + 1,
        end: node.end_position().row as u32 + 1,
    }
}

/// Extract the first line of a node's text as signature.
fn first_line_signature(node: &tree_sitter::Node, source: &[u8]) -> Option<String> {
    let text = node.utf8_text(source).ok()?;
    let first_line = text.lines().next()?;
    Some(first_line.to_string())
}

/// Truncate a signature to the given max character length (D-081).
/// Uses char-boundary-safe truncation to avoid panics on non-ASCII.
fn truncate_signature(sig: Option<&str>, max_len: usize) -> Option<String> {
    sig.map(|s| {
        let first_line = s.lines().next().unwrap_or(s);
        let truncated: String = first_line.chars().take(max_len).collect();
        if truncated.len() < first_line.len() {
            format!("{}...", truncated)
        } else {
            first_line.to_string()
        }
    })
}

/// TypeScript/JavaScript import resolver.
pub(crate) struct TypeScriptResolver {
    /// TypeScript path alias configs, keyed by directory containing tsconfig.json.
    ts_configs: Option<BTreeMap<PathBuf, TsConfig>>,
    /// Bundler alias configs (Vite/Webpack), keyed by directory containing config file.
    bundler_configs: Option<BTreeMap<PathBuf, BundlerConfig>>,
}

impl TypeScriptResolver {
    pub fn new() -> Self {
        Self {
            ts_configs: None,
            bundler_configs: None,
        }
    }

    /// Inject tsconfig path alias configurations for config-aware resolution (D-118).
    pub fn with_ts_configs(mut self, configs: BTreeMap<PathBuf, TsConfig>) -> Self {
        if !configs.is_empty() {
            self.ts_configs = Some(configs);
        }
        self
    }

    /// Inject bundler alias configurations for Vite/Webpack resolution (D-150).
    pub fn with_bundler_configs(mut self, configs: BTreeMap<PathBuf, BundlerConfig>) -> Self {
        if !configs.is_empty() {
            self.bundler_configs = Some(configs);
        }
        self
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

impl TypeScriptResolver {
    /// Try to resolve an import specifier via tsconfig path aliases (D-118).
    ///
    /// Finds the nearest tsconfig for the importing file, then:
    /// 1. Tries `resolve_path_alias` to get candidate paths from paths mapping
    /// 2. For each candidate, applies extension probing against known_files
    /// 3. If no alias matches but baseUrl is set, tries baseUrl-relative resolution
    ///
    /// Returns None on failure (silent fallthrough to existing logic).
    fn resolve_via_tsconfig(
        &self,
        specifier: &str,
        from_file: &CanonicalPath,
        known_files: &FileSet,
        configs: &BTreeMap<PathBuf, TsConfig>,
    ) -> Option<CanonicalPath> {
        // Determine the directory of the importing file for nearest-tsconfig lookup.
        // from_file is a CanonicalPath (project-relative), so we need to find
        // the tsconfig whose config_dir is an ancestor.
        let from_dir = from_file.parent().unwrap_or("");
        let from_dir_path = PathBuf::from(from_dir);

        let tsconfig = crate::parser::config::find_nearest_tsconfig(&from_dir_path, configs)?;

        // Try path alias resolution
        let candidates = crate::parser::config::tsconfig::resolve_path_alias(specifier, tsconfig);

        for candidate_path in &candidates {
            // Convert the absolute candidate path to a project-relative CanonicalPath.
            // The candidate is config_dir-relative (absolute on disk), so we need to
            // strip the config_dir prefix to get a project-relative path.
            // However, since our known_files are project-relative and config_dir
            // may be absolute, we convert candidates to strings and try probing.
            let candidate_str = candidate_path.to_string_lossy();
            // Normalize: strip leading ./ if present
            let normalized = candidate_str
                .strip_prefix("./")
                .unwrap_or(&candidate_str);

            if let Some(resolved) = Self::probe_ts_extensions(normalized, known_files) {
                return Some(resolved);
            }
        }

        None
    }

    /// Try to resolve an import specifier via bundler alias configs (D-150).
    ///
    /// Finds the nearest bundler config for the importing file, then checks
    /// if the specifier starts with any alias prefix. If match, substitutes
    /// the prefix and probes extensions against known_files.
    ///
    /// Priority: tsconfig paths > bundler aliases (D-150).
    fn resolve_via_bundler(
        &self,
        specifier: &str,
        from_file: &CanonicalPath,
        known_files: &FileSet,
        configs: &BTreeMap<PathBuf, BundlerConfig>,
    ) -> Option<CanonicalPath> {
        let from_dir = from_file.parent().unwrap_or("");
        let from_dir_path = PathBuf::from(from_dir);

        let bundler_config = crate::parser::config::find_nearest_bundler(&from_dir_path, configs)?;

        // Check aliases: longest prefix match first for specificity
        let mut matching_aliases: Vec<(&String, &String)> = bundler_config
            .aliases
            .iter()
            .filter(|(prefix, _)| specifier.starts_with(prefix.as_str()))
            .collect();
        matching_aliases.sort_by(|a, b| b.0.len().cmp(&a.0.len()));

        for (prefix, target) in matching_aliases {
            let rest = &specifier[prefix.len()..];
            // Rest may start with / — strip it
            let rest = rest.strip_prefix('/').unwrap_or(rest);

            let candidate_base = if rest.is_empty() {
                target.clone()
            } else {
                format!("{}/{}", target, rest)
            };

            if let Some(resolved) = Self::probe_ts_extensions(&candidate_base, known_files) {
                return Some(resolved);
            }
        }

        None
    }

    /// Probe a base path with TypeScript/JavaScript extension variants.
    ///
    /// Tries: exact match, .ts, .tsx, .js, .jsx, .mjs, .cjs, then index files.
    fn probe_ts_extensions(base: &str, known_files: &FileSet) -> Option<CanonicalPath> {
        // 1. Exact match
        let exact = CanonicalPath::new(base);
        if known_files.contains(&exact) {
            return Some(exact);
        }

        // 2. Extension probing
        let extensions = &["ts", "tsx", "js", "jsx", "mjs", "cjs"];
        for ext in extensions {
            let candidate = CanonicalPath::new(format!("{}.{}", base, ext));
            if known_files.contains(&candidate) {
                return Some(candidate);
            }
        }

        // 3. Index file probing
        let index_extensions = &["ts", "tsx", "js", "jsx"];
        for ext in index_extensions {
            let candidate = CanonicalPath::new(format!("{}/index.{}", base, ext));
            if known_files.contains(&candidate) {
                return Some(candidate);
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

        // Priority 1: tsconfig path alias resolution (D-118).
        // This MUST come before the bare-specifier skip, because aliases like
        // `@/components/Button` look like bare specifiers but should resolve
        // via tsconfig paths.
        if let Some(ref configs) = self.ts_configs {
            if let Some(resolved) = self.resolve_via_tsconfig(specifier, from_file, known_files, configs) {
                return Some(resolved);
            }
        }

        // Priority 2: bundler alias resolution (D-150).
        // After tsconfig paths (which are the language-level standard) but before
        // bare specifier skip. Bundler aliases like `@` → `./src` look like bare
        // specifiers but should resolve via bundler config.
        if let Some(ref configs) = self.bundler_configs {
            if let Some(resolved) = self.resolve_via_bundler(specifier, from_file, known_files, configs) {
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

#[cfg(test)]
mod config_resolution_tests {
    use super::*;
    use crate::parser::config::TsConfig;

    fn make_resolver_with_config(
        config_dir: &str,
        base_url: Option<&str>,
        paths: Vec<(&str, Vec<&str>)>,
    ) -> TypeScriptResolver {
        let mut ts_configs = BTreeMap::new();
        let mut paths_map = BTreeMap::new();
        for (pattern, targets) in paths {
            paths_map.insert(
                pattern.to_string(),
                targets.into_iter().map(|s| s.to_string()).collect(),
            );
        }
        let config = TsConfig {
            config_dir: PathBuf::from(config_dir),
            base_url: base_url.map(|s| s.to_string()),
            paths: paths_map,
        };
        ts_configs.insert(PathBuf::from(config_dir), config);
        TypeScriptResolver::new().with_ts_configs(ts_configs)
    }

    fn make_import(path: &str) -> RawImport {
        RawImport {
            path: path.to_string(),
            symbols: vec![],
            is_type_only: false,
            kind: ImportKind::Regular,
        }
    }

    #[test]
    fn resolve_via_path_alias() {
        let resolver = make_resolver_with_config(
            "",
            Some("."),
            vec![("@/*", vec!["src/*"])],
        );
        let known = FileSet::from_iter(vec![CanonicalPath::new("src/components/Button.ts")]);

        let import = make_import("@/components/Button");
        let from = CanonicalPath::new("src/app.ts");

        let result = resolver.resolve(&import, &from, &known, None);
        assert_eq!(
            result,
            Some(CanonicalPath::new("src/components/Button.ts"))
        );
    }

    #[test]
    fn resolve_via_base_url() {
        let resolver = make_resolver_with_config(
            "",
            Some("src"),
            vec![],
        );
        let known = FileSet::from_iter(vec![CanonicalPath::new("src/utils/helper.ts")]);

        let import = make_import("utils/helper");
        let from = CanonicalPath::new("src/app.ts");

        let result = resolver.resolve(&import, &from, &known, None);
        assert_eq!(
            result,
            Some(CanonicalPath::new("src/utils/helper.ts"))
        );
    }

    #[test]
    fn alias_fails_falls_through_to_standard() {
        // Path alias maps to a file that doesn't exist, but the import
        // is also a relative path that does exist via standard resolution.
        let resolver = make_resolver_with_config(
            "",
            Some("."),
            vec![("@/*", vec!["lib/*"])],
        );
        let known = FileSet::from_iter(vec![CanonicalPath::new("src/utils.ts")]);

        // This is a relative import — alias won't match, standard will.
        let import = make_import("./utils");
        let from = CanonicalPath::new("src/app.ts");

        let result = resolver.resolve(&import, &from, &known, None);
        assert_eq!(result, Some(CanonicalPath::new("src/utils.ts")));
    }

    #[test]
    fn no_config_preserves_existing_behavior() {
        let resolver = TypeScriptResolver::new();
        let known = FileSet::from_iter(vec![CanonicalPath::new("src/utils.ts")]);

        let import = make_import("./utils");
        let from = CanonicalPath::new("src/app.ts");

        let result = resolver.resolve(&import, &from, &known, None);
        assert_eq!(result, Some(CanonicalPath::new("src/utils.ts")));
    }

    #[test]
    fn alias_with_extension_probing() {
        // Alias resolves to path without extension — probing should find .tsx
        let resolver = make_resolver_with_config(
            "",
            Some("."),
            vec![("@components/*", vec!["src/components/*"])],
        );
        let known = FileSet::from_iter(vec![CanonicalPath::new("src/components/Button.tsx")]);

        let import = make_import("@components/Button");
        let from = CanonicalPath::new("src/pages/Home.tsx");

        let result = resolver.resolve(&import, &from, &known, None);
        assert_eq!(
            result,
            Some(CanonicalPath::new("src/components/Button.tsx"))
        );
    }

    #[test]
    fn alias_with_index_probing() {
        // Alias resolves to directory — index probing should find index.ts
        let resolver = make_resolver_with_config(
            "",
            Some("."),
            vec![("@/*", vec!["src/*"])],
        );
        let known = FileSet::from_iter(vec![CanonicalPath::new("src/components/index.ts")]);

        let import = make_import("@/components");
        let from = CanonicalPath::new("src/app.ts");

        let result = resolver.resolve(&import, &from, &known, None);
        assert_eq!(
            result,
            Some(CanonicalPath::new("src/components/index.ts"))
        );
    }

    #[test]
    fn bare_specifier_without_config_skipped() {
        // Without config, bare specifiers like "lodash" are skipped (npm packages)
        let resolver = TypeScriptResolver::new();
        let known = FileSet::new();

        let import = make_import("lodash");
        let from = CanonicalPath::new("src/app.ts");

        let result = resolver.resolve(&import, &from, &known, None);
        assert_eq!(result, None);
    }

    #[test]
    fn empty_configs_no_effect() {
        // with_ts_configs with empty map should behave like no config
        let resolver = TypeScriptResolver::new().with_ts_configs(BTreeMap::new());
        assert!(resolver.ts_configs.is_none());
    }

    // --- Bundler alias resolution (D-150) ---

    fn make_bundler_resolver(aliases: Vec<(&str, &str)>) -> TypeScriptResolver {
        use crate::parser::config::BundlerConfig;
        let mut alias_map = BTreeMap::new();
        for (k, v) in aliases {
            alias_map.insert(k.to_string(), v.to_string());
        }
        let mut configs = BTreeMap::new();
        configs.insert(
            PathBuf::from(""),
            BundlerConfig {
                config_dir: PathBuf::from(""),
                aliases: alias_map,
                modules: Vec::new(),
            },
        );
        TypeScriptResolver::new().with_bundler_configs(configs)
    }

    #[test]
    fn bundler_alias_resolves() {
        let resolver = make_bundler_resolver(vec![("@", "src")]);
        let known = FileSet::from_iter(vec![
            CanonicalPath::new("src/components/Button.tsx"),
        ]);

        let import = make_import("@/components/Button");
        let from = CanonicalPath::new("src/app.ts");

        let result = resolver.resolve(&import, &from, &known, None);
        assert_eq!(
            result,
            Some(CanonicalPath::new("src/components/Button.tsx"))
        );
    }

    #[test]
    fn bundler_alias_exact_match() {
        let resolver = make_bundler_resolver(vec![("~utils", "src/utils")]);
        let known = FileSet::from_iter(vec![
            CanonicalPath::new("src/utils/index.ts"),
        ]);

        let import = make_import("~utils");
        let from = CanonicalPath::new("src/app.ts");

        let result = resolver.resolve(&import, &from, &known, None);
        assert_eq!(
            result,
            Some(CanonicalPath::new("src/utils/index.ts"))
        );
    }

    #[test]
    fn tsconfig_takes_priority_over_bundler() {
        // SC-10: tsconfig paths > bundler aliases
        let mut ts_configs = BTreeMap::new();
        let mut paths = BTreeMap::new();
        paths.insert("@/*".to_string(), vec!["lib/*".to_string()]);
        ts_configs.insert(
            PathBuf::from(""),
            TsConfig {
                config_dir: PathBuf::from(""),
                base_url: Some(".".to_string()),
                paths,
            },
        );

        let mut bundler_aliases = BTreeMap::new();
        bundler_aliases.insert("@".to_string(), "src".to_string());
        let mut bundler_configs = BTreeMap::new();
        bundler_configs.insert(
            PathBuf::from(""),
            BundlerConfig {
                config_dir: PathBuf::from(""),
                aliases: bundler_aliases,
                modules: Vec::new(),
            },
        );

        let resolver = TypeScriptResolver::new()
            .with_ts_configs(ts_configs)
            .with_bundler_configs(bundler_configs);

        // File exists in lib/ (tsconfig target) but NOT in src/ (bundler target)
        let known = FileSet::from_iter(vec![
            CanonicalPath::new("lib/utils.ts"),
        ]);

        let import = make_import("@/utils");
        let from = CanonicalPath::new("src/app.ts");

        let result = resolver.resolve(&import, &from, &known, None);
        // Should resolve via tsconfig (lib/), not bundler (src/)
        assert_eq!(result, Some(CanonicalPath::new("lib/utils.ts")));
    }

    #[test]
    fn bundler_alias_no_match_falls_through() {
        let resolver = make_bundler_resolver(vec![("@", "src")]);
        let known = FileSet::new();

        let import = make_import("lodash");
        let from = CanonicalPath::new("src/app.ts");

        // "lodash" doesn't match "@" alias, bare specifier → None
        let result = resolver.resolve(&import, &from, &known, None);
        assert_eq!(result, None);
    }

    #[test]
    fn empty_bundler_configs_no_effect() {
        let resolver = TypeScriptResolver::new().with_bundler_configs(BTreeMap::new());
        assert!(resolver.bundler_configs.is_none());
    }
}
