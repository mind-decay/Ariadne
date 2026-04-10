use crate::model::semantic::Boundary;
use crate::model::symbol::SymbolDef;
use crate::model::workspace::WorkspaceInfo;
use crate::model::{CanonicalPath, FileSet};

/// Discriminant for import origin.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ImportKind {
    /// Standard import statement
    Regular,
    /// Rust `mod` declaration — path is the module name, not a filesystem path
    ModDeclaration,
    /// Markdown link reference — path is a relative file link
    Link,
    /// .csproj <ProjectReference> cross-project edge
    ProjectReference,
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

/// Result of parsing a source file.
pub enum ParseOutcome {
    /// Parsed successfully with no errors.
    Ok(Vec<RawImport>, Vec<RawExport>, Vec<SymbolDef>, Vec<Boundary>),
    /// Parsed with partial errors (>0% but ≤50% ERROR nodes) — W007.
    Partial(Vec<RawImport>, Vec<RawExport>, Vec<SymbolDef>, Vec<Boundary>),
    /// Parse failed (>50% ERROR nodes or no tree produced) — W001.
    Failed,
}

/// Extracts imports/exports from AST (language syntax knowledge).
pub trait LanguageParser: Send + Sync {
    fn language(&self) -> &str;
    fn extensions(&self) -> &[&str];
    fn tree_sitter_language(&self) -> tree_sitter::Language;
    /// Return the tree-sitter grammar for a specific file extension.
    /// Override when a single parser covers multiple grammars (e.g. TS vs TSX).
    fn tree_sitter_language_for_ext(&self, _ext: &str) -> tree_sitter::Language {
        self.tree_sitter_language()
    }
    fn extract_imports(&self, tree: &tree_sitter::Tree, source: &[u8]) -> Vec<RawImport>;
    fn extract_exports(&self, tree: &tree_sitter::Tree, source: &[u8]) -> Vec<RawExport>;

    /// Bypass tree-sitter for file formats that need custom parsing (D-145).
    /// Return `Some(outcome)` to skip tree-sitter parse entirely.
    fn raw_parse(&self, _source: &[u8], _extension: &str, _path: &CanonicalPath)
        -> Option<ParseOutcome> { None }
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
