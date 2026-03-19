use crate::model::workspace::WorkspaceInfo;
use crate::model::{CanonicalPath, FileSet};
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

    /// Find the first child of a given kind.
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
                if let Some(dotted) = Self::find_child_by_kind(&child, "dotted_name") {
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
                    if let Some(name_node) = Self::find_child_by_kind(&child, "dotted_name")
                        .or_else(|| Self::find_child_by_kind(&child, "identifier"))
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
                                    Self::find_child_by_kind(&child, "dotted_name")
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
                if let Some(assignment) = Self::find_child_by_kind(&node, "assignment") {
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
                            Self::find_child_by_kind(&node, "function_definition")
                                .or_else(|| Self::find_child_by_kind(&node, "class_definition"))
                        } else {
                            Some(node)
                        };

                        if let Some(def) = def_node {
                            if let Some(name_node) = Self::find_child_by_kind(&def, "identifier") {
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
                        if let Some(name_node) = Self::find_child_by_kind(&node, "identifier") {
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
        if let Some(block) = Self::find_child_by_kind(node, "block") {
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
                                    Self::find_child_by_kind(&child, "dotted_name")
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

/// Python import resolver.
pub(crate) struct PythonResolver;

impl PythonResolver {
    pub fn new() -> Self {
        Self
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
