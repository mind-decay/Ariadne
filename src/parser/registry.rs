use std::collections::HashMap;
use std::sync::Arc;

use super::csharp;
use super::go;
use super::java;
use super::json_lang;
use super::markdown;
use super::python::{PythonParser, PythonResolver};
use super::rust_lang::{RustParser, RustResolver};
use super::symbols::SymbolExtractor;
use super::traits::{ImportResolver, LanguageParser, RawExport, RawImport};
use super::typescript::{TypeScriptParser, TypeScriptResolver};
use super::yaml;
use crate::model::symbol::SymbolDef;

/// Result of parsing a source file.
pub enum ParseOutcome {
    /// Parsed successfully with no errors.
    Ok(Vec<RawImport>, Vec<RawExport>, Vec<SymbolDef>),
    /// Parsed with partial errors (>0% but ≤50% ERROR nodes) — W007.
    Partial(Vec<RawImport>, Vec<RawExport>, Vec<SymbolDef>),
    /// Parse failed (>50% ERROR nodes or no tree produced) — W001.
    Failed,
}

/// Registry of language parsers and resolvers, indexed by file extension.
pub struct ParserRegistry {
    parsers: Vec<Box<dyn LanguageParser>>,
    resolvers: Vec<Box<dyn ImportResolver>>,
    symbol_extractors: Vec<(Vec<&'static str>, Arc<dyn SymbolExtractor>)>,
    extension_index: HashMap<String, usize>,
    symbol_ext_index: HashMap<String, usize>,
}

impl ParserRegistry {
    pub fn new() -> Self {
        Self {
            parsers: Vec::new(),
            resolvers: Vec::new(),
            symbol_extractors: Vec::new(),
            extension_index: HashMap::new(),
            symbol_ext_index: HashMap::new(),
        }
    }

    /// Register a parser and its corresponding resolver.
    pub fn register(&mut self, parser: Box<dyn LanguageParser>, resolver: Box<dyn ImportResolver>) {
        let index = self.parsers.len();
        for ext in parser.extensions() {
            self.extension_index.insert(ext.to_string(), index);
        }
        self.parsers.push(parser);
        self.resolvers.push(resolver);
    }

    /// Register a symbol extractor for a set of file extensions.
    pub fn register_symbol_extractor(
        &mut self,
        extensions: Vec<&'static str>,
        extractor: Arc<dyn SymbolExtractor>,
    ) {
        let index = self.symbol_extractors.len();
        for ext in &extensions {
            self.symbol_ext_index.insert(ext.to_string(), index);
        }
        self.symbol_extractors.push((extensions, extractor));
    }

    /// Look up a symbol extractor by file extension.
    pub fn symbol_extractor_for(&self, extension: &str) -> Option<&dyn SymbolExtractor> {
        self.symbol_ext_index
            .get(extension)
            .map(|&i| self.symbol_extractors[i].1.as_ref())
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
        Self::with_tier1_config(None)
    }

    /// Create a registry with all Tier 1 parsers, with optional Rust crate name
    /// for resolving `use <crate_name>::` imports as internal.
    pub fn with_tier1_config(rust_crate_name: Option<String>) -> Self {
        let mut registry = Self::new();

        // Create shared instances for symbol extractors (D-077: separate trait)
        let ts_extractor: Arc<dyn SymbolExtractor> = Arc::new(TypeScriptParser::new());
        let rust_extractor: Arc<dyn SymbolExtractor> =
            Arc::new(RustParser::with_crate_name(rust_crate_name.clone()));
        let go_extractor: Arc<dyn SymbolExtractor> = go::symbol_extractor();
        let python_extractor: Arc<dyn SymbolExtractor> = super::python::symbol_extractor();
        let csharp_extractor: Arc<dyn SymbolExtractor> = csharp::symbol_extractor();
        let java_extractor: Arc<dyn SymbolExtractor> = java::symbol_extractor();

        registry.register(
            Box::new(TypeScriptParser::new()),
            Box::new(TypeScriptResolver::new()),
        );
        registry.register(
            Box::new(PythonParser::new()),
            Box::new(PythonResolver::new()),
        );
        registry.register(
            Box::new(RustParser::with_crate_name(rust_crate_name)),
            Box::new(RustResolver::new()),
        );
        registry.register(go::parser(), go::resolver());
        registry.register(csharp::parser(), csharp::resolver());
        registry.register(java::parser(), java::resolver());
        registry.register(markdown::parser(), markdown::resolver());
        registry.register(json_lang::parser(), json_lang::resolver());
        registry.register(yaml::parser(), yaml::resolver());

        // Register symbol extractors (D-077: separate trait)
        registry.register_symbol_extractor(
            vec!["ts", "tsx", "js", "jsx", "mjs", "cjs"],
            ts_extractor,
        );
        registry.register_symbol_extractor(vec!["rs"], rust_extractor);
        registry.register_symbol_extractor(vec!["go"], go_extractor);
        registry.register_symbol_extractor(vec!["py", "pyi"], python_extractor);
        registry.register_symbol_extractor(vec!["cs"], csharp_extractor);
        registry.register_symbol_extractor(vec!["java"], java_extractor);

        registry
    }

    /// Parse source code with the appropriate parser.
    /// Returns `ParseOutcome::Failed` if >50% of top-level nodes are ERROR (W001).
    /// Returns `ParseOutcome::Partial` if >0% but ≤50% ERROR nodes (W007).
    /// Returns `ParseOutcome::Ok` otherwise.
    pub fn parse_source(
        &self,
        source: &[u8],
        parser: &dyn LanguageParser,
        extension: &str,
    ) -> Result<ParseOutcome, String> {
        let mut ts_parser = tree_sitter::Parser::new();
        ts_parser
            .set_language(&parser.tree_sitter_language_for_ext(extension))
            .map_err(|e| format!("grammar version mismatch for {}: {}", parser.language(), e))?;

        let tree = match ts_parser.parse(source, None) {
            Some(t) => t,
            None => return Ok(ParseOutcome::Failed),
        };

        // Check error rate
        let root = tree.root_node();
        let mut is_partial = false;
        if root.has_error() {
            let total = root.child_count();
            if total > 0 {
                let error_count = (0..total)
                    .filter(|&i| root.child(i).is_some_and(|n| n.is_error()))
                    .count();
                if error_count * 2 > total {
                    // >50% ERROR nodes → skip file entirely
                    return Ok(ParseOutcome::Failed);
                }
                if error_count > 0 {
                    is_partial = true;
                }
            }
        }

        let imports = parser.extract_imports(&tree, source);
        let exports = parser.extract_exports(&tree, source);

        // Extract symbols from the same tree (D-077: no re-parsing).
        // catch_unwind guards against panics in symbol extractors (W019 safety).
        let symbols = self
            .symbol_extractor_for(extension)
            .and_then(|ext| {
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    ext.extract_symbols(&tree, source)
                }))
                .ok()
            })
            .unwrap_or_default();

        if is_partial {
            Ok(ParseOutcome::Partial(imports, exports, symbols))
        } else {
            Ok(ParseOutcome::Ok(imports, exports, symbols))
        }
    }

    /// Re-parse imports from source bytes for a given file extension.
    /// Used by the freshness engine for lightweight import change detection.
    pub fn reparse_imports(&self, extension: &str, source: &[u8]) -> Option<Vec<RawImport>> {
        let parser = self.parser_for(extension)?;
        let ts_lang = parser.tree_sitter_language_for_ext(extension);
        let mut ts_parser = tree_sitter::Parser::new();
        ts_parser.set_language(&ts_lang).ok()?;
        let tree = ts_parser.parse(source, None)?;
        Some(parser.extract_imports(&tree, source))
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
