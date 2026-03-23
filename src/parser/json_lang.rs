use crate::model::workspace::WorkspaceInfo;
use crate::model::{CanonicalPath, FileSet};
use crate::parser::traits::{ImportResolver, LanguageParser, RawExport, RawImport};

/// JSON parser — recognises `.json` files as graph nodes with no dependencies.
struct JsonParser;

impl LanguageParser for JsonParser {
    fn language(&self) -> &str {
        "json"
    }

    fn extensions(&self) -> &[&str] {
        &["json"]
    }

    fn tree_sitter_language(&self) -> tree_sitter::Language {
        tree_sitter::Language::from(tree_sitter_json::LANGUAGE)
    }

    fn extract_imports(&self, _tree: &tree_sitter::Tree, _source: &[u8]) -> Vec<RawImport> {
        Vec::new()
    }

    fn extract_exports(&self, _tree: &tree_sitter::Tree, _source: &[u8]) -> Vec<RawExport> {
        Vec::new()
    }
}

/// JSON resolver — always returns None (no imports to resolve).
struct JsonResolver;

impl ImportResolver for JsonResolver {
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
    Box::new(JsonParser)
}

pub(crate) fn resolver() -> Box<dyn ImportResolver> {
    Box::new(JsonResolver)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(source: &str) -> tree_sitter::Tree {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter::Language::from(tree_sitter_json::LANGUAGE))
            .unwrap();
        parser.parse(source, None).unwrap()
    }

    fn json_imports(source: &str) -> Vec<RawImport> {
        let tree = parse(source);
        JsonParser.extract_imports(&tree, source.as_bytes())
    }

    fn json_exports(source: &str) -> Vec<RawExport> {
        let tree = parse(source);
        JsonParser.extract_exports(&tree, source.as_bytes())
    }

    #[test]
    fn valid_json_returns_no_imports() {
        let source = r#"{"name": "test", "version": "1.0"}"#;
        assert!(json_imports(source).is_empty());
    }

    #[test]
    fn valid_json_returns_no_exports() {
        let source = r#"{"name": "test", "version": "1.0"}"#;
        assert!(json_exports(source).is_empty());
    }

    #[test]
    fn empty_json_object() {
        assert!(json_imports("{}").is_empty());
        assert!(json_exports("{}").is_empty());
    }

    #[test]
    fn empty_source() {
        // tree-sitter still produces a tree for empty input
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter::Language::from(tree_sitter_json::LANGUAGE))
            .unwrap();
        let tree = parser.parse(b"", None).unwrap();
        assert!(JsonParser.extract_imports(&tree, b"").is_empty());
        assert!(JsonParser.extract_exports(&tree, b"").is_empty());
    }

    #[test]
    fn resolver_always_none() {
        let import = RawImport {
            path: "./other.json".to_string(),
            symbols: vec![],
            is_type_only: false,
            kind: crate::parser::traits::ImportKind::Regular,
        };
        let from = CanonicalPath::new("data/config.json");
        let files = FileSet::new();
        assert_eq!(JsonResolver.resolve(&import, &from, &files, None), None);
    }
}
