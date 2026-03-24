use crate::model::symbol::SymbolDef;

/// Extracts symbol definitions from a parsed AST (D-077).
/// Separate from LanguageParser — receives the same tree-sitter Tree already parsed.
pub trait SymbolExtractor: Send + Sync {
    fn extract_symbols(&self, tree: &tree_sitter::Tree, source: &[u8]) -> Vec<SymbolDef>;
}
