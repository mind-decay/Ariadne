use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use crate::model::symbol::{LineSpan, SymbolDef, SymbolKind, Visibility};
use crate::model::workspace::WorkspaceInfo;
use crate::model::{CanonicalPath, FileSet};
use crate::parser::config::csproj::CsprojConfig;
use crate::parser::config::find_nearest_csproj;
use crate::parser::symbols::SymbolExtractor;
use crate::parser::traits::{ImportKind, ImportResolver, LanguageParser, RawExport, RawImport};

/// C# language parser.
pub(crate) struct CSharpParser;

impl LanguageParser for CSharpParser {
    fn language(&self) -> &str {
        "csharp"
    }

    fn extensions(&self) -> &[&str] {
        &["cs", "razor"]
    }

    fn tree_sitter_language(&self) -> tree_sitter::Language {
        tree_sitter::Language::from(tree_sitter_c_sharp::LANGUAGE)
    }

    fn extract_imports(&self, tree: &tree_sitter::Tree, source: &[u8]) -> Vec<RawImport> {
        let source_str = std::str::from_utf8(source).unwrap_or("");

        // Detect .razor content: these files use Razor directives (@using, @inject, etc.)
        // which are not valid C#. tree-sitter C# parser will produce ERROR nodes for them,
        // so we use regex-based extraction instead.
        if is_razor_content(source_str) {
            return extract_razor_imports(source_str);
        }

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

/// Detect whether source content is a .razor file by checking for Razor directives.
///
/// .razor files typically start with @-prefixed directives (@page, @using, @inject, etc.).
/// We check if the first non-empty line starts with '@'.
fn is_razor_content(source: &str) -> bool {
    source
        .lines()
        .find(|line| !line.trim().is_empty())
        .map(|line| line.trim_start().starts_with('@'))
        .unwrap_or(false)
}

/// Extract imports from .razor file content using text-based parsing.
///
/// Handles these Razor directives:
/// - `@using Namespace` — namespace import
/// - `@inject ServiceType PropertyName` — DI injection (import the service type)
/// - `@inherits BaseType` — base type import
fn extract_razor_imports(source: &str) -> Vec<RawImport> {
    let mut imports = Vec::new();
    for line in source.lines() {
        let trimmed = line.trim();
        if let Some(ns) = trimmed.strip_prefix("@using ") {
            let ns = ns.trim().trim_end_matches(';');
            if !ns.is_empty() {
                imports.push(RawImport {
                    path: ns.to_string(),
                    symbols: Vec::new(),
                    is_type_only: false,
                    kind: ImportKind::Regular,
                });
            }
        } else if let Some(rest) = trimmed.strip_prefix("@inject ") {
            // @inject ServiceType PropertyName
            let parts: Vec<&str> = rest.trim().splitn(2, ' ').collect();
            if let Some(&service_type) = parts.first() {
                if !service_type.is_empty() {
                    imports.push(RawImport {
                        path: service_type.to_string(),
                        symbols: Vec::new(),
                        is_type_only: false,
                        kind: ImportKind::Regular,
                    });
                }
            }
        } else if let Some(rest) = trimmed.strip_prefix("@inherits ") {
            let base = rest.trim();
            if !base.is_empty() {
                imports.push(RawImport {
                    path: base.to_string(),
                    symbols: Vec::new(),
                    is_type_only: false,
                    kind: ImportKind::Regular,
                });
            }
        }
    }
    imports
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

impl SymbolExtractor for CSharpParser {
    fn extract_symbols(&self, tree: &tree_sitter::Tree, source: &[u8]) -> Vec<SymbolDef> {
        let mut symbols = Vec::new();
        let root = tree.root_node();
        extract_csharp_symbols_from_node(&root, source, &mut symbols, None);
        symbols
    }
}

/// Recursively extract symbol definitions from a C# AST node.
fn extract_csharp_symbols_from_node(
    node: &tree_sitter::Node,
    source: &[u8],
    symbols: &mut Vec<SymbolDef>,
    parent_name: Option<&str>,
) {
    for i in 0..node.child_count() {
        let child = match node.child(i) {
            Some(c) => c,
            None => continue,
        };

        match child.kind() {
            "class_declaration" => {
                let name = find_declaration_name(&child, source);
                let visibility = csharp_visibility(&child, source);
                if let Some(ref n) = name {
                    symbols.push(SymbolDef {
                        name: n.clone(),
                        kind: SymbolKind::Class,
                        visibility,
                        span: csharp_node_span(&child),
                        signature: csharp_truncate_signature(&child, source, 200),
                        parent: parent_name.map(|s| s.to_string()),
                    });
                }
                // Extract members inside class body
                if let Some(body) = child.child_by_field_name("body") {
                    extract_csharp_symbols_from_node(&body, source, symbols, name.as_deref());
                }
            }
            "struct_declaration" => {
                let name = find_declaration_name(&child, source);
                let visibility = csharp_visibility(&child, source);
                if let Some(ref n) = name {
                    symbols.push(SymbolDef {
                        name: n.clone(),
                        kind: SymbolKind::Struct,
                        visibility,
                        span: csharp_node_span(&child),
                        signature: csharp_truncate_signature(&child, source, 200),
                        parent: parent_name.map(|s| s.to_string()),
                    });
                }
                if let Some(body) = child.child_by_field_name("body") {
                    extract_csharp_symbols_from_node(&body, source, symbols, name.as_deref());
                }
            }
            "interface_declaration" => {
                let name = find_declaration_name(&child, source);
                let visibility = csharp_visibility(&child, source);
                if let Some(ref n) = name {
                    symbols.push(SymbolDef {
                        name: n.clone(),
                        kind: SymbolKind::Interface,
                        visibility,
                        span: csharp_node_span(&child),
                        signature: csharp_truncate_signature(&child, source, 200),
                        parent: parent_name.map(|s| s.to_string()),
                    });
                }
                if let Some(body) = child.child_by_field_name("body") {
                    extract_csharp_symbols_from_node(&body, source, symbols, name.as_deref());
                }
            }
            "method_declaration" => {
                if let Some(name_node) = child.child_by_field_name("name") {
                    if let Ok(name) = name_node.utf8_text(source) {
                        symbols.push(SymbolDef {
                            name: name.to_string(),
                            kind: SymbolKind::Method,
                            visibility: csharp_visibility(&child, source),
                            span: csharp_node_span(&child),
                            signature: csharp_truncate_signature(&child, source, 200),
                            parent: parent_name.map(|s| s.to_string()),
                        });
                    }
                }
            }
            "field_declaration" => {
                // Check for const modifier
                if has_modifier(&child, source, "const") {
                    // Extract variable declarator names
                    extract_csharp_field_names(&child, source, symbols, parent_name, true);
                }
            }
            "namespace_declaration" | "file_scoped_namespace_declaration" => {
                extract_csharp_symbols_from_node(&child, source, symbols, parent_name);
            }
            "declaration_list" => {
                extract_csharp_symbols_from_node(&child, source, symbols, parent_name);
            }
            _ => {}
        }
    }
}

/// Extract field names from a field_declaration.
fn extract_csharp_field_names(
    node: &tree_sitter::Node,
    source: &[u8],
    symbols: &mut Vec<SymbolDef>,
    parent_name: Option<&str>,
    _is_const: bool,
) {
    let visibility = csharp_visibility(node, source);
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            if child.kind() == "variable_declaration" {
                for j in 0..child.child_count() {
                    if let Some(declarator) = child.child(j) {
                        if declarator.kind() == "variable_declarator" {
                            if let Some(name_node) = declarator.child_by_field_name("name") {
                                if let Ok(name) = name_node.utf8_text(source) {
                                    symbols.push(SymbolDef {
                                        name: name.to_string(),
                                        kind: SymbolKind::Const,
                                        visibility,
                                        span: csharp_node_span(node),
                                        signature: csharp_truncate_signature(node, source, 200),
                                        parent: parent_name.map(|s| s.to_string()),
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

/// Determine C# visibility from modifier nodes.
fn csharp_visibility(node: &tree_sitter::Node, source: &[u8]) -> Visibility {
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            if child.kind() == "modifier" || child.kind() == "public" || child.kind() == "private"
                || child.kind() == "protected" || child.kind() == "internal"
            {
                let text = child.utf8_text(source).unwrap_or("");
                match text {
                    "public" => return Visibility::Public,
                    "internal" => return Visibility::Internal,
                    "private" | "protected" => return Visibility::Private,
                    _ => {}
                }
            }
        }
    }
    // Default in C# is internal for top-level, private for members
    Visibility::Internal
}

/// Check if a node has a specific modifier.
fn has_modifier(node: &tree_sitter::Node, source: &[u8], modifier: &str) -> bool {
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            if child.kind() == "modifier" || child.kind() == modifier {
                let text = child.utf8_text(source).unwrap_or("");
                if text == modifier {
                    return true;
                }
            }
        }
    }
    false
}

/// Get LineSpan from a tree-sitter node (1-based).
fn csharp_node_span(node: &tree_sitter::Node) -> LineSpan {
    LineSpan {
        start: node.start_position().row as u32 + 1,
        end: node.end_position().row as u32 + 1,
    }
}

/// Extract first line of node text, truncated to max_len.
fn csharp_truncate_signature(
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

/// C# import resolver with optional .csproj configuration awareness.
struct CSharpResolver {
    /// .csproj configs, keyed by project-relative directory.
    csproj_configs: BTreeMap<PathBuf, CsprojConfig>,
    /// Known NuGet package names (from all .csproj PackageReferences).
    known_packages: BTreeSet<String>,
    /// Known framework namespace prefixes.
    framework_prefixes: &'static [&'static str],
}

impl CSharpResolver {
    fn new() -> Self {
        Self {
            csproj_configs: BTreeMap::new(),
            known_packages: BTreeSet::new(),
            framework_prefixes: &["System", "Microsoft", "Windows", "Mono"],
        }
    }

    fn with_csproj_configs(mut self, configs: BTreeMap<PathBuf, CsprojConfig>) -> Self {
        // Build known_packages from all PackageReferences across all configs
        for config in configs.values() {
            for pkg in &config.package_references {
                self.known_packages.insert(pkg.name.clone());
            }
        }
        self.csproj_configs = configs;
        self
    }

    /// Check if namespace starts with a known framework prefix.
    fn is_framework_namespace(&self, namespace: &str) -> bool {
        let first_segment = namespace.split('.').next().unwrap_or("");
        self.framework_prefixes.contains(&first_segment)
    }

    /// Check if namespace first segment matches a known NuGet package.
    fn is_nuget_package(&self, namespace: &str) -> bool {
        let first_segment = namespace.split('.').next().unwrap_or("");
        self.known_packages.contains(first_segment)
    }

    /// Naive resolution: namespace dots to path separators, scan known_files.
    /// This is the original resolution logic, preserved as fallback.
    fn resolve_naive(
        &self,
        namespace: &str,
        known_files: &FileSet,
    ) -> Option<CanonicalPath> {
        let path_from_ns = namespace.replace('.', "/");

        // Try as a direct .cs file
        let direct = CanonicalPath::new(format!("{}.cs", path_from_ns));
        if known_files.contains(&direct) {
            return Some(direct);
        }

        // Try directory pattern: namespace may map to a directory containing files
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

    /// Config-aware resolution using .csproj data.
    fn resolve_with_config(
        &self,
        namespace: &str,
        from_file: &CanonicalPath,
        known_files: &FileSet,
    ) -> Option<CanonicalPath> {
        // Find the nearest .csproj for the importing file
        let from_dir = Path::new(from_file.as_str())
            .parent()
            .unwrap_or(Path::new(""));
        let nearest_csproj = find_nearest_csproj(from_dir, &self.csproj_configs);

        // Namespace-to-path mapping with root namespace stripping
        if let Some(config) = nearest_csproj {
            // Try with root namespace stripping
            if let Some(ref root_ns) = config.root_namespace {
                if let Some(rest) = namespace.strip_prefix(root_ns) {
                    let stripped = rest.strip_prefix('.').unwrap_or(rest);
                    if !stripped.is_empty() {
                        let relative_path = stripped.replace('.', "/");
                        let candidate_str = format!(
                            "{}/{}.cs",
                            config.project_dir.display(),
                            relative_path
                        );
                        let candidate = CanonicalPath::new(&candidate_str);
                        if known_files.contains(&candidate) {
                            return Some(candidate);
                        }
                    }
                }
            }

            // Try direct namespace-to-path within the project directory
            let ns_path = namespace.replace('.', "/");
            let candidate_str = format!("{}/{}.cs", config.project_dir.display(), ns_path);
            let candidate = CanonicalPath::new(&candidate_str);
            if known_files.contains(&candidate) {
                return Some(candidate);
            }

            // Cross-project resolution via ProjectReference paths
            for proj_ref in &config.project_references {
                if let Some(ref resolved_path) = proj_ref.resolved_path {
                    let ref_dir = resolved_path
                        .parent()
                        .unwrap_or(Path::new(""));
                    // Check if a referenced project's csproj config exists
                    if let Some(ref_config) = self.csproj_configs.get(ref_dir) {
                        // Check if namespace matches referenced project's root namespace
                        let matches_ref = ref_config
                            .root_namespace
                            .as_ref()
                            .map(|rns| namespace.starts_with(rns.as_str()))
                            .unwrap_or(false);

                        if matches_ref {
                            if let Some(ref root_ns) = ref_config.root_namespace {
                                let rest = namespace
                                    .strip_prefix(root_ns.as_str())
                                    .unwrap_or(namespace);
                                let stripped = rest.strip_prefix('.').unwrap_or(rest);
                                if !stripped.is_empty() {
                                    let relative_path = stripped.replace('.', "/");
                                    let candidate_str = format!(
                                        "{}/{}.cs",
                                        ref_config.project_dir.display(),
                                        relative_path
                                    );
                                    let candidate = CanonicalPath::new(&candidate_str);
                                    if known_files.contains(&candidate) {
                                        return Some(candidate);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // Fallback: naive scan
        self.resolve_naive(namespace, known_files)
    }
}

impl ImportResolver for CSharpResolver {
    fn resolve(
        &self,
        import: &RawImport,
        from_file: &CanonicalPath,
        known_files: &FileSet,
        _workspace: Option<&WorkspaceInfo>,
    ) -> Option<CanonicalPath> {
        let namespace = &import.path;

        // 1. Framework filter
        if self.is_framework_namespace(namespace) {
            return None;
        }

        // 2. NuGet filter
        if self.is_nuget_package(namespace) {
            return None;
        }

        // 3. If no csproj configs, use naive resolution (backward compat)
        if self.csproj_configs.is_empty() {
            return self.resolve_naive(namespace, known_files);
        }

        // 4-6. Config-aware resolution
        self.resolve_with_config(namespace, from_file, known_files)
    }
}

pub(crate) fn parser() -> Box<dyn LanguageParser> {
    Box::new(CSharpParser)
}

pub(crate) fn resolver() -> Box<dyn ImportResolver> {
    Box::new(CSharpResolver::new())
}

pub(crate) fn resolver_with_config(configs: BTreeMap<PathBuf, CsprojConfig>) -> Box<dyn ImportResolver> {
    Box::new(CSharpResolver::new().with_csproj_configs(configs))
}

pub(crate) fn symbol_extractor() -> std::sync::Arc<dyn SymbolExtractor> {
    std::sync::Arc::new(CSharpParser)
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

    // ---- Resolver tests ----

    fn make_import(path: &str) -> RawImport {
        RawImport {
            path: path.to_string(),
            symbols: Vec::new(),
            is_type_only: false,
            kind: ImportKind::Regular,
        }
    }

    fn make_from_file(path: &str) -> CanonicalPath {
        CanonicalPath::new(path)
    }

    #[test]
    fn test_resolver_filters_system_namespace() {
        let resolver = CSharpResolver::new();
        let files = FileSet::new();
        let import = make_import("System.Collections.Generic");
        let result = resolver.resolve(&import, &make_from_file("src/App.cs"), &files, None);
        assert!(result.is_none());
    }

    #[test]
    fn test_resolver_filters_microsoft_namespace() {
        let resolver = CSharpResolver::new();
        let files = FileSet::new();
        let import = make_import("Microsoft.Extensions.DependencyInjection");
        let result = resolver.resolve(&import, &make_from_file("src/App.cs"), &files, None);
        assert!(result.is_none());
    }

    #[test]
    fn test_resolver_filters_nuget_package() {
        use crate::parser::config::csproj::PackageRef;

        let config = CsprojConfig {
            project_path: PathBuf::from("App.csproj"),
            project_dir: PathBuf::from(""),
            target_framework: None,
            root_namespace: None,
            assembly_name: None,
            project_references: Vec::new(),
            package_references: vec![PackageRef {
                name: "Newtonsoft".to_string(),
                version: None,
            }],
        };
        let mut configs = BTreeMap::new();
        configs.insert(PathBuf::from(""), config);
        let resolver = CSharpResolver::new().with_csproj_configs(configs);

        let files = FileSet::new();
        let import = make_import("Newtonsoft.Json");
        let result = resolver.resolve(&import, &make_from_file("src/App.cs"), &files, None);
        assert!(result.is_none());
    }

    #[test]
    fn test_resolver_namespace_to_path() {
        let resolver = CSharpResolver::new();
        let files = FileSet::from_iter(vec![
            CanonicalPath::new("MyApp/Services/UserService.cs"),
        ]);

        let import = make_import("MyApp.Services.UserService");
        let result = resolver.resolve(&import, &make_from_file("src/App.cs"), &files, None);
        assert_eq!(
            result,
            Some(CanonicalPath::new("MyApp/Services/UserService.cs"))
        );
    }

    #[test]
    fn test_resolver_with_root_namespace_stripping() {
        let config = CsprojConfig {
            project_path: PathBuf::from("src/MyApp/MyApp.csproj"),
            project_dir: PathBuf::from("src/MyApp"),
            target_framework: Some("net8.0".to_string()),
            root_namespace: Some("MyApp".to_string()),
            assembly_name: Some("MyApp".to_string()),
            project_references: Vec::new(),
            package_references: Vec::new(),
        };
        let mut configs = BTreeMap::new();
        configs.insert(PathBuf::from("src/MyApp"), config);

        let resolver = CSharpResolver::new().with_csproj_configs(configs);
        let files = FileSet::from_iter(vec![
            CanonicalPath::new("src/MyApp/Services/Foo.cs"),
        ]);

        let import = make_import("MyApp.Services.Foo");
        let result = resolver.resolve(
            &import,
            &make_from_file("src/MyApp/Program.cs"),
            &files,
            None,
        );
        assert_eq!(
            result,
            Some(CanonicalPath::new("src/MyApp/Services/Foo.cs"))
        );
    }

    #[test]
    fn test_resolver_fallback_without_config() {
        let resolver = CSharpResolver::new();
        let files = FileSet::from_iter(vec![
            CanonicalPath::new("Models/User.cs"),
        ]);

        let import = make_import("Models.User");
        let result = resolver.resolve(&import, &make_from_file("src/App.cs"), &files, None);
        assert_eq!(result, Some(CanonicalPath::new("Models/User.cs")));
    }

    // ---- Razor tests ----

    #[test]
    fn test_razor_using_directive() {
        let source = "@using MyApp.Models\n<h1>Hello</h1>";
        let tree = parse(source);
        let imports = CSharpParser.extract_imports(&tree, source.as_bytes());
        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].path, "MyApp.Models");
    }

    #[test]
    fn test_razor_inject_directive() {
        let source = "@inject IUserService UserService\n<p>@UserService.Name</p>";
        let tree = parse(source);
        let imports = CSharpParser.extract_imports(&tree, source.as_bytes());
        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].path, "IUserService");
    }

    #[test]
    fn test_razor_inherits_directive() {
        let source = "@inherits LayoutComponentBase\n<div>@Body</div>";
        let tree = parse(source);
        let imports = CSharpParser.extract_imports(&tree, source.as_bytes());
        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].path, "LayoutComponentBase");
    }

    #[test]
    fn test_razor_mixed_content() {
        let source = r#"@page "/counter"
@using MyApp.Data
@inject WeatherService Weather

<h1>Counter</h1>
<p>Current count: @count</p>
<button @onclick="Increment">Click me</button>

@code {
    private int count = 0;
    private void Increment() { count++; }
}
"#;
        let tree = parse(source);
        let imports = CSharpParser.extract_imports(&tree, source.as_bytes());
        // Should extract @using and @inject, but not @page or @code
        assert_eq!(imports.len(), 2);
        assert_eq!(imports[0].path, "MyApp.Data");
        assert_eq!(imports[1].path, "WeatherService");
    }

    #[test]
    fn test_cs_file_not_detected_as_razor() {
        // Regular C# file starting with "using" should NOT be detected as razor
        let source = r#"using System;
using System.Linq;

namespace MyApp {
    public class Foo {}
}
"#;
        let tree = parse(source);
        let imports = CSharpParser.extract_imports(&tree, source.as_bytes());
        assert_eq!(imports.len(), 2);
        assert_eq!(imports[0].path, "System");
        assert_eq!(imports[1].path, "System.Linq");
    }
}
