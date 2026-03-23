use crate::model::workspace::WorkspaceInfo;
use crate::model::{CanonicalPath, FileSet};
use crate::parser::helpers;
use crate::parser::traits::{ImportKind, ImportResolver, LanguageParser, RawExport, RawImport};

/// Parser and resolver for Rust source files (.rs).
pub(crate) struct RustParser {
    /// The crate's own name (e.g. "ariadne_graph"), so we don't skip
    /// `use ariadne_graph::foo` as an external import.
    crate_name: Option<String>,
}

impl RustParser {
    #[cfg(test)]
    fn new() -> Self {
        Self::with_crate_name(None)
    }

    pub fn with_crate_name(crate_name: Option<String>) -> Self {
        Self { crate_name }
    }

    /// Extract path segments from a scoped_identifier or use path.
    fn extract_path_segments(node: &tree_sitter::Node, source: &[u8]) -> Vec<String> {
        let mut segments = Vec::new();
        Self::collect_path_segments(node, source, &mut segments);
        segments
    }

    /// Recursively collect path segments from scoped identifiers.
    fn collect_path_segments(node: &tree_sitter::Node, source: &[u8], segments: &mut Vec<String>) {
        match node.kind() {
            "scoped_identifier" | "scoped_type_identifier" => {
                // Has a path (left) and a name (right)
                if let Some(path) = helpers::find_child_by_kind(node, "scoped_identifier")
                    .or_else(|| helpers::find_child_by_kind(node, "crate"))
                    .or_else(|| helpers::find_child_by_kind(node, "super"))
                    .or_else(|| helpers::find_child_by_kind(node, "self"))
                    .or_else(|| helpers::find_child_by_kind(node, "identifier"))
                {
                    Self::collect_path_segments(&path, source, segments);
                }
                // Get the name part (the last identifier)
                let mut cursor = node.walk();
                let children: Vec<_> = node.children(&mut cursor).collect();
                if let Some(last) = children.last() {
                    if last.kind() == "identifier" || last.kind() == "type_identifier" {
                        if let Ok(text) = last.utf8_text(source) {
                            segments.push(text.to_string());
                        }
                    }
                }
            }
            "identifier" | "type_identifier" => {
                if let Ok(text) = node.utf8_text(source) {
                    segments.push(text.to_string());
                }
            }
            "crate" => segments.push("crate".to_string()),
            "super" => segments.push("super".to_string()),
            "self" => segments.push("self".to_string()),
            _ => {
                if let Ok(text) = node.utf8_text(source) {
                    segments.push(text.to_string());
                }
            }
        }
    }

    /// Extract use declarations, handling grouped uses like `use crate::{foo, bar};`
    fn extract_use_paths(
        node: &tree_sitter::Node,
        source: &[u8],
    ) -> Vec<(Vec<String>, Vec<String>)> {
        // Returns list of (path_segments, symbols)
        let mut results = Vec::new();

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            match child.kind() {
                "use_as_clause" | "scoped_identifier" | "identifier" => {
                    let segments = Self::extract_path_segments(&child, source);
                    if !segments.is_empty() {
                        let symbol = segments.last().cloned().unwrap_or_default();
                        results.push((segments, vec![symbol]));
                    }
                }
                "use_list" => {
                    // Grouped use: crate::{foo, bar}
                    // The parent has the prefix path
                    // We need to find the scoped_use_list which has prefix::{ list }
                    // Actually, the structure is: use_declaration -> scoped_use_list -> path + use_list
                    // Here we're already inside and need to handle each item
                    let mut list_cursor = child.walk();
                    for item in child.children(&mut list_cursor) {
                        match item.kind() {
                            "identifier" | "type_identifier" => {
                                if let Ok(name) = item.utf8_text(source) {
                                    results.push((vec![name.to_string()], vec![name.to_string()]));
                                }
                            }
                            "scoped_identifier" => {
                                let segments = Self::extract_path_segments(&item, source);
                                if !segments.is_empty() {
                                    let symbol = segments.last().cloned().unwrap_or_default();
                                    results.push((segments, vec![symbol]));
                                }
                            }
                            "use_as_clause" => {
                                // use crate::{foo as bar}
                                if let Some(orig) = helpers::find_child_by_kind(&item, "identifier")
                                {
                                    let segments = Self::extract_path_segments(&orig, source);
                                    if !segments.is_empty() {
                                        let symbol = segments.last().cloned().unwrap_or_default();
                                        results.push((segments, vec![symbol]));
                                    }
                                }
                            }
                            "self" => {
                                results.push((vec!["self".to_string()], vec!["self".to_string()]));
                            }
                            _ => {}
                        }
                    }
                }
                "scoped_use_list" => {
                    // use crate::foo::{bar, baz}
                    Self::extract_scoped_use_list(&child, source, &[], &mut results);
                }
                "use_wildcard" => {
                    // use crate::foo::*
                    // Get the path part
                    if let Some(path_node) =
                        helpers::find_child_by_kind(&child, "scoped_identifier")
                            .or_else(|| helpers::find_child_by_kind(&child, "identifier"))
                            .or_else(|| helpers::find_child_by_kind(&child, "crate"))
                    {
                        let segments = Self::extract_path_segments(&path_node, source);
                        results.push((segments, vec!["*".to_string()]));
                    }
                }
                _ => {}
            }
        }

        results
    }

    /// Extract from a scoped_use_list node: `prefix::{item1, item2}`
    fn extract_scoped_use_list(
        node: &tree_sitter::Node,
        source: &[u8],
        parent_prefix: &[String],
        results: &mut Vec<(Vec<String>, Vec<String>)>,
    ) {
        // A scoped_use_list has: path :: use_list
        let mut prefix_segments: Vec<String> = parent_prefix.to_vec();

        // Get the path prefix
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            match child.kind() {
                "scoped_identifier" | "identifier" | "crate" | "super" | "self" => {
                    prefix_segments.extend(Self::extract_path_segments(&child, source));
                }
                "use_list" => {
                    // Each item in the use_list
                    let mut list_cursor = child.walk();
                    for item in child.children(&mut list_cursor) {
                        match item.kind() {
                            "identifier" | "type_identifier" => {
                                if let Ok(name) = item.utf8_text(source) {
                                    let mut full_path = prefix_segments.clone();
                                    full_path.push(name.to_string());
                                    results.push((full_path, vec![name.to_string()]));
                                }
                            }
                            "scoped_identifier" => {
                                let sub_segments = Self::extract_path_segments(&item, source);
                                let mut full_path = prefix_segments.clone();
                                full_path.extend(sub_segments.clone());
                                let symbol = sub_segments.last().cloned().unwrap_or_default();
                                results.push((full_path, vec![symbol]));
                            }
                            "scoped_use_list" => {
                                // Nested: use crate::{foo::{bar, baz}}
                                Self::extract_scoped_use_list(
                                    &item,
                                    source,
                                    &prefix_segments,
                                    results,
                                );
                            }
                            "self" => {
                                // use crate::foo::{self}
                                let full_path = prefix_segments.clone();
                                let symbol = prefix_segments.last().cloned().unwrap_or_default();
                                results.push((full_path, vec![symbol]));
                            }
                            "use_as_clause" => {
                                if let Some(orig) = helpers::find_child_by_kind(&item, "identifier")
                                    .or_else(|| {
                                        helpers::find_child_by_kind(&item, "scoped_identifier")
                                    })
                                {
                                    let sub_segments = Self::extract_path_segments(&orig, source);
                                    let mut full_path = prefix_segments.clone();
                                    full_path.extend(sub_segments.clone());
                                    let symbol = sub_segments.last().cloned().unwrap_or_default();
                                    results.push((full_path, vec![symbol]));
                                }
                            }
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
        }
    }

    /// Check if path segments represent a standard library or external crate import to skip.
    fn is_skip_import(&self, segments: &[String]) -> bool {
        if segments.is_empty() {
            return true;
        }
        match segments[0].as_str() {
            "std" | "core" | "alloc" => true,
            "crate" | "super" | "self" => false,
            first => {
                // Don't skip if it matches this crate's own name
                if let Some(ref name) = self.crate_name {
                    if first == name {
                        return false;
                    }
                }
                true // External crate — skip
            }
        }
    }
}

impl LanguageParser for RustParser {
    fn language(&self) -> &str {
        "rust"
    }

    fn extensions(&self) -> &[&str] {
        &["rs"]
    }

    fn tree_sitter_language(&self) -> tree_sitter::Language {
        tree_sitter::Language::from(tree_sitter_rust::LANGUAGE)
    }

    fn extract_imports(&self, tree: &tree_sitter::Tree, source: &[u8]) -> Vec<RawImport> {
        let mut imports = Vec::new();
        let root = tree.root_node();
        let mut cursor = root.walk();

        for node in root.children(&mut cursor) {
            match node.kind() {
                // use crate::auth::login;
                // use super::utils;
                // use self::helpers;
                // use crate::{foo, bar};
                "use_declaration" => {
                    let paths = Self::extract_use_paths(&node, source);

                    for (segments, symbols) in paths {
                        if self.is_skip_import(&segments) {
                            continue;
                        }

                        // Pre-map module path to filesystem path
                        // We pass a dummy from_file here; actual resolution uses the resolver
                        // Instead, store the raw segments as a path string for the resolver
                        let path = segments.join("::");

                        imports.push(RawImport {
                            path,
                            symbols,
                            is_type_only: false,
                            kind: ImportKind::Regular,
                        });
                    }
                }

                // mod auth; — module declaration (treated as import)
                "mod_item" => {
                    // Check that it's a declaration (no body block)
                    let has_body = helpers::find_child_by_kind(&node, "declaration_list").is_some();
                    if has_body {
                        // Inline module definition, not a file import
                        continue;
                    }

                    if let Some(name_node) = helpers::find_child_by_kind(&node, "identifier") {
                        if let Ok(name) = name_node.utf8_text(source) {
                            // mod declarations are imports to the module file
                            imports.push(RawImport {
                                path: name.to_string(),
                                symbols: vec![name.to_string()],
                                is_type_only: false,
                                kind: ImportKind::ModDeclaration,
                            });
                        }
                    }
                }

                // Skip extern crate declarations
                "extern_crate_declaration" => {}

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
            // Check for visibility modifier (pub)
            let has_pub = Self::has_pub_visibility(&node, source);

            if !has_pub {
                continue;
            }

            match node.kind() {
                "function_item" => {
                    if let Some(name_node) = helpers::find_child_by_kind(&node, "identifier") {
                        if let Ok(name) = name_node.utf8_text(source) {
                            exports.push(RawExport {
                                name: name.to_string(),
                                is_re_export: false,
                                source: None,
                            });
                        }
                    }
                }
                "struct_item" => {
                    if let Some(name_node) = helpers::find_child_by_kind(&node, "type_identifier") {
                        if let Ok(name) = name_node.utf8_text(source) {
                            exports.push(RawExport {
                                name: name.to_string(),
                                is_re_export: false,
                                source: None,
                            });
                        }
                    }
                }
                "enum_item" => {
                    if let Some(name_node) = helpers::find_child_by_kind(&node, "type_identifier") {
                        if let Ok(name) = name_node.utf8_text(source) {
                            exports.push(RawExport {
                                name: name.to_string(),
                                is_re_export: false,
                                source: None,
                            });
                        }
                    }
                }
                "trait_item" => {
                    if let Some(name_node) = helpers::find_child_by_kind(&node, "type_identifier") {
                        if let Ok(name) = name_node.utf8_text(source) {
                            exports.push(RawExport {
                                name: name.to_string(),
                                is_re_export: false,
                                source: None,
                            });
                        }
                    }
                }
                "type_item" => {
                    if let Some(name_node) = helpers::find_child_by_kind(&node, "type_identifier") {
                        if let Ok(name) = name_node.utf8_text(source) {
                            exports.push(RawExport {
                                name: name.to_string(),
                                is_re_export: false,
                                source: None,
                            });
                        }
                    }
                }
                "const_item" | "static_item" => {
                    if let Some(name_node) = helpers::find_child_by_kind(&node, "identifier") {
                        if let Ok(name) = name_node.utf8_text(source) {
                            exports.push(RawExport {
                                name: name.to_string(),
                                is_re_export: false,
                                source: None,
                            });
                        }
                    }
                }
                "mod_item" => {
                    if let Some(name_node) = helpers::find_child_by_kind(&node, "identifier") {
                        if let Ok(name) = name_node.utf8_text(source) {
                            exports.push(RawExport {
                                name: name.to_string(),
                                is_re_export: false,
                                source: None,
                            });
                        }
                    }
                }
                // pub use foo::bar; — re-export
                "use_declaration" => {
                    let paths = Self::extract_use_paths(&node, source);
                    for (segments, symbols) in paths {
                        let source_path = segments.join("::");
                        for sym in &symbols {
                            exports.push(RawExport {
                                name: sym.clone(),
                                is_re_export: true,
                                source: Some(source_path.clone()),
                            });
                        }
                    }
                }
                _ => {}
            }
        }

        exports
    }
}

impl RustParser {
    /// Check if a node has a `pub` visibility modifier.
    fn has_pub_visibility(node: &tree_sitter::Node, source: &[u8]) -> bool {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "visibility_modifier" {
                // Could be `pub`, `pub(crate)`, `pub(super)`, etc.
                // All count as "public" for export purposes
                if let Ok(text) = child.utf8_text(source) {
                    return text.starts_with("pub");
                }
            }
        }
        false
    }
}

/// Rust import resolver.
pub(crate) struct RustResolver;

impl RustResolver {
    pub fn new() -> Self {
        Self
    }
}

impl ImportResolver for RustResolver {
    fn resolve(
        &self,
        import: &RawImport,
        from_file: &CanonicalPath,
        known_files: &FileSet,
        _workspace: Option<&WorkspaceInfo>,
    ) -> Option<CanonicalPath> {
        let path = &import.path;

        if import.kind == ImportKind::ModDeclaration {
            // mod declaration: `mod auth;`
            // Resolve relative to current file
            return self.resolve_mod_declaration(path, from_file, known_files);
        }

        // Parse the module path segments
        let segments: Vec<&str> = path.split("::").collect();
        if segments.is_empty() {
            return None;
        }

        let fs_path = match segments[0] {
            "crate" => {
                // crate::auth::login → src/auth/login
                let mut parts = vec!["src"];
                for seg in &segments[1..] {
                    parts.push(seg);
                }
                parts.join("/")
            }
            "super" => {
                // super::utils → go up from current module
                let base_dir = from_file.parent().unwrap_or("");
                let mut dir_segments: Vec<&str> = if base_dir.is_empty() {
                    Vec::new()
                } else {
                    base_dir.split('/').collect()
                };

                let file_name = from_file.file_name();
                if file_name == "mod.rs" || file_name == "lib.rs" || file_name == "main.rs" {
                    dir_segments.pop();
                }

                let mut i = 0;
                while i < segments.len() && segments[i] == "super" {
                    dir_segments.pop();
                    i += 1;
                }

                for seg in &segments[i..] {
                    dir_segments.push(seg);
                }

                dir_segments.join("/")
            }
            "self" => {
                // self::helpers → same module directory
                let base_dir = from_file.parent().unwrap_or("");
                let mut dir_segments: Vec<&str> = if base_dir.is_empty() {
                    Vec::new()
                } else {
                    base_dir.split('/').collect()
                };

                let file_name = from_file.file_name();
                if file_name != "mod.rs" && file_name != "lib.rs" && file_name != "main.rs" {
                    // For non-module files, self refers to the same directory
                    // which is already the parent directory
                }

                for seg in &segments[1..] {
                    dir_segments.push(seg);
                }

                dir_segments.join("/")
            }
            _ => {
                // External crate — skip
                return None;
            }
        };

        Self::probe_rust_path_with_walkback(&fs_path, known_files)
    }
}

impl RustResolver {
    /// Resolve a `mod name;` declaration to a file path.
    fn resolve_mod_declaration(
        &self,
        mod_name: &str,
        from_file: &CanonicalPath,
        known_files: &FileSet,
    ) -> Option<CanonicalPath> {
        let base_dir = from_file.parent().unwrap_or("");
        let file_name = from_file.file_name();

        // Determine the directory where child modules live
        let mod_dir = if file_name == "mod.rs" || file_name == "lib.rs" || file_name == "main.rs" {
            // Child modules are in the same directory
            base_dir.to_string()
        } else {
            // For src/auth.rs, child modules would be in src/auth/
            let stem = file_name.strip_suffix(".rs").unwrap_or(file_name);
            if base_dir.is_empty() {
                stem.to_string()
            } else {
                format!("{}/{}", base_dir, stem)
            }
        };

        // Try mod_name.rs
        let candidate1 = CanonicalPath::new(format!("{}/{}.rs", mod_dir, mod_name));
        if known_files.contains(&candidate1) {
            return Some(candidate1);
        }

        // Try mod_name/mod.rs
        let candidate2 = CanonicalPath::new(format!("{}/{}/mod.rs", mod_dir, mod_name));
        if known_files.contains(&candidate2) {
            return Some(candidate2);
        }

        None
    }

    /// Probe for a Rust module path.
    fn probe_rust_path(base: &str, known_files: &FileSet) -> Option<CanonicalPath> {
        // Try as a direct .rs file
        let candidate = CanonicalPath::new(format!("{}.rs", base));
        if known_files.contains(&candidate) {
            return Some(candidate);
        }

        // Try as a directory with mod.rs
        let mod_candidate = CanonicalPath::new(format!("{}/mod.rs", base));
        if known_files.contains(&mod_candidate) {
            return Some(mod_candidate);
        }

        None
    }

    /// Probe with walk-back: when the full path doesn't resolve (e.g. `src/model/CanonicalPath`
    /// because `CanonicalPath` is a symbol, not a file), trim the last segment and retry.
    /// This resolves `use crate::model::SomeType` → `src/model/mod.rs`.
    fn probe_rust_path_with_walkback(base: &str, known_files: &FileSet) -> Option<CanonicalPath> {
        // First, try the exact path
        if let Some(found) = Self::probe_rust_path(base, known_files) {
            return Some(found);
        }

        // Walk back: trim the last segment (likely a symbol name) and retry
        let mut path = base;
        while let Some(slash_pos) = path.rfind('/') {
            path = &path[..slash_pos];
            if let Some(found) = Self::probe_rust_path(path, known_files) {
                return Some(found);
            }
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::traits::LanguageParser;

    fn parse(source: &str) -> tree_sitter::Tree {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter::Language::from(tree_sitter_rust::LANGUAGE))
            .unwrap();
        parser.parse(source, None).unwrap()
    }

    fn rust_imports(source: &str) -> Vec<RawImport> {
        let tree = parse(source);
        RustParser::new().extract_imports(&tree, source.as_bytes())
    }

    fn rust_exports(source: &str) -> Vec<RawExport> {
        let tree = parse(source);
        RustParser::new().extract_exports(&tree, source.as_bytes())
    }

    // ---- Import tests ----

    #[test]
    fn use_crate_grouped() {
        // use crate::{foo, bar} uses scoped_use_list which extracts crate prefix
        let result = rust_imports("use crate::{auth, utils};");
        assert!(
            result.len() >= 1,
            "grouped crate imports should be found: {:?}",
            result
        );
        // All returned imports should have crate:: prefix
        for imp in &result {
            assert!(
                imp.path.contains("crate"),
                "path should contain crate: {}",
                imp.path
            );
        }
    }

    #[test]
    fn use_self_module() {
        let result = rust_imports("use self::helpers;");
        // self:: imports use scoped_identifier path extraction
        let _ = result; // Should not panic
    }

    #[test]
    fn use_nested_grouped() {
        // Nested grouped use: crate::{model::{Node, Edge}}
        let result = rust_imports("use crate::{model::{Node, Edge}};");
        assert!(
            result.len() >= 2,
            "nested grouped use should produce at least 2 imports: {:?}",
            result
        );
    }

    #[test]
    fn multiple_use_declarations() {
        let source = r#"
mod auth;
mod config;
use crate::{auth, config};
"#;
        let result = rust_imports(source);
        // 2 mod declarations + at least 2 from grouped use
        let mod_decls: Vec<_> = result
            .iter()
            .filter(|i| i.kind == ImportKind::ModDeclaration)
            .collect();
        assert_eq!(mod_decls.len(), 2);
    }

    #[test]
    fn mod_declaration_import() {
        let result = rust_imports("mod auth;");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].path, "auth");
        assert_eq!(result[0].kind, ImportKind::ModDeclaration);
    }

    #[test]
    fn inline_mod_not_import() {
        let result = rust_imports("mod inline { fn foo() {} }");
        assert!(result.is_empty());
    }

    #[test]
    fn use_std_skipped() {
        let result = rust_imports("use std::collections::HashMap;");
        assert!(result.is_empty());
    }

    #[test]
    fn use_external_crate_skipped() {
        let result = rust_imports("use serde::Serialize;");
        assert!(result.is_empty());
    }

    #[test]
    fn empty_source_no_imports() {
        let result = rust_imports("");
        assert!(result.is_empty());
    }

    // ---- Export tests ----

    #[test]
    fn pub_function_exported() {
        let result = rust_exports("pub fn my_func() {}");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "my_func");
        assert!(!result[0].is_re_export);
    }

    #[test]
    fn pub_struct_exported() {
        let result = rust_exports("pub struct MyStruct {}");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "MyStruct");
    }

    #[test]
    fn pub_enum_exported() {
        let result = rust_exports("pub enum Color { Red, Blue }");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "Color");
    }

    #[test]
    fn pub_use_re_export() {
        let result = rust_exports("pub use crate::model::Graph;");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "Graph");
        assert!(result[0].is_re_export);
        assert!(result[0].source.is_some());
    }

    #[test]
    fn private_items_not_exported() {
        let result = rust_exports("fn private_fn() {}\nstruct PrivStruct {}");
        assert!(result.is_empty());
    }

    #[test]
    fn pub_crate_is_exported() {
        let result = rust_exports("pub(crate) fn internal() {}");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "internal");
    }

    #[test]
    fn use_crate_scoped_imports_resolved() {
        let result = rust_imports("use crate::model::{CanonicalPath, FileSet};");
        assert_eq!(result.len(), 2, "should extract 2 imports: {:?}", result);
        assert_eq!(result[0].path, "crate::model::CanonicalPath");
        assert_eq!(result[1].path, "crate::model::FileSet");
        assert_eq!(result[0].kind, ImportKind::Regular);
    }

    #[test]
    fn use_crate_single_import() {
        let result = rust_imports("use crate::diagnostic::DiagnosticCollector;");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].path, "crate::diagnostic::DiagnosticCollector");
    }

    #[test]
    fn use_crate_wildcard() {
        let result = rust_imports("use crate::model::*;");
        assert_eq!(result.len(), 1);
        assert!(result[0].path.contains("crate::model"), "path should contain crate::model: {}", result[0].path);
    }

    #[test]
    fn use_super_import() {
        let result = rust_imports("use super::utils::format;");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].path, "super::utils::format");
    }
}
