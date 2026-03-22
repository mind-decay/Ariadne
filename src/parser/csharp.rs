use crate::model::workspace::WorkspaceInfo;
use crate::model::{CanonicalPath, FileSet};
use crate::parser::traits::{ImportKind, ImportResolver, LanguageParser, RawExport, RawImport};

/// C# language parser.
struct CSharpParser;

impl LanguageParser for CSharpParser {
    fn language(&self) -> &str {
        "csharp"
    }

    fn extensions(&self) -> &[&str] {
        &["cs"]
    }

    fn tree_sitter_language(&self) -> tree_sitter::Language {
        tree_sitter::Language::from(tree_sitter_c_sharp::LANGUAGE)
    }

    fn extract_imports(&self, tree: &tree_sitter::Tree, source: &[u8]) -> Vec<RawImport> {
        let mut imports = Vec::new();
        let root = tree.root_node();

        collect_using_directives(&root, source, &mut imports);

        imports
    }

    fn extract_exports(&self, tree: &tree_sitter::Tree, source: &[u8]) -> Vec<RawExport> {
        let mut exports = Vec::new();
        let root = tree.root_node();

        collect_public_declarations(&root, source, &mut exports);

        exports
    }
}

/// Recursively collect using directives from the tree.
fn collect_using_directives(node: &tree_sitter::Node, source: &[u8], imports: &mut Vec<RawImport>) {
    for i in 0..node.child_count() {
        let child = match node.child(i) {
            Some(c) => c,
            None => continue,
        };

        match child.kind() {
            "using_directive" => {
                if let Some(raw) = extract_using_directive(&child, source) {
                    imports.push(raw);
                }
            }
            "global_statement" => {
                // global using directives may be wrapped
                collect_using_directives(&child, source, imports);
            }
            "namespace_declaration" | "file_scoped_namespace_declaration" => {
                // Using directives can appear inside namespace declarations
                collect_using_directives(&child, source, imports);
            }
            "declaration_list" => {
                collect_using_directives(&child, source, imports);
            }
            _ => {}
        }
    }
}

/// Extract a using directive into a RawImport.
/// Handles:
///   using Namespace;
///   using static Namespace.Class;
///   using Alias = Namespace;
///   global using Namespace;
fn extract_using_directive(node: &tree_sitter::Node, source: &[u8]) -> Option<RawImport> {
    let text = node.utf8_text(source).unwrap_or("").trim().to_string();

    // Skip if it doesn't look like a using directive
    if !text.contains("using") {
        return None;
    }

    // Remove trailing semicolon
    let text = text.trim_end_matches(';').trim();

    // Strip "global" prefix if present
    let text = text
        .strip_prefix("global")
        .map(|s| s.trim())
        .unwrap_or(text);

    // Strip "using" prefix
    let text = text.strip_prefix("using")?.trim();

    // Check for "static" modifier
    let (is_static, text) = if let Some(rest) = text.strip_prefix("static") {
        (true, rest.trim())
    } else {
        (false, text)
    };

    // Check for alias: "Alias = Namespace"
    let (path, symbols) = if let Some((alias, namespace)) = text.split_once('=') {
        let alias = alias.trim().to_string();
        let namespace = namespace.trim().to_string();
        (namespace, vec![alias])
    } else if is_static {
        (text.to_string(), vec!["static".to_string()])
    } else {
        (text.to_string(), Vec::new())
    };

    if path.is_empty() {
        return None;
    }

    Some(RawImport {
        path,
        symbols,
        is_type_only: false,
        kind: ImportKind::Regular,
    })
}

/// Collect public class/interface/struct/enum declarations.
fn collect_public_declarations(
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
            "class_declaration"
            | "interface_declaration"
            | "struct_declaration"
            | "enum_declaration"
            | "record_declaration" => {
                if is_public(&child, source) {
                    if let Some(name) = find_declaration_name(&child, source) {
                        exports.push(RawExport {
                            name,
                            is_re_export: false,
                            source: None,
                        });
                    }
                }
            }
            "namespace_declaration" | "file_scoped_namespace_declaration" => {
                collect_public_declarations(&child, source, exports);
            }
            "declaration_list" => {
                collect_public_declarations(&child, source, exports);
            }
            _ => {}
        }
    }
}

/// Check if a declaration has a "public" modifier.
fn is_public(node: &tree_sitter::Node, source: &[u8]) -> bool {
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            if child.kind() == "modifier" || child.kind() == "public" {
                let text = child.utf8_text(source).unwrap_or("");
                if text == "public" {
                    return true;
                }
            }
        }
    }
    false
}

/// Find the name identifier in a type declaration.
fn find_declaration_name(node: &tree_sitter::Node, source: &[u8]) -> Option<String> {
    // Try named field first (more reliable)
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

/// C# import resolver.
struct CSharpResolver;

impl ImportResolver for CSharpResolver {
    fn resolve(
        &self,
        import: &RawImport,
        _from_file: &CanonicalPath,
        known_files: &FileSet,
        _workspace: Option<&WorkspaceInfo>,
    ) -> Option<CanonicalPath> {
        let namespace = &import.path;

        // Convert namespace dots to path separators
        let path_from_ns = namespace.replace('.', "/");

        // Try as a direct .cs file
        let direct = CanonicalPath::new(format!("{}.cs", path_from_ns));
        if known_files.contains(&direct) {
            return Some(direct);
        }

        // Try with the last segment as the filename within the namespace directory
        // e.g., MyApp.Services.FooService -> MyApp/Services/FooService.cs
        // Already covered above, but also try the directory pattern:
        // namespace may map to a directory containing files
        let dir_prefix = format!("{}/", path_from_ns);
        for file in known_files.iter() {
            let file_str = file.as_str();
            if file_str.starts_with(&dir_prefix) && file_str.ends_with(".cs") {
                let remainder = &file_str[dir_prefix.len()..];
                if !remainder.contains('/') {
                    return Some(file.clone());
                }
            }
        }

        None
    }
}

pub(crate) fn parser() -> Box<dyn LanguageParser> {
    Box::new(CSharpParser)
}

pub(crate) fn resolver() -> Box<dyn ImportResolver> {
    Box::new(CSharpResolver)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::traits::LanguageParser;

    fn parse(source: &str) -> tree_sitter::Tree {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter::Language::from(tree_sitter_c_sharp::LANGUAGE))
            .unwrap();
        parser.parse(source, None).unwrap()
    }

    fn cs_imports(source: &str) -> Vec<RawImport> {
        let tree = parse(source);
        CSharpParser.extract_imports(&tree, source.as_bytes())
    }

    fn cs_exports(source: &str) -> Vec<RawExport> {
        let tree = parse(source);
        CSharpParser.extract_exports(&tree, source.as_bytes())
    }

    // ---- Import tests ----

    #[test]
    fn using_namespace() {
        let source = "using System.Collections.Generic;";
        let result = cs_imports(source);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].path, "System.Collections.Generic");
        assert!(result[0].symbols.is_empty());
    }

    #[test]
    fn using_static() {
        let source = "using static System.Math;";
        let result = cs_imports(source);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].path, "System.Math");
        assert!(result[0].symbols.contains(&"static".to_string()));
    }

    #[test]
    fn using_alias() {
        let source = "using MyList = System.Collections.Generic.List<int>;";
        let result = cs_imports(source);
        assert_eq!(result.len(), 1);
        // The alias name should be in symbols, the namespace in path
        assert!(result[0].symbols.contains(&"MyList".to_string()));
    }

    #[test]
    fn multiple_usings() {
        let source = r#"
using System;
using System.IO;
using System.Linq;
"#;
        let result = cs_imports(source);
        assert_eq!(result.len(), 3);
    }

    #[test]
    fn using_inside_namespace() {
        let source = r#"
namespace MyApp {
    using System.Linq;
    public class Foo {}
}
"#;
        let result = cs_imports(source);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].path, "System.Linq");
    }

    #[test]
    fn empty_source_no_imports() {
        let result = cs_imports("");
        assert!(result.is_empty());
    }

    #[test]
    fn malformed_no_crash() {
        let result = cs_imports("using ;");
        let _ = result;
    }

    // ---- Export tests ----

    #[test]
    fn public_class_exported() {
        let source = r#"
namespace MyApp {
    public class MyService {}
}
"#;
        let result = cs_exports(source);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "MyService");
    }

    #[test]
    fn public_interface_exported() {
        let source = r#"
namespace MyApp {
    public interface IRepository {}
}
"#;
        let result = cs_exports(source);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "IRepository");
    }

    #[test]
    fn internal_class_not_exported() {
        let source = r#"
namespace MyApp {
    class InternalClass {}
}
"#;
        let result = cs_exports(source);
        assert!(result.is_empty());
    }

    #[test]
    fn empty_source_no_exports() {
        let result = cs_exports("");
        assert!(result.is_empty());
    }
}
