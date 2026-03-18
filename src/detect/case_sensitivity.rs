use std::path::Path;

use crate::model::types::{CanonicalPath, FileSet};

/// Check if the filesystem at `root` is case-insensitive.
/// Creates a temp file and checks if a case-swapped variant exists.
/// Returns false on detection failure (assume case-sensitive — safer default).
pub fn is_case_insensitive(root: &Path) -> bool {
    use std::fs;

    let probe = root.join(".ariadne_case_test");
    let swapped = root.join(".ARIADNE_CASE_TEST");

    // Create the probe file
    if fs::write(&probe, "").is_err() {
        return false;
    }

    // Check if the case-swapped variant resolves to an existing file
    let result = swapped.exists();

    // Clean up
    let _ = fs::remove_file(&probe);

    result
}

/// Find a case-insensitive match for `target` in `known_files`.
/// Used when exact match fails on case-insensitive filesystems.
/// Returns None if no match found.
pub fn find_case_insensitive(
    target: &CanonicalPath,
    known_files: &FileSet,
) -> Option<CanonicalPath> {
    let target_lower = target.as_str().to_lowercase();
    known_files
        .iter()
        .find(|f| f.as_str().to_lowercase() == target_lower)
        .cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_case_insensitive_on_current_platform() {
        let dir = tempfile::tempdir().unwrap();
        let result = is_case_insensitive(dir.path());

        // macOS is case-insensitive by default, Linux is case-sensitive
        if cfg!(target_os = "macos") {
            assert!(result, "macOS should report case-insensitive");
        } else if cfg!(target_os = "linux") {
            assert!(!result, "Linux should report case-sensitive");
        }
        // On other platforms, just verify it returns without error
    }

    #[test]
    fn test_is_case_insensitive_invalid_root() {
        // Non-existent directory should return false (safe default)
        let result = is_case_insensitive(Path::new("/nonexistent/path/ariadne_test"));
        assert!(!result);
    }

    #[test]
    fn test_find_case_insensitive_exact_match() {
        let files = FileSet::from_iter(vec![
            CanonicalPath::new("src/App.tsx"),
            CanonicalPath::new("src/utils.ts"),
        ]);
        let target = CanonicalPath::new("src/App.tsx");
        let result = find_case_insensitive(&target, &files);
        assert_eq!(result.unwrap().as_str(), "src/App.tsx");
    }

    #[test]
    fn test_find_case_insensitive_different_case() {
        let files = FileSet::from_iter(vec![
            CanonicalPath::new("src/App.tsx"),
            CanonicalPath::new("src/utils.ts"),
        ]);
        let target = CanonicalPath::new("src/app.tsx");
        let result = find_case_insensitive(&target, &files);
        assert_eq!(result.unwrap().as_str(), "src/App.tsx");
    }

    #[test]
    fn test_find_case_insensitive_no_match() {
        let files = FileSet::from_iter(vec![
            CanonicalPath::new("src/App.tsx"),
            CanonicalPath::new("src/utils.ts"),
        ]);
        let target = CanonicalPath::new("src/missing.ts");
        let result = find_case_insensitive(&target, &files);
        assert!(result.is_none());
    }

    #[test]
    fn test_find_case_insensitive_empty_fileset() {
        let files = FileSet::new();
        let target = CanonicalPath::new("src/App.tsx");
        let result = find_case_insensitive(&target, &files);
        assert!(result.is_none());
    }
}
