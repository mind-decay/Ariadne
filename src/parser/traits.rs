use crate::model::workspace::WorkspaceInfo;
use crate::model::{CanonicalPath, FileSet};

/// Discriminant for import origin.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ImportKind {
    /// Standard import statement
    Regular,
    /// Rust `mod` declaration — path is the module name, not a filesystem path
    ModDeclaration,
}

/// Raw import extracted from AST (unresolved).
#[derive(Clone, Debug)]
pub struct RawImport {
    pub path: String,
    pub symbols: Vec<String>,
    pub is_type_only: bool,
    pub kind: ImportKind,
}

/// Raw export extracted from AST.
#[derive(Clone, Debug)]
pub struct RawExport {
    pub name: String,
    pub is_re_export: bool,
    pub source: Option<String>,
}

/// Extracts imports/exports from AST (language syntax knowledge).
pub trait LanguageParser: Send + Sync {
    fn language(&self) -> &str;
    fn extensions(&self) -> &[&str];
    fn tree_sitter_language(&self) -> tree_sitter::Language;
    fn extract_imports(&self, tree: &tree_sitter::Tree, source: &[u8]) -> Vec<RawImport>;
    fn extract_exports(&self, tree: &tree_sitter::Tree, source: &[u8]) -> Vec<RawExport>;
}

/// Resolves raw import paths to canonical file paths (filesystem knowledge).
pub trait ImportResolver: Send + Sync {
    fn resolve(
        &self,
        import: &RawImport,
        from_file: &CanonicalPath,
        known_files: &FileSet,
        workspace: Option<&WorkspaceInfo>,
    ) -> Option<CanonicalPath>;
}
