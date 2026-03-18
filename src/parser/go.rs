use crate::model::workspace::WorkspaceInfo;
use crate::model::{CanonicalPath, FileSet};
use crate::parser::traits::{ImportKind, ImportResolver, LanguageParser, RawExport, RawImport};

/// Go language parser.
struct GoParser;

impl LanguageParser for GoParser {
    fn language(&self) -> &str {
        "go"
    }

    fn extensions(&self) -> &[&str] {
        &["go"]
    }

    fn tree_sitter_language(&self) -> tree_sitter::Language {
        tree_sitter::Language::from(tree_sitter_go::LANGUAGE)
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

            // Single import: import "fmt" → one import_spec child
            // Grouped import: import (\n"fmt"\n"os"\n) → one import_spec_list child
            for j in 0..node.child_count() {
                let child = match node.child(j) {
                    Some(c) => c,
                    None => continue,
                };

                match child.kind() {
                    "import_spec" => {
                        if let Some(raw) = extract_go_import_spec(&child, source) {
                            imports.push(raw);
                        }
                    }
                    "import_spec_list" => {
                        for k in 0..child.child_count() {
                            if let Some(spec) = child.child(k) {
                                if spec.kind() == "import_spec" {
                                    if let Some(raw) = extract_go_import_spec(&spec, source) {
                                        imports.push(raw);
                                    }
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        imports
    }

    fn extract_exports(&self, _tree: &tree_sitter::Tree, _source: &[u8]) -> Vec<RawExport> {
        // Go doesn't have file-level exports in the same way as other languages.
        // Exported symbols are determined by capitalization, not declarations.
        Vec::new()
    }
}

/// Extract a single Go import spec node into a RawImport.
fn extract_go_import_spec(node: &tree_sitter::Node, source: &[u8]) -> Option<RawImport> {
    // Use named fields: "path" (required) and "name" (optional alias/dot/blank)
    let path_node = node.child_by_field_name("path")?;
    let path = strip_go_quotes(&node_text(&path_node, source));
    if path.is_empty() {
        return None;
    }

    let symbols = if let Some(name_node) = node.child_by_field_name("name") {
        let name_text = node_text(&name_node, source);
        match name_text.as_str() {
            "." => vec![".".to_string()],
            "_" => vec!["_".to_string()],
            other => vec![other.to_string()],
        }
    } else {
        Vec::new()
    };

    Some(RawImport {
        path,
        symbols,
        is_type_only: false,
        kind: ImportKind::Regular,
    })
}

fn node_text(node: &tree_sitter::Node, source: &[u8]) -> String {
    node.utf8_text(source).unwrap_or("").to_string()
}

fn strip_go_quotes(s: &str) -> String {
    let s = s.trim();
    if (s.starts_with('"') && s.ends_with('"'))
        || (s.starts_with('`') && s.ends_with('`'))
    {
        s[1..s.len() - 1].to_string()
    } else {
        s.to_string()
    }
}

/// Go import resolver.
struct GoResolver;

impl GoResolver {
    /// Try to find the module path from go.mod by scanning known_files.
    fn find_module_path(known_files: &FileSet) -> Option<String> {
        // Look for go.mod in known_files
        for file in known_files.iter() {
            if file.file_name() == "go.mod" {
                // We can't read file contents from FileSet, so we just know it exists.
                // The module path discovery would require reading go.mod content,
                // which we don't have access to here. Return None — the pipeline
                // would need to supply this separately in the future.
                return None;
            }
        }
        None
    }

    /// Check if an import path looks like a standard library import.
    /// Standard library imports don't contain dots (e.g., "fmt", "net/http").
    fn is_stdlib(import_path: &str) -> bool {
        !import_path.contains('.')
    }

    /// Check if an import path is external (contains dots but doesn't match module path).
    fn is_external(import_path: &str, _module_path: Option<&str>) -> bool {
        if !import_path.contains('.') {
            return false; // stdlib, not external
        }
        // If we have a module path, check if import starts with it
        if let Some(mp) = _module_path {
            if import_path.starts_with(mp) {
                return false; // internal
            }
        }
        true // has dots but doesn't match module path
    }
}

impl ImportResolver for GoResolver {
    fn resolve(
        &self,
        import: &RawImport,
        _from_file: &CanonicalPath,
        known_files: &FileSet,
        _workspace: Option<&WorkspaceInfo>,
    ) -> Option<CanonicalPath> {
        let import_path = &import.path;

        // Skip standard library imports
        if Self::is_stdlib(import_path) {
            return None;
        }

        let module_path = Self::find_module_path(known_files);

        // Skip external imports
        if Self::is_external(import_path, module_path.as_deref()) {
            return None;
        }

        // For internal imports: strip the module prefix and resolve
        let relative = match &module_path {
            Some(mp) => import_path
                .strip_prefix(mp)
                .and_then(|s| s.strip_prefix('/'))
                .unwrap_or(import_path),
            None => import_path,
        };

        // Go imports are directory-based. Find any .go file in that directory
        // within known_files.
        let dir_prefix = format!("{}/", relative);

        // First, try to find any file that starts with this directory prefix
        for file in known_files.iter() {
            let file_str = file.as_str();
            if file_str.starts_with(&dir_prefix) && file_str.ends_with(".go") {
                // Only match direct children (no deeper subdirectories)
                let remainder = &file_str[dir_prefix.len()..];
                if !remainder.contains('/') {
                    return Some(file.clone());
                }
            }
        }

        // Also try: the import path itself as a .go file (less common but possible)
        let direct = CanonicalPath::new(format!("{}.go", relative));
        if known_files.contains(&direct) {
            return Some(direct);
        }

        None
    }
}

pub(crate) fn parser() -> Box<dyn LanguageParser> {
    Box::new(GoParser)
}

pub(crate) fn resolver() -> Box<dyn ImportResolver> {
    Box::new(GoResolver)
}
