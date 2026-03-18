use crate::model::workspace::WorkspaceInfo;
use crate::model::{CanonicalPath, FileSet};
use crate::parser::traits::{ImportKind, ImportResolver, LanguageParser, RawExport, RawImport};

/// Java language parser.
struct JavaParser;

impl LanguageParser for JavaParser {
    fn language(&self) -> &str {
        "java"
    }

    fn extensions(&self) -> &[&str] {
        &["java"]
    }

    fn tree_sitter_language(&self) -> tree_sitter::Language {
        tree_sitter::Language::from(tree_sitter_java::LANGUAGE)
    }

    fn extract_imports(&self, tree: &tree_sitter::Tree, source: &[u8]) -> Vec<RawImport> {
        let mut imports = Vec::new();
        let root = tree.root_node();

        for i in 0..root.child_count() {
            let node = match root.child(i) {
                Some(n) => n,
                None => continue,
            };

            if node.kind() != "import_declaration" {
                continue;
            }

            let text = node.utf8_text(source).unwrap_or("").trim().to_string();

            // Remove trailing semicolon
            let text = text.trim_end_matches(';').trim();

            // Strip "import" prefix
            let text = match text.strip_prefix("import") {
                Some(t) => t.trim(),
                None => continue,
            };

            // Check for "static" modifier
            let (is_static, text) = if let Some(rest) = text.strip_prefix("static") {
                (true, rest.trim())
            } else {
                (false, text)
            };

            if text.is_empty() {
                continue;
            }

            let symbols = if is_static {
                vec!["static".to_string()]
            } else {
                Vec::new()
            };

            imports.push(RawImport {
                path: text.to_string(),
                symbols,
                is_type_only: false,
                kind: ImportKind::Regular,
            });
        }

        imports
    }

    fn extract_exports(&self, tree: &tree_sitter::Tree, source: &[u8]) -> Vec<RawExport> {
        let mut exports = Vec::new();
        let root = tree.root_node();

        collect_public_java_declarations(&root, source, &mut exports);

        exports
    }
}

/// Collect public class/interface/enum declarations from the AST.
fn collect_public_java_declarations(
    node: &tree_sitter::Node,
    source: &[u8],
    exports: &mut Vec<RawExport>,
) {
    for i in 0..node.child_count() {
        let child = match node.child(i) {
            Some(c) => c,
            None => continue,
        };

        match child.kind() {
            "class_declaration" | "interface_declaration" | "enum_declaration"
            | "record_declaration" | "annotation_type_declaration" => {
                if has_public_modifier(&child, source) {
                    if let Some(name) = find_java_declaration_name(&child, source) {
                        exports.push(RawExport {
                            name,
                            is_re_export: false,
                            source: None,
                        });
                    }
                }
            }
            "program" => {
                collect_public_java_declarations(&child, source, exports);
            }
            _ => {}
        }
    }
}

/// Check if a declaration node has a "public" modifier.
fn has_public_modifier(node: &tree_sitter::Node, _source: &[u8]) -> bool {
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            if child.kind() == "modifiers" {
                // "public" is an anonymous node inside modifiers
                for j in 0..child.child_count() {
                    if let Some(modifier) = child.child(j) {
                        if modifier.kind() == "public" {
                            return true;
                        }
                    }
                }
            }
        }
    }
    false
}

/// Find the name identifier in a Java type declaration.
fn find_java_declaration_name(node: &tree_sitter::Node, source: &[u8]) -> Option<String> {
    // Try named field first
    if let Some(name_node) = node.child_by_field_name("name") {
        return Some(name_node.utf8_text(source).unwrap_or("").to_string());
    }
    // Fallback: iterate children
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            if child.kind() == "identifier" {
                return Some(child.utf8_text(source).unwrap_or("").to_string());
            }
        }
    }
    None
}

/// Java import resolver.
struct JavaResolver;

impl ImportResolver for JavaResolver {
    fn resolve(
        &self,
        import: &RawImport,
        _from_file: &CanonicalPath,
        known_files: &FileSet,
        _workspace: Option<&WorkspaceInfo>,
    ) -> Option<CanonicalPath> {
        let import_path = &import.path;

        // Handle wildcard imports (com.example.*)
        if import_path.ends_with(".*") {
            let package = import_path.trim_end_matches(".*");
            let dir = package.replace('.', "/");

            // Try with src/main/java/ prefix
            let prefixed_dir = format!("src/main/java/{}/", dir);
            for file in known_files.iter() {
                let file_str = file.as_str();
                if file_str.starts_with(&prefixed_dir) && file_str.ends_with(".java") {
                    let remainder = &file_str[prefixed_dir.len()..];
                    if !remainder.contains('/') {
                        return Some(file.clone());
                    }
                }
            }

            // Try without prefix
            let plain_dir = format!("{}/", dir);
            for file in known_files.iter() {
                let file_str = file.as_str();
                if file_str.starts_with(&plain_dir) && file_str.ends_with(".java") {
                    let remainder = &file_str[plain_dir.len()..];
                    if !remainder.contains('/') {
                        return Some(file.clone());
                    }
                }
            }

            return None;
        }

        // For static imports, the path includes the member name.
        // e.g., "com.example.Foo.method" — we want "com/example/Foo.java"
        let class_path = if import.symbols.iter().any(|s| s == "static") {
            // Strip the last segment (member name) to get the class
            match import_path.rsplit_once('.') {
                Some((class_part, _)) => class_part,
                None => import_path.as_str(),
            }
        } else {
            import_path.as_str()
        };

        let file_path = class_path.replace('.', "/");

        // Try with src/main/java/ prefix
        let prefixed = CanonicalPath::new(format!("src/main/java/{}.java", file_path));
        if known_files.contains(&prefixed) {
            return Some(prefixed);
        }

        // Try without prefix
        let plain = CanonicalPath::new(format!("{}.java", file_path));
        if known_files.contains(&plain) {
            return Some(plain);
        }

        None
    }
}

pub(crate) fn parser() -> Box<dyn LanguageParser> {
    Box::new(JavaParser)
}

pub(crate) fn resolver() -> Box<dyn ImportResolver> {
    Box::new(JavaResolver)
}
