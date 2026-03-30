use crate::model::symbol::{LineSpan, SymbolDef, SymbolKind, Visibility};
use crate::model::workspace::WorkspaceInfo;
use crate::model::{CanonicalPath, FileSet};
use crate::parser::helpers;
use crate::parser::symbols::SymbolExtractor;
use crate::parser::config::PyProjectConfig;
use crate::parser::traits::{ImportKind, ImportResolver, LanguageParser, RawExport, RawImport};

/// Parser and resolver for Python files (.py, .pyi).
pub(crate) struct PythonParser;

impl PythonParser {
    pub fn new() -> Self {
        Self
    }

    /// Extract the string content from a string node, stripping quotes.
    fn string_content<'a>(node: &tree_sitter::Node<'a>, source: &'a [u8]) -> Option<&'a str> {
        let text = node.utf8_text(source).ok()?;
        // Strip surrounding quotes (single, double, triple-single, triple-double)
        if text.starts_with("\"\"\"") || text.starts_with("'''") {
            let inner = &text[3..text.len().saturating_sub(3)];
            return Some(inner);
        }
        if text.len() >= 2 {
            let first = text.as_bytes()[0];
            let last = text.as_bytes()[text.len() - 1];
            if (first == b'"' || first == b'\'') && first == last {
                return Some(&text[1..text.len() - 1]);
            }
        }
        Some(text)
    }

    /// Count the number of leading dots in a relative import.
    fn count_relative_dots(node: &tree_sitter::Node, source: &[u8]) -> usize {
        // In tree-sitter-python, relative imports have a "relative_import" node
        // or the module name starts with dots
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "relative_import" {
                // The relative_import contains import_prefix (the dots) and optionally a dotted_name
                let mut rel_cursor = child.walk();
                for rel_child in child.children(&mut rel_cursor) {
                    if rel_child.kind() == "import_prefix" {
                        if let Ok(text) = rel_child.utf8_text(source) {
                            return text.chars().filter(|c| *c == '.').count();
                        }
                    }
                }
            }
        }
        0
    }

    /// Extract the module name from a from_import or import statement.
    fn extract_module_name(node: &tree_sitter::Node, source: &[u8]) -> Option<String> {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "dotted_name" {
                return child.utf8_text(source).ok().map(|s| s.to_string());
            }
            if child.kind() == "relative_import" {
                // Get the dotted_name within the relative_import
                if let Some(dotted) = helpers::find_child_by_kind(&child, "dotted_name") {
                    return dotted.utf8_text(source).ok().map(|s| s.to_string());
                }
                // Just dots, no module name (e.g., `from . import foo`)
                return None;
            }
        }
        None
    }

    /// Extract imported names from from_import statement.
    fn extract_from_import_names(node: &tree_sitter::Node, source: &[u8]) -> Vec<String> {
        let mut names = Vec::new();
        let mut cursor = node.walk();
        let mut past_import = false;
        for child in node.children(&mut cursor) {
            if child.kind() == "import" {
                past_import = true;
                continue;
            }
            if !past_import {
                continue;
            }
            match child.kind() {
                "dotted_name" | "identifier" => {
                    if let Ok(name) = child.utf8_text(source) {
                        names.push(name.to_string());
                    }
                }
                "aliased_import" => {
                    if let Some(name_node) = helpers::find_child_by_kind(&child, "dotted_name")
                        .or_else(|| helpers::find_child_by_kind(&child, "identifier"))
                    {
                        if let Ok(name) = name_node.utf8_text(source) {
                            names.push(name.to_string());
                        }
                    }
                }
                "wildcard_import" => {
                    names.push("*".to_string());
                }
                _ => {}
            }
        }
        names
    }

    /// Check if an import is a __future__ import.
    fn is_future_import(module_name: &str) -> bool {
        module_name == "__future__"
    }
}

impl LanguageParser for PythonParser {
    fn language(&self) -> &str {
        "python"
    }

    fn extensions(&self) -> &[&str] {
        &["py", "pyi"]
    }

    fn tree_sitter_language(&self) -> tree_sitter::Language {
        tree_sitter::Language::from(tree_sitter_python::LANGUAGE)
    }

    fn extract_imports(&self, tree: &tree_sitter::Tree, source: &[u8]) -> Vec<RawImport> {
        let mut imports = Vec::new();
        let root = tree.root_node();
        let mut cursor = root.walk();

        for node in root.children(&mut cursor) {
            match node.kind() {
                // `import os` or `import os.path`
                "import_statement" => {
                    let mut child_cursor = node.walk();
                    for child in node.children(&mut child_cursor) {
                        match child.kind() {
                            "dotted_name" => {
                                if let Ok(name) = child.utf8_text(source) {
                                    if !Self::is_future_import(name) {
                                        imports.push(RawImport {
                                            path: name.to_string(),
                                            symbols: Vec::new(),
                                            is_type_only: false,
                                            kind: ImportKind::Regular,
                                        });
                                    }
                                }
                            }
                            "aliased_import" => {
                                if let Some(name_node) =
                                    helpers::find_child_by_kind(&child, "dotted_name")
                                {
                                    if let Ok(name) = name_node.utf8_text(source) {
                                        if !Self::is_future_import(name) {
                                            imports.push(RawImport {
                                                path: name.to_string(),
                                                symbols: Vec::new(),
                                                is_type_only: false,
                                                kind: ImportKind::Regular,
                                            });
                                        }
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }

                // `from os import path` or `from . import foo` or `from ..utils import helper`
                "import_from_statement" => {
                    let dot_count = Self::count_relative_dots(&node, source);
                    let module_name = Self::extract_module_name(&node, source);
                    let symbols = Self::extract_from_import_names(&node, source);

                    // Build the path
                    let path = if dot_count > 0 {
                        // Relative import: dots + optional module name
                        let dots: String = ".".repeat(dot_count);
                        match &module_name {
                            Some(name) => format!("{}{}", dots, name),
                            None => dots,
                        }
                    } else {
                        match &module_name {
                            Some(name) => name.clone(),
                            None => continue,
                        }
                    };

                    // Skip __future__ imports
                    if module_name.as_deref() == Some("__future__") {
                        continue;
                    }

                    imports.push(RawImport {
                        path,
                        symbols,
                        is_type_only: false,
                        kind: ImportKind::Regular,
                    });
                }

                // Check for TYPE_CHECKING blocks
                "if_statement" => {
                    Self::extract_type_checking_imports(&node, source, &mut imports);
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
        let mut has_all = false;

        for node in root.children(&mut cursor) {
            // __all__ = ['foo', 'bar']
            if node.kind() == "expression_statement" {
                if let Some(assignment) = helpers::find_child_by_kind(&node, "assignment") {
                    let mut assign_cursor = assignment.walk();
                    let children: Vec<_> = assignment.children(&mut assign_cursor).collect();

                    // Check if left side is __all__
                    if let Some(left) = children.first() {
                        if left.kind() == "identifier"
                            && left.utf8_text(source).ok() == Some("__all__")
                        {
                            has_all = true;
                            // Right side should be a list
                            for child in &children {
                                if child.kind() == "list" {
                                    let mut list_cursor = child.walk();
                                    for item in child.children(&mut list_cursor) {
                                        if item.kind() == "string" {
                                            if let Some(name) = Self::string_content(&item, source)
                                            {
                                                exports.push(RawExport {
                                                    name: name.to_string(),
                                                    is_re_export: false,
                                                    source: None,
                                                });
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // If no __all__, fall back to public definitions
        if !has_all {
            let mut fallback_cursor = root.walk();
            for node in root.children(&mut fallback_cursor) {
                match node.kind() {
                    "function_definition" | "decorated_definition" => {
                        let def_node = if node.kind() == "decorated_definition" {
                            // Get the actual definition inside the decorator
                            helpers::find_child_by_kind(&node, "function_definition")
                                .or_else(|| helpers::find_child_by_kind(&node, "class_definition"))
                        } else {
                            Some(node)
                        };

                        if let Some(def) = def_node {
                            if let Some(name_node) = helpers::find_child_by_kind(&def, "identifier")
                            {
                                if let Ok(name) = name_node.utf8_text(source) {
                                    if !name.starts_with('_') {
                                        exports.push(RawExport {
                                            name: name.to_string(),
                                            is_re_export: false,
                                            source: None,
                                        });
                                    }
                                }
                            }
                        }
                    }
                    "class_definition" => {
                        if let Some(name_node) = helpers::find_child_by_kind(&node, "identifier") {
                            if let Ok(name) = name_node.utf8_text(source) {
                                if !name.starts_with('_') {
                                    exports.push(RawExport {
                                        name: name.to_string(),
                                        is_re_export: false,
                                        source: None,
                                    });
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        exports
    }
}

impl PythonParser {
    /// Extract imports from within TYPE_CHECKING blocks, marking them as type-only.
    fn extract_type_checking_imports(
        node: &tree_sitter::Node,
        source: &[u8],
        imports: &mut Vec<RawImport>,
    ) {
        // Check if condition is TYPE_CHECKING
        let mut cursor = node.walk();
        let mut is_type_checking = false;

        for child in node.children(&mut cursor) {
            if child.kind() == "identifier" || child.kind() == "attribute" {
                if let Ok(text) = child.utf8_text(source) {
                    if text == "TYPE_CHECKING" || text.ends_with(".TYPE_CHECKING") {
                        is_type_checking = true;
                    }
                }
            }
        }

        if !is_type_checking {
            return;
        }

        // Find the block and extract imports from it
        if let Some(block) = helpers::find_child_by_kind(node, "block") {
            let mut block_cursor = block.walk();
            for stmt in block.children(&mut block_cursor) {
                match stmt.kind() {
                    "import_statement" => {
                        let mut child_cursor = stmt.walk();
                        for child in stmt.children(&mut child_cursor) {
                            if child.kind() == "dotted_name" {
                                if let Ok(name) = child.utf8_text(source) {
                                    imports.push(RawImport {
                                        path: name.to_string(),
                                        symbols: Vec::new(),
                                        is_type_only: true,
                                        kind: ImportKind::Regular,
                                    });
                                }
                            }
                            if child.kind() == "aliased_import" {
                                if let Some(name_node) =
                                    helpers::find_child_by_kind(&child, "dotted_name")
                                {
                                    if let Ok(name) = name_node.utf8_text(source) {
                                        imports.push(RawImport {
                                            path: name.to_string(),
                                            symbols: Vec::new(),
                                            is_type_only: true,
                                            kind: ImportKind::Regular,
                                        });
                                    }
                                }
                            }
                        }
                    }
                    "import_from_statement" => {
                        let dot_count = Self::count_relative_dots(&stmt, source);
                        let module_name = Self::extract_module_name(&stmt, source);
                        let symbols = Self::extract_from_import_names(&stmt, source);

                        let path = if dot_count > 0 {
                            let dots: String = ".".repeat(dot_count);
                            match &module_name {
                                Some(name) => format!("{}{}", dots, name),
                                None => dots,
                            }
                        } else {
                            match &module_name {
                                Some(name) => name.clone(),
                                None => continue,
                            }
                        };

                        imports.push(RawImport {
                            path,
                            symbols,
                            is_type_only: true,
                            kind: ImportKind::Regular,
                        });
                    }
                    _ => {}
                }
            }
        }
    }
}

impl SymbolExtractor for PythonParser {
    fn extract_symbols(&self, tree: &tree_sitter::Tree, source: &[u8]) -> Vec<SymbolDef> {
        let mut symbols = Vec::new();
        let root = tree.root_node();
        extract_python_symbols_from_node(&root, source, &mut symbols, None);
        symbols
    }
}

/// Recursively extract symbol definitions from a Python AST node.
fn extract_python_symbols_from_node(
    node: &tree_sitter::Node,
    source: &[u8],
    symbols: &mut Vec<SymbolDef>,
    parent_name: Option<&str>,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "function_definition" => {
                if let Some(name_node) = helpers::find_child_by_kind(&child, "identifier") {
                    if let Ok(name) = name_node.utf8_text(source) {
                        // Skip `self` — not a symbol
                        if name == "self" {
                            continue;
                        }
                        let kind = if parent_name.is_some() {
                            SymbolKind::Method
                        } else {
                            SymbolKind::Function
                        };
                        symbols.push(SymbolDef {
                            name: name.to_string(),
                            kind,
                            visibility: python_visibility(name),
                            span: python_node_span(&child),
                            signature: python_truncate_signature(&child, source, 200),
                            parent: parent_name.map(|s| s.to_string()),
                        });
                    }
                }
            }
            "decorated_definition" => {
                // Get the actual definition inside the decorator
                if let Some(def) = helpers::find_child_by_kind(&child, "function_definition")
                    .or_else(|| helpers::find_child_by_kind(&child, "class_definition"))
                {
                    // Process the inner definition with the same parent context
                    let inner_kind = def.kind();
                    if inner_kind == "class_definition" {
                        let class_name = helpers::find_child_by_kind(&def, "identifier")
                            .and_then(|n| n.utf8_text(source).ok())
                            .map(|s| s.to_string());
                        if let Some(ref name) = class_name {
                            symbols.push(SymbolDef {
                                name: name.clone(),
                                kind: SymbolKind::Class,
                                visibility: python_visibility(name),
                                span: python_node_span(&child),
                                signature: python_truncate_signature(&def, source, 200),
                                parent: parent_name.map(|s| s.to_string()),
                            });
                        }
                        // Extract methods inside the class body
                        if let Some(body) = helpers::find_child_by_kind(&def, "block") {
                            extract_python_symbols_from_node(
                                &body,
                                source,
                                symbols,
                                class_name.as_deref(),
                            );
                        }
                    } else {
                        // function_definition inside decorated_definition
                        if let Some(name_node) = helpers::find_child_by_kind(&def, "identifier") {
                            if let Ok(name) = name_node.utf8_text(source) {
                                if name != "self" {
                                    let kind = if parent_name.is_some() {
                                        SymbolKind::Method
                                    } else {
                                        SymbolKind::Function
                                    };
                                    symbols.push(SymbolDef {
                                        name: name.to_string(),
                                        kind,
                                        visibility: python_visibility(name),
                                        span: python_node_span(&child),
                                        signature: python_truncate_signature(&def, source, 200),
                                        parent: parent_name.map(|s| s.to_string()),
                                    });
                                }
                            }
                        }
                    }
                }
            }
            "class_definition" => {
                let class_name = helpers::find_child_by_kind(&child, "identifier")
                    .and_then(|n| n.utf8_text(source).ok())
                    .map(|s| s.to_string());
                if let Some(ref name) = class_name {
                    symbols.push(SymbolDef {
                        name: name.clone(),
                        kind: SymbolKind::Class,
                        visibility: python_visibility(name),
                        span: python_node_span(&child),
                        signature: python_truncate_signature(&child, source, 200),
                        parent: parent_name.map(|s| s.to_string()),
                    });
                }
                // Extract methods inside the class body
                if let Some(body) = helpers::find_child_by_kind(&child, "block") {
                    extract_python_symbols_from_node(&body, source, symbols, class_name.as_deref());
                }
            }
            "expression_statement" => {
                // Check for UPPER_CASE constant assignments
                if let Some(assignment) = helpers::find_child_by_kind(&child, "assignment") {
                    let mut assign_cursor = assignment.walk();
                    let children: Vec<_> = assignment.children(&mut assign_cursor).collect();
                    if let Some(left) = children.first() {
                        if left.kind() == "identifier" {
                            if let Ok(name) = left.utf8_text(source) {
                                if is_python_constant(name) {
                                    symbols.push(SymbolDef {
                                        name: name.to_string(),
                                        kind: SymbolKind::Const,
                                        visibility: python_visibility(name),
                                        span: python_node_span(&child),
                                        signature: python_truncate_signature(
                                            &child, source, 200,
                                        ),
                                        parent: parent_name.map(|s| s.to_string()),
                                    });
                                }
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

/// Python visibility: starts with `__` → Private, `_` → Private, else Public.
fn python_visibility(name: &str) -> Visibility {
    if name.starts_with("__") || name.starts_with('_') {
        Visibility::Private
    } else {
        Visibility::Public
    }
}

/// Check if a name matches UPPER_CASE constant pattern: `^[A-Z][A-Z0-9_]*$`.
fn is_python_constant(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    let mut chars = name.chars();
    match chars.next() {
        Some(c) if c.is_ascii_uppercase() => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
}

/// Get LineSpan from a tree-sitter node (1-based).
fn python_node_span(node: &tree_sitter::Node) -> LineSpan {
    LineSpan {
        start: node.start_position().row as u32 + 1,
        end: node.end_position().row as u32 + 1,
    }
}

/// Extract first line of node text, truncated to max_len.
/// Uses char-safe truncation to avoid panics on non-ASCII.
fn python_truncate_signature(
    node: &tree_sitter::Node,
    source: &[u8],
    max_len: usize,
) -> Option<String> {
    let text = node.utf8_text(source).ok()?;
    let first_line = text.lines().next()?;
    let truncated: String = first_line.chars().take(max_len).collect();
    if truncated.len() < first_line.len() {
        Some(format!("{}...", truncated))
    } else {
        Some(first_line.to_string())
    }
}

/// Python import resolver.
pub(crate) struct PythonResolver {
    /// Python project config for src-layout resolution (D-118).
    config: Option<PyProjectConfig>,
}

impl PythonResolver {
    pub fn new() -> Self {
        Self { config: None }
    }

    /// Inject Python project configuration for config-aware resolution (D-118).
    pub fn with_config(config: PyProjectConfig) -> Self {
        Self { config: Some(config) }
    }

    /// Check if a specifier is a relative import (starts with dots).
    fn is_relative(path: &str) -> bool {
        path.starts_with('.')
    }

    /// Count leading dots and return (dot_count, remainder).
    fn parse_relative(path: &str) -> (usize, &str) {
        let dot_count = path.chars().take_while(|c| *c == '.').count();
        (dot_count, &path[dot_count..])
    }
}

impl ImportResolver for PythonResolver {
    fn resolve(
        &self,
        import: &RawImport,
        from_file: &CanonicalPath,
        known_files: &FileSet,
        _workspace: Option<&WorkspaceInfo>,
    ) -> Option<CanonicalPath> {
        let specifier = &import.path;

        if Self::is_relative(specifier) {
            // Relative import
            let (dot_count, remainder) = Self::parse_relative(specifier);

            // Start from the directory of the current file
            let base_dir = from_file.parent().unwrap_or("");

            // Go up (dot_count - 1) directories (one dot = current package)
            let mut segments: Vec<&str> = if base_dir.is_empty() {
                Vec::new()
            } else {
                base_dir.split('/').collect()
            };

            // Each dot beyond the first goes up one directory
            for _ in 1..dot_count {
                segments.pop();
            }

            // Append the remainder (dots to slashes)
            if !remainder.is_empty() {
                for part in remainder.split('.') {
                    if !part.is_empty() {
                        segments.push(part);
                    }
                }
            }

            let resolved_base = segments.join("/");
            Self::probe_python_path(&resolved_base, known_files)
        } else {
            // Absolute import — convert dots to path separators
            let path = specifier.replace('.', "/");

            // If src-layout is configured (D-118), also try prepending "src/"
            // before standard resolution. This handles projects where packages
            // live under src/ but imports use bare package names.
            if let Some(ref cfg) = self.config {
                if cfg.src_layout {
                    let src_path = format!("src/{}", path);
                    if let Some(resolved) = Self::probe_python_path(&src_path, known_files) {
                        return Some(resolved);
                    }
                }
            }

            Self::probe_python_path(&path, known_files)
        }
    }
}

impl PythonResolver {
    /// Probe for a Python module path: try .py, .pyi, and __init__.py.
    fn probe_python_path(base: &str, known_files: &FileSet) -> Option<CanonicalPath> {
        // Try as a direct file
        for ext in &["py", "pyi"] {
            let candidate = CanonicalPath::new(format!("{}.{}", base, ext));
            if known_files.contains(&candidate) {
                return Some(candidate);
            }
        }

        // Try as a package directory (__init__.py)
        let init_candidate = CanonicalPath::new(format!("{}/__init__.py", base));
        if known_files.contains(&init_candidate) {
            return Some(init_candidate);
        }

        let init_pyi = CanonicalPath::new(format!("{}/__init__.pyi", base));
        if known_files.contains(&init_pyi) {
            return Some(init_pyi);
        }

        None
    }
}

pub(crate) fn symbol_extractor() -> std::sync::Arc<dyn SymbolExtractor> {
    std::sync::Arc::new(PythonParser::new())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::traits::LanguageParser;

    fn parse(source: &str) -> tree_sitter::Tree {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter::Language::from(tree_sitter_python::LANGUAGE))
            .unwrap();
        parser.parse(source, None).unwrap()
    }

    fn imports(source: &str) -> Vec<RawImport> {
        let tree = parse(source);
        PythonParser::new().extract_imports(&tree, source.as_bytes())
    }

    fn exports(source: &str) -> Vec<RawExport> {
        let tree = parse(source);
        PythonParser::new().extract_exports(&tree, source.as_bytes())
    }

    // ---- Import tests ----

    #[test]
    fn import_simple_module() {
        let result = imports("import os");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].path, "os");
        assert!(result[0].symbols.is_empty());
        assert!(!result[0].is_type_only);
    }

    #[test]
    fn import_dotted_module() {
        let result = imports("import os.path");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].path, "os.path");
    }

    #[test]
    fn from_import_symbols() {
        let result = imports("from os import path, getcwd");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].path, "os");
        assert_eq!(result[0].symbols, vec!["path", "getcwd"]);
    }

    #[test]
    fn from_relative_import_single_dot() {
        let result = imports("from . import utils");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].path, ".");
        assert_eq!(result[0].symbols, vec!["utils"]);
    }

    #[test]
    fn from_relative_import_double_dot() {
        let result = imports("from ..pkg import helper");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].path, "..pkg");
        assert_eq!(result[0].symbols, vec!["helper"]);
    }

    #[test]
    fn from_import_wildcard() {
        let result = imports("from pkg import *");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].path, "pkg");
        assert_eq!(result[0].symbols, vec!["*"]);
    }

    #[test]
    fn import_aliased() {
        let result = imports("import numpy as np");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].path, "numpy");
    }

    #[test]
    fn type_checking_imports() {
        let source = r#"
from typing import TYPE_CHECKING
if TYPE_CHECKING:
    import some_module
    from other import Foo
"#;
        let result = imports(source);
        // The top-level `from typing import TYPE_CHECKING` counts as one
        // Plus 2 type-only imports inside the block
        let type_only: Vec<_> = result.iter().filter(|i| i.is_type_only).collect();
        assert_eq!(type_only.len(), 2);
        assert!(type_only.iter().any(|i| i.path == "some_module"));
        assert!(type_only.iter().any(|i| i.path == "other"));
    }

    #[test]
    fn future_import_skipped() {
        let result = imports("from __future__ import annotations");
        assert!(result.is_empty());
    }

    #[test]
    fn empty_source_no_imports() {
        let result = imports("");
        assert!(result.is_empty());
    }

    #[test]
    fn malformed_source_no_crash() {
        let result = imports("from import");
        // Should not panic; result may be empty or partial
        let _ = result;
    }

    // ---- Export tests ----

    #[test]
    fn exports_all_list() {
        let source = r#"__all__ = ['foo', 'bar']"#;
        let result = exports(source);
        assert_eq!(result.len(), 2);
        assert!(result.iter().any(|e| e.name == "foo"));
        assert!(result.iter().any(|e| e.name == "bar"));
    }

    #[test]
    fn exports_public_functions_and_classes() {
        let source = r#"
def public_func():
    pass

def _private_func():
    pass

class MyClass:
    pass

class _Internal:
    pass
"#;
        let result = exports(source);
        let names: Vec<&str> = result.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"public_func"));
        assert!(names.contains(&"MyClass"));
        assert!(!names.contains(&"_private_func"));
        assert!(!names.contains(&"_Internal"));
    }

    #[test]
    fn exports_all_overrides_fallback() {
        let source = r#"
__all__ = ['only_this']

def public_func():
    pass
"#;
        let result = exports(source);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "only_this");
    }

    #[test]
    fn exports_empty_source() {
        let result = exports("");
        assert!(result.is_empty());
    }
}

#[cfg(test)]
mod config_resolution_tests {
    use super::*;
    use crate::parser::config::PyProjectConfig;

    fn make_import(path: &str) -> RawImport {
        RawImport {
            path: path.to_string(),
            symbols: vec![],
            is_type_only: false,
            kind: ImportKind::Regular,
        }
    }

    #[test]
    fn resolve_src_layout_absolute_import() {
        let resolver = PythonResolver::with_config(PyProjectConfig {
            src_layout: true,
            package_name: Some("myapp".to_string()),
        });
        let known = FileSet::from_iter(vec![CanonicalPath::new("src/myapp/utils.py")]);

        let import = make_import("myapp.utils");
        let from = CanonicalPath::new("src/myapp/main.py");

        let result = resolver.resolve(&import, &from, &known, None);
        assert_eq!(result, Some(CanonicalPath::new("src/myapp/utils.py")));
    }

    #[test]
    fn resolve_src_layout_package_init() {
        let resolver = PythonResolver::with_config(PyProjectConfig {
            src_layout: true,
            package_name: Some("myapp".to_string()),
        });
        let known = FileSet::from_iter(vec![CanonicalPath::new("src/myapp/__init__.py")]);

        let import = make_import("myapp");
        let from = CanonicalPath::new("tests/test_main.py");

        let result = resolver.resolve(&import, &from, &known, None);
        assert_eq!(result, Some(CanonicalPath::new("src/myapp/__init__.py")));
    }

    #[test]
    fn no_src_layout_standard_resolution() {
        let resolver = PythonResolver::with_config(PyProjectConfig {
            src_layout: false,
            package_name: Some("myapp".to_string()),
        });
        let known = FileSet::from_iter(vec![CanonicalPath::new("myapp/utils.py")]);

        let import = make_import("myapp.utils");
        let from = CanonicalPath::new("myapp/main.py");

        let result = resolver.resolve(&import, &from, &known, None);
        assert_eq!(result, Some(CanonicalPath::new("myapp/utils.py")));
    }

    #[test]
    fn no_config_preserves_existing_behavior() {
        let resolver = PythonResolver::new();
        let known = FileSet::from_iter(vec![CanonicalPath::new("myapp/utils.py")]);

        let import = make_import("myapp.utils");
        let from = CanonicalPath::new("myapp/main.py");

        let result = resolver.resolve(&import, &from, &known, None);
        assert_eq!(result, Some(CanonicalPath::new("myapp/utils.py")));
    }

    #[test]
    fn src_layout_fallback_to_standard() {
        // src-layout configured but file is not under src/ — falls through
        let resolver = PythonResolver::with_config(PyProjectConfig {
            src_layout: true,
            package_name: Some("myapp".to_string()),
        });
        let known = FileSet::from_iter(vec![CanonicalPath::new("myapp/utils.py")]);

        let import = make_import("myapp.utils");
        let from = CanonicalPath::new("tests/test_main.py");

        let result = resolver.resolve(&import, &from, &known, None);
        assert_eq!(result, Some(CanonicalPath::new("myapp/utils.py")));
    }
}
