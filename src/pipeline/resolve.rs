use crate::diagnostic::DiagnosticCollector;
use crate::model::{CanonicalPath, FileSet};
use crate::parser::{ImportResolver, RawImport};

/// Resolve a single import using the given resolver.
/// Returns None if unresolved (and records diagnostic).
pub fn resolve_import(
    import: &RawImport,
    from_file: &CanonicalPath,
    known_files: &FileSet,
    resolver: &dyn ImportResolver,
    diagnostics: &DiagnosticCollector,
) -> Option<CanonicalPath> {
    match resolver.resolve(import, from_file, known_files) {
        Some(resolved) => Some(resolved),
        None => {
            // Unresolved — likely external package
            // Only count, don't emit warning (W006 is verbose-only)
            diagnostics.increment_unresolved();
            None
        }
    }
}
