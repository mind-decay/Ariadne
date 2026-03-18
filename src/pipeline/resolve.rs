use crate::detect::find_case_insensitive;
use crate::diagnostic::DiagnosticCollector;
use crate::model::workspace::WorkspaceInfo;
use crate::model::{CanonicalPath, FileSet};
use crate::parser::{ImportResolver, RawImport};

/// Resolve a single import using the given resolver.
/// Returns None if unresolved (and records diagnostic).
/// When `case_insensitive` is true, tries case-insensitive fallback after exact match fails.
pub fn resolve_import(
    import: &RawImport,
    from_file: &CanonicalPath,
    known_files: &FileSet,
    resolver: &dyn ImportResolver,
    diagnostics: &DiagnosticCollector,
    workspace: Option<&WorkspaceInfo>,
    case_insensitive: bool,
) -> Option<CanonicalPath> {
    let resolved = resolver.resolve(import, from_file, known_files, workspace);

    // Case-insensitive fallback: if exact resolution failed and FS is case-insensitive,
    // try to find a case-insensitive match in known_files
    let resolved = match resolved {
        Some(r) => Some(r),
        None if case_insensitive => {
            // Build candidate path from import and try case-insensitive lookup
            // The resolver already tried exact match; we re-resolve without FS case constraint
            // by checking all known_files for a case-insensitive match of the import path
            let import_path = CanonicalPath::new(&import.path);
            find_case_insensitive(&import_path, known_files)
        }
        None => None,
    };

    match resolved {
        Some(resolved) if resolved == *from_file => None, // INV-2: filter self-imports
        Some(resolved) => Some(resolved),
        None => {
            diagnostics.increment_unresolved();
            None
        }
    }
}
