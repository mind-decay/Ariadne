use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::model::symbol::{LineSpan, SymbolDef, SymbolKind, Visibility};
use crate::model::workspace::WorkspaceInfo;
use crate::model::{CanonicalPath, FileSet};
use crate::parser::config::{GradleConfig, MavenConfig};
use crate::parser::symbols::SymbolExtractor;
use crate::parser::traits::{ImportKind, ImportResolver, LanguageParser, RawExport, RawImport};

/// Java language parser.
pub(crate) struct JavaParser;

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
            "class_declaration"
            | "interface_declaration"
            | "enum_declaration"
            | "record_declaration"
            | "annotation_type_declaration" => {
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

impl SymbolExtractor for JavaParser {
    fn extract_symbols(&self, tree: &tree_sitter::Tree, source: &[u8]) -> Vec<SymbolDef> {
        let mut symbols = Vec::new();
        let root = tree.root_node();
        extract_java_symbols_from_node(&root, source, &mut symbols, None);
        symbols
    }
}

/// Recursively extract symbol definitions from a Java AST node.
fn extract_java_symbols_from_node(
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
                let name = find_java_declaration_name(&child, source);
                let visibility = java_visibility(&child, source);
                if let Some(ref n) = name {
                    symbols.push(SymbolDef {
                        name: n.clone(),
                        kind: SymbolKind::Class,
                        visibility,
                        span: java_node_span(&child),
                        signature: java_truncate_signature(&child, source, 200),
                        parent: parent_name.map(|s| s.to_string()),
                    });
                }
                // Extract members inside class body
                if let Some(body) = child.child_by_field_name("body") {
                    extract_java_symbols_from_node(&body, source, symbols, name.as_deref());
                }
            }
            "interface_declaration" => {
                let name = find_java_declaration_name(&child, source);
                let visibility = java_visibility(&child, source);
                if let Some(ref n) = name {
                    symbols.push(SymbolDef {
                        name: n.clone(),
                        kind: SymbolKind::Interface,
                        visibility,
                        span: java_node_span(&child),
                        signature: java_truncate_signature(&child, source, 200),
                        parent: parent_name.map(|s| s.to_string()),
                    });
                }
                if let Some(body) = child.child_by_field_name("body") {
                    extract_java_symbols_from_node(&body, source, symbols, name.as_deref());
                }
            }
            "method_declaration" => {
                if let Some(name_node) = child.child_by_field_name("name") {
                    if let Ok(name) = name_node.utf8_text(source) {
                        symbols.push(SymbolDef {
                            name: name.to_string(),
                            kind: SymbolKind::Method,
                            visibility: java_visibility(&child, source),
                            span: java_node_span(&child),
                            signature: java_truncate_signature(&child, source, 200),
                            parent: parent_name.map(|s| s.to_string()),
                        });
                    }
                }
            }
            "field_declaration" => {
                // Check for static final → Const
                if has_java_modifier(&child, "static") && has_java_modifier(&child, "final") {
                    extract_java_field_names(&child, source, symbols, parent_name);
                }
            }
            "enum_declaration" => {
                let name = find_java_declaration_name(&child, source);
                let visibility = java_visibility(&child, source);
                if let Some(ref n) = name {
                    symbols.push(SymbolDef {
                        name: n.clone(),
                        kind: SymbolKind::Enum,
                        visibility,
                        span: java_node_span(&child),
                        signature: java_truncate_signature(&child, source, 200),
                        parent: parent_name.map(|s| s.to_string()),
                    });
                }
            }
            "program" => {
                extract_java_symbols_from_node(&child, source, symbols, parent_name);
            }
            _ => {}
        }
    }
}

/// Extract field names from a Java field_declaration (for static final constants).
fn extract_java_field_names(
    node: &tree_sitter::Node,
    source: &[u8],
    symbols: &mut Vec<SymbolDef>,
    parent_name: Option<&str>,
) {
    let visibility = java_visibility(node, source);
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            if child.kind() == "variable_declarator" {
                if let Some(name_node) = child.child_by_field_name("name") {
                    if let Ok(name) = name_node.utf8_text(source) {
                        symbols.push(SymbolDef {
                            name: name.to_string(),
                            kind: SymbolKind::Const,
                            visibility,
                            span: java_node_span(node),
                            signature: java_truncate_signature(node, source, 200),
                            parent: parent_name.map(|s| s.to_string()),
                        });
                    }
                }
            }
        }
    }
}

/// Determine Java visibility from modifier nodes.
fn java_visibility(node: &tree_sitter::Node, _source: &[u8]) -> Visibility {
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            if child.kind() == "modifiers" {
                for j in 0..child.child_count() {
                    if let Some(modifier) = child.child(j) {
                        match modifier.kind() {
                            "public" => return Visibility::Public,
                            "private" | "protected" => return Visibility::Private,
                            _ => {}
                        }
                    }
                }
            }
        }
    }
    // Default (package-private) → Internal
    Visibility::Internal
}

/// Check if a Java declaration has a specific modifier.
fn has_java_modifier(node: &tree_sitter::Node, modifier: &str) -> bool {
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            if child.kind() == "modifiers" {
                for j in 0..child.child_count() {
                    if let Some(m) = child.child(j) {
                        if m.kind() == modifier {
                            return true;
                        }
                    }
                }
            }
        }
    }
    false
}

/// Get LineSpan from a tree-sitter node (1-based).
fn java_node_span(node: &tree_sitter::Node) -> LineSpan {
    LineSpan {
        start: node.start_position().row as u32 + 1,
        end: node.end_position().row as u32 + 1,
    }
}

/// Extract first line of node text, truncated to max_len.
fn java_truncate_signature(
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

/// Build system configuration injected at construction time (D-138).
#[allow(dead_code)]
pub enum JavaBuildConfig {
    Gradle {
        config: GradleConfig,
        subconfigs: BTreeMap<PathBuf, GradleConfig>,
    },
    Maven {
        configs: BTreeMap<PathBuf, MavenConfig>,
    },
    Both {
        gradle_config: GradleConfig,
        gradle_subconfigs: BTreeMap<PathBuf, GradleConfig>,
        maven_configs: BTreeMap<PathBuf, MavenConfig>,
    },
}

/// Config-aware Java import resolver (D-144).
/// Falls back to the heuristic resolver if config provides no match.
struct JavaConfigResolver {
    build_config: JavaBuildConfig,
}

impl JavaConfigResolver {
    /// Collect source directories with their base paths from the build config.
    fn collect_source_dirs(&self) -> Vec<(PathBuf, String)> {
        match &self.build_config {
            JavaBuildConfig::Gradle { config, subconfigs } => {
                let mut dirs = Vec::new();
                for src_dir in &config.source_dirs {
                    dirs.push((config.config_dir.clone(), src_dir.clone()));
                }
                for (sub_path, sub_config) in subconfigs {
                    for src_dir in &sub_config.source_dirs {
                        dirs.push((sub_path.clone(), src_dir.clone()));
                    }
                }
                dirs
            }
            JavaBuildConfig::Maven { configs } => {
                let mut dirs = Vec::new();
                for (_path, config) in configs {
                    let src_dir = config
                        .source_directory
                        .as_deref()
                        .unwrap_or("src/main/java")
                        .to_string();
                    dirs.push((config.config_dir.clone(), src_dir));
                }
                dirs
            }
            JavaBuildConfig::Both {
                gradle_config,
                gradle_subconfigs,
                maven_configs: _,
            } => {
                // D-138: prefer Gradle source dirs
                let mut dirs = Vec::new();
                for src_dir in &gradle_config.source_dirs {
                    dirs.push((gradle_config.config_dir.clone(), src_dir.clone()));
                }
                for (sub_path, sub_config) in gradle_subconfigs {
                    for src_dir in &sub_config.source_dirs {
                        dirs.push((sub_path.clone(), src_dir.clone()));
                    }
                }
                dirs
            }
        }
    }

    /// Extract the project group prefix (top 2 segments) for external dep filtering.
    fn project_group_prefix(&self) -> Option<String> {
        let group = match &self.build_config {
            JavaBuildConfig::Gradle { config, .. } => config.group.as_deref(),
            JavaBuildConfig::Maven { configs } => {
                configs.values().next().and_then(|c| c.group_id.as_deref())
            }
            JavaBuildConfig::Both { gradle_config, .. } => gradle_config.group.as_deref(),
        };
        group.map(|g| {
            // Take top 2 segments: "com.example.foo" -> "com.example"
            let parts: Vec<&str> = g.split('.').collect();
            if parts.len() >= 2 {
                format!("{}.{}", parts[0], parts[1])
            } else {
                g.to_string()
            }
        })
    }

    /// Check if an import path looks external relative to the project group.
    fn is_external_import(&self, import_path: &str) -> bool {
        if let Some(project_prefix) = self.project_group_prefix() {
            let import_parts: Vec<&str> = import_path.split('.').collect();
            let project_parts: Vec<&str> = project_prefix.split('.').collect();
            if import_parts.len() >= 2 && project_parts.len() >= 2 {
                // Compare top 2 segments
                let import_prefix = format!("{}.{}", import_parts[0], import_parts[1]);
                return import_prefix != project_prefix;
            }
        }
        false
    }
}

impl ImportResolver for JavaConfigResolver {
    fn resolve(
        &self,
        import: &RawImport,
        _from_file: &CanonicalPath,
        known_files: &FileSet,
        _workspace: Option<&WorkspaceInfo>,
    ) -> Option<CanonicalPath> {
        let import_path = &import.path;

        // Step 5: External dependency filter — skip clearly external imports
        if self.is_external_import(import_path) {
            return None;
        }

        // Handle wildcard imports (com.example.*)
        if import_path.ends_with(".*") {
            let package = import_path.trim_end_matches(".*");
            let dir = package.replace('.', "/");

            // Try config source dirs first
            for (base_path, src_dir) in self.collect_source_dirs() {
                let candidate_dir = if base_path.as_os_str().is_empty() {
                    format!("{}/{}/", src_dir, dir)
                } else {
                    format!("{}/{}/{}/", base_path.display(), src_dir, dir)
                };
                for file in known_files.iter() {
                    let file_str = file.as_str();
                    if file_str.starts_with(&candidate_dir) && file_str.ends_with(".java") {
                        let remainder = &file_str[candidate_dir.len()..];
                        if !remainder.contains('/') {
                            return Some(file.clone());
                        }
                    }
                }
            }

            // Fallback: src/main/java/ prefix
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

            // Fallback: plain dir
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

        // For static imports, strip the last segment (member name)
        let class_path = if import.symbols.iter().any(|s| s == "static") {
            match import_path.rsplit_once('.') {
                Some((class_part, _)) => class_part,
                None => import_path.as_str(),
            }
        } else {
            import_path.as_str()
        };

        let file_path = class_path.replace('.', "/");

        // Try config source dirs first (D-144 layered resolution)
        for (base_path, src_dir) in self.collect_source_dirs() {
            let candidate = if base_path.as_os_str().is_empty() {
                CanonicalPath::new(format!("{}/{}.java", src_dir, file_path))
            } else {
                CanonicalPath::new(format!(
                    "{}/{}/{}.java",
                    base_path.display(),
                    src_dir,
                    file_path
                ))
            };
            if known_files.contains(&candidate) {
                return Some(candidate);
            }
        }

        // Fallback: src/main/java/ prefix
        let prefixed = CanonicalPath::new(format!("src/main/java/{}.java", file_path));
        if known_files.contains(&prefixed) {
            return Some(prefixed);
        }

        // Fallback: plain path
        let plain = CanonicalPath::new(format!("{}.java", file_path));
        if known_files.contains(&plain) {
            return Some(plain);
        }

        None
    }
}

/// Create a config-aware Java import resolver (D-144).
pub(crate) fn resolver_with_config(build_config: JavaBuildConfig) -> Box<dyn ImportResolver> {
    Box::new(JavaConfigResolver { build_config })
}

pub(crate) fn symbol_extractor() -> std::sync::Arc<dyn SymbolExtractor> {
    std::sync::Arc::new(JavaParser)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::traits::LanguageParser;

    fn parse(source: &str) -> tree_sitter::Tree {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter::Language::from(tree_sitter_java::LANGUAGE))
            .unwrap();
        parser.parse(source, None).unwrap()
    }

    fn java_imports(source: &str) -> Vec<RawImport> {
        let tree = parse(source);
        JavaParser.extract_imports(&tree, source.as_bytes())
    }

    fn java_exports(source: &str) -> Vec<RawExport> {
        let tree = parse(source);
        JavaParser.extract_exports(&tree, source.as_bytes())
    }

    // ---- Import tests ----

    #[test]
    fn import_single_class() {
        let source = "import com.example.MyClass;";
        let result = java_imports(source);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].path, "com.example.MyClass");
        assert!(result[0].symbols.is_empty());
    }

    #[test]
    fn import_wildcard() {
        let source = "import com.example.*;";
        let result = java_imports(source);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].path, "com.example.*");
    }

    #[test]
    fn import_static() {
        let source = "import static org.junit.Assert.assertEquals;";
        let result = java_imports(source);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].path, "org.junit.Assert.assertEquals");
        assert!(result[0].symbols.contains(&"static".to_string()));
    }

    #[test]
    fn import_multiple() {
        let source = r#"
import java.util.List;
import java.util.Map;
import java.io.File;
"#;
        let result = java_imports(source);
        assert_eq!(result.len(), 3);
    }

    #[test]
    fn import_static_wildcard() {
        let source = "import static com.example.Constants.*;";
        let result = java_imports(source);
        assert_eq!(result.len(), 1);
        assert!(result[0].symbols.contains(&"static".to_string()));
    }

    #[test]
    fn empty_source_no_imports() {
        let result = java_imports("");
        assert!(result.is_empty());
    }

    #[test]
    fn malformed_no_crash() {
        let result = java_imports("import ;");
        let _ = result;
    }

    // ---- Export tests ----

    #[test]
    fn public_class_exported() {
        let source = "public class MyService {}";
        let result = java_exports(source);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "MyService");
    }

    #[test]
    fn public_interface_exported() {
        let source = "public interface Dao {}";
        let result = java_exports(source);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "Dao");
    }

    #[test]
    fn package_private_class_not_exported() {
        let source = "class Internal {}";
        let result = java_exports(source);
        assert!(result.is_empty());
    }

    #[test]
    fn public_enum_exported() {
        let source = "public enum Status { ACTIVE, INACTIVE }";
        let result = java_exports(source);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "Status");
    }

    #[test]
    fn empty_source_no_exports() {
        let result = java_exports("");
        assert!(result.is_empty());
    }

    // ---- Config resolver tests (D-144) ----

    fn make_file_set(paths: &[&str]) -> FileSet {
        paths
            .iter()
            .map(|p| CanonicalPath::new(p.to_string()))
            .collect()
    }

    fn make_import(path: &str) -> RawImport {
        RawImport {
            path: path.to_string(),
            symbols: Vec::new(),
            is_type_only: false,
            kind: ImportKind::Regular,
        }
    }

    fn dummy_from_file() -> CanonicalPath {
        CanonicalPath::new("src/main/java/com/example/App.java".to_string())
    }

    #[test]
    fn test_config_resolver_gradle_source_dir() {
        let config = JavaBuildConfig::Gradle {
            config: GradleConfig {
                config_dir: PathBuf::new(),
                group: Some("com.example".to_string()),
                version: None,
                source_dirs: vec!["src/main/java".to_string()],
                test_source_dirs: vec![],
                dependencies: vec![],
                subprojects: vec![],
                is_android: false,
            },
            subconfigs: BTreeMap::new(),
        };
        let resolver = JavaConfigResolver { build_config: config };
        let file_set = make_file_set(&["src/main/java/com/example/MyClass.java"]);
        let import = make_import("com.example.MyClass");
        let result = resolver.resolve(&import, &dummy_from_file(), &file_set, None);
        assert_eq!(
            result,
            Some(CanonicalPath::new(
                "src/main/java/com/example/MyClass.java".to_string()
            ))
        );
    }

    #[test]
    fn test_config_resolver_maven_source_dir() {
        let mut configs = BTreeMap::new();
        configs.insert(
            PathBuf::new(),
            MavenConfig {
                config_path: PathBuf::new(),
                config_dir: PathBuf::new(),
                group_id: Some("com.example".to_string()),
                artifact_id: Some("myapp".to_string()),
                version: None,
                packaging: None,
                source_directory: Some("src/main/java".to_string()),
                test_source_directory: None,
                modules: vec![],
                dependencies: vec![],
                parent: None,
            },
        );
        let config = JavaBuildConfig::Maven { configs };
        let resolver = JavaConfigResolver { build_config: config };
        let file_set = make_file_set(&["src/main/java/com/example/Service.java"]);
        let import = make_import("com.example.Service");
        let result = resolver.resolve(&import, &dummy_from_file(), &file_set, None);
        assert_eq!(
            result,
            Some(CanonicalPath::new(
                "src/main/java/com/example/Service.java".to_string()
            ))
        );
    }

    #[test]
    fn test_config_resolver_external_dep_skipped() {
        let config = JavaBuildConfig::Gradle {
            config: GradleConfig {
                config_dir: PathBuf::new(),
                group: Some("com.example".to_string()),
                version: None,
                source_dirs: vec!["src/main/java".to_string()],
                test_source_dirs: vec![],
                dependencies: vec![],
                subprojects: vec![],
                is_android: false,
            },
            subconfigs: BTreeMap::new(),
        };
        let resolver = JavaConfigResolver { build_config: config };
        let file_set = make_file_set(&["src/main/java/com/example/MyClass.java"]);
        // External import — different top-2 segments
        let import = make_import("org.springframework.web.bind.annotation.RestController");
        let result = resolver.resolve(&import, &dummy_from_file(), &file_set, None);
        assert_eq!(result, None);
    }

    #[test]
    fn test_config_resolver_fallback() {
        // Empty source dirs — no config match, falls back to src/main/java/
        let config = JavaBuildConfig::Gradle {
            config: GradleConfig {
                config_dir: PathBuf::new(),
                group: None,
                version: None,
                source_dirs: vec![],
                test_source_dirs: vec![],
                dependencies: vec![],
                subprojects: vec![],
                is_android: false,
            },
            subconfigs: BTreeMap::new(),
        };
        let resolver = JavaConfigResolver { build_config: config };
        let file_set = make_file_set(&["src/main/java/com/example/Fallback.java"]);
        let import = make_import("com.example.Fallback");
        let result = resolver.resolve(&import, &dummy_from_file(), &file_set, None);
        assert_eq!(
            result,
            Some(CanonicalPath::new(
                "src/main/java/com/example/Fallback.java".to_string()
            ))
        );
    }
}
