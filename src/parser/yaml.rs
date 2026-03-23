use crate::model::workspace::WorkspaceInfo;
use crate::model::{CanonicalPath, FileSet};
use crate::parser::traits::{ImportResolver, LanguageParser, RawExport, RawImport};

/// YAML parser — recognises `.yaml` and `.yml` files as graph nodes with no dependencies.
struct YamlParser;

impl LanguageParser for YamlParser {
    fn language(&self) -> &str {
        "yaml"
    }

    fn extensions(&self) -> &[&str] {
        &["yaml", "yml"]
    }

    fn tree_sitter_language(&self) -> tree_sitter::Language {
        tree_sitter::Language::from(tree_sitter_yaml::LANGUAGE)
    }

    fn extract_imports(&self, _tree: &tree_sitter::Tree, _source: &[u8]) -> Vec<RawImport> {
        Vec::new()
    }

    fn extract_exports(&self, _tree: &tree_sitter::Tree, _source: &[u8]) -> Vec<RawExport> {
        Vec::new()
    }
}

/// YAML resolver — always returns None (no imports to resolve).
struct YamlResolver;

impl ImportResolver for YamlResolver {
    fn resolve(
        &self,
        _import: &RawImport,
        _from_file: &CanonicalPath,
        _known_files: &FileSet,
        _workspace: Option<&WorkspaceInfo>,
    ) -> Option<CanonicalPath> {
        None
    }
}

pub(crate) fn parser() -> Box<dyn LanguageParser> {
    Box::new(YamlParser)
}

pub(crate) fn resolver() -> Box<dyn ImportResolver> {
    Box::new(YamlResolver)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(source: &str) -> tree_sitter::Tree {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter::Language::from(tree_sitter_yaml::LANGUAGE))
            .unwrap();
        parser.parse(source, None).unwrap()
    }

    fn yaml_imports(source: &str) -> Vec<RawImport> {
        let tree = parse(source);
        YamlParser.extract_imports(&tree, source.as_bytes())
    }

    fn yaml_exports(source: &str) -> Vec<RawExport> {
        let tree = parse(source);
        YamlParser.extract_exports(&tree, source.as_bytes())
    }

    #[test]
    fn valid_yaml_returns_no_imports() {
        let source = "name: test\nversion: 1.0\nitems:\n  - 1\n  - 2\n";
        assert!(yaml_imports(source).is_empty());
    }

    #[test]
    fn valid_yaml_returns_no_exports() {
        let source = "name: test\nversion: 1.0\n";
        assert!(yaml_exports(source).is_empty());
    }

    #[test]
    fn empty_yaml() {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter::Language::from(tree_sitter_yaml::LANGUAGE))
            .unwrap();
        let tree = parser.parse(b"", None).unwrap();
        assert!(YamlParser.extract_imports(&tree, b"").is_empty());
        assert!(YamlParser.extract_exports(&tree, b"").is_empty());
    }

    #[test]
    fn resolver_always_none() {
        let import = RawImport {
            path: "./other.yaml".to_string(),
            symbols: vec![],
            is_type_only: false,
            kind: crate::parser::traits::ImportKind::Regular,
        };
        let from = CanonicalPath::new("data/config.yaml");
        let files = FileSet::new();
        assert_eq!(YamlResolver.resolve(&import, &from, &files, None), None);
    }
}
