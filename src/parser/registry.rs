use std::collections::HashMap;

use super::csharp;
use super::go;
use super::java;
use super::python::{PythonParser, PythonResolver};
use super::rust_lang::{RustParser, RustResolver};
use super::traits::{ImportResolver, LanguageParser, RawExport, RawImport};
use super::typescript::{TypeScriptParser, TypeScriptResolver};

/// Registry of language parsers and resolvers, indexed by file extension.
pub struct ParserRegistry {
    parsers: Vec<Box<dyn LanguageParser>>,
    resolvers: Vec<Box<dyn ImportResolver>>,
    extension_index: HashMap<String, usize>,
}

impl ParserRegistry {
    pub fn new() -> Self {
        Self {
            parsers: Vec::new(),
            resolvers: Vec::new(),
            extension_index: HashMap::new(),
        }
    }

    /// Register a parser and its corresponding resolver.
    pub fn register(
        &mut self,
        parser: Box<dyn LanguageParser>,
        resolver: Box<dyn ImportResolver>,
    ) {
        let index = self.parsers.len();
        for ext in parser.extensions() {
            self.extension_index.insert(ext.to_string(), index);
        }
        self.parsers.push(parser);
        self.resolvers.push(resolver);
    }

    /// Look up a parser by file extension.
    pub fn parser_for(&self, extension: &str) -> Option<&dyn LanguageParser> {
        self.extension_index
            .get(extension)
            .map(|&i| self.parsers[i].as_ref())
    }

    /// Look up a resolver by file extension.
    pub fn resolver_for(&self, extension: &str) -> Option<&dyn ImportResolver> {
        self.extension_index
            .get(extension)
            .map(|&i| self.resolvers[i].as_ref())
    }

    /// Create a registry with all Tier 1 language parsers registered.
    pub fn with_tier1() -> Self {
        let mut registry = Self::new();
        // Chunk 4: TypeScript/JavaScript, Python, Rust
        registry.register(
            Box::new(TypeScriptParser::new()),
            Box::new(TypeScriptResolver::new()),
        );
        registry.register(
            Box::new(PythonParser::new()),
            Box::new(PythonResolver::new()),
        );
        registry.register(
            Box::new(RustParser::new()),
            Box::new(RustResolver::new()),
        );
        registry.register(go::parser(), go::resolver());
        registry.register(csharp::parser(), csharp::resolver());
        registry.register(java::parser(), java::resolver());
        registry
    }

    /// Parse source code with the appropriate parser.
    /// Returns None if >50% of top-level nodes are ERROR (W001).
    /// Otherwise extracts imports/exports from valid subtrees.
    pub fn parse_source(
        &self,
        source: &[u8],
        parser: &dyn LanguageParser,
    ) -> Result<Option<(tree_sitter::Tree, Vec<RawImport>, Vec<RawExport>)>, String> {
        let mut ts_parser = tree_sitter::Parser::new();
        ts_parser
            .set_language(&parser.tree_sitter_language())
            .map_err(|e| format!("grammar version mismatch for {}: {}", parser.language(), e))?;

        let tree = match ts_parser.parse(source, None) {
            Some(t) => t,
            None => return Ok(None),
        };

        // Check error rate
        let root = tree.root_node();
        if root.has_error() {
            let total = root.child_count();
            if total > 0 {
                let error_count = (0..total)
                    .filter(|&i| root.child(i).map_or(false, |n| n.is_error()))
                    .count();
                if error_count * 2 > total {
                    // >50% ERROR nodes → skip file entirely
                    return Ok(None);
                }
            }
        }

        let imports = parser.extract_imports(&tree, source);
        let exports = parser.extract_exports(&tree, source);

        Ok(Some((tree, imports, exports)))
    }

    /// List all supported extensions.
    pub fn supported_extensions(&self) -> Vec<&str> {
        let mut exts: Vec<&str> = self.extension_index.keys().map(|s| s.as_str()).collect();
        exts.sort();
        exts
    }

    /// List all registered language names.
    pub fn language_names(&self) -> Vec<&str> {
        self.parsers.iter().map(|p| p.language()).collect()
    }
}

impl Default for ParserRegistry {
    fn default() -> Self {
        Self::new()
    }
}
