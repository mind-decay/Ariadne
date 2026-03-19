use std::collections::BTreeSet;
use std::fmt;

/// Canonical file path relative to project root.
/// Normalized: forward slashes, no `./`, no `..`, no trailing slash.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CanonicalPath(String);

impl CanonicalPath {
    /// Create a new CanonicalPath, enforcing normalization invariants.
    pub fn new(path: impl Into<String>) -> Self {
        let raw = path.into();
        let normalized = Self::normalize(&raw);
        Self(normalized)
    }

    fn normalize(path: &str) -> String {
        // Replace backslashes with forward slashes
        let path = path.replace('\\', "/");
        // Remove leading ./
        let path = path.strip_prefix("./").unwrap_or(&path);
        // Split into segments, resolve . and ..
        let mut segments: Vec<&str> = Vec::new();
        for segment in path.split('/') {
            match segment {
                "" | "." => continue,
                ".." => {
                    segments.pop();
                }
                s => segments.push(s),
            }
        }
        segments.join("/")
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_string(self) -> String {
        self.0
    }

    /// Get the parent directory path, if any.
    pub fn parent(&self) -> Option<&str> {
        self.0.rfind('/').map(|i| &self.0[..i])
    }

    /// Get the file extension, if any.
    pub fn extension(&self) -> Option<&str> {
        self.0.rsplit_once('.').map(|(_, ext)| ext)
    }

    /// Get the file name (last segment).
    pub fn file_name(&self) -> &str {
        self.0
            .rsplit_once('/')
            .map(|(_, name)| name)
            .unwrap_or(&self.0)
    }
}

impl fmt::Display for CanonicalPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl serde::Serialize for CanonicalPath {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

/// Content hash (xxHash64, lowercase hex, 16 chars).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ContentHash(String);

impl ContentHash {
    /// Create from a pre-computed hex string. Only called by hash module.
    pub fn new(hex: String) -> Self {
        Self(hex)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_string(self) -> String {
        self.0
    }
}

impl serde::Serialize for ContentHash {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

/// Cluster identifier.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ClusterId(String);

impl ClusterId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_string(self) -> String {
        self.0
    }
}

impl fmt::Display for ClusterId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl serde::Serialize for ClusterId {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

/// Exported/imported symbol name.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Symbol(String);

impl Symbol {
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_string(self) -> String {
        self.0
    }
}

impl serde::Serialize for Symbol {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

/// Set of known files for import resolution existence checks (D-024).
/// BTreeSet for deterministic iteration.
#[derive(Clone, Debug)]
pub struct FileSet(BTreeSet<CanonicalPath>);

impl FileSet {
    pub fn new() -> Self {
        Self(BTreeSet::new())
    }

    #[allow(clippy::should_implement_trait)]
    pub fn from_iter(iter: impl IntoIterator<Item = CanonicalPath>) -> Self {
        Self(iter.into_iter().collect())
    }

    pub fn contains(&self, path: &CanonicalPath) -> bool {
        self.0.contains(path)
    }

    pub fn iter(&self) -> impl Iterator<Item = &CanonicalPath> {
        self.0.iter()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl Default for FileSet {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- CanonicalPath normalization ---

    #[test]
    fn normalize_basic_path_unchanged() {
        let p = CanonicalPath::new("src/auth/login.ts");
        assert_eq!(p.as_str(), "src/auth/login.ts");
    }

    #[test]
    fn normalize_backslashes() {
        let p = CanonicalPath::new("src\\auth\\login.ts");
        assert_eq!(p.as_str(), "src/auth/login.ts");
    }

    #[test]
    fn normalize_leading_dot_slash() {
        let p = CanonicalPath::new("./src/auth/login.ts");
        assert_eq!(p.as_str(), "src/auth/login.ts");
    }

    #[test]
    fn normalize_double_slashes() {
        let p = CanonicalPath::new("src//auth//login.ts");
        assert_eq!(p.as_str(), "src/auth/login.ts");
    }

    #[test]
    fn normalize_triple_slashes() {
        let p = CanonicalPath::new("src///auth///login.ts");
        assert_eq!(p.as_str(), "src/auth/login.ts");
    }

    #[test]
    fn normalize_trailing_slash() {
        let p = CanonicalPath::new("src/auth/");
        assert_eq!(p.as_str(), "src/auth");
    }

    #[test]
    fn normalize_trailing_double_slash() {
        let p = CanonicalPath::new("src/auth//");
        assert_eq!(p.as_str(), "src/auth");
    }

    #[test]
    fn normalize_dot_dot_resolves() {
        let p = CanonicalPath::new("src/auth/../utils/helper.ts");
        assert_eq!(p.as_str(), "src/utils/helper.ts");
    }

    #[test]
    fn normalize_dot_dot_escaping_root_clamped() {
        // More .. than segments: should clamp at root (empty)
        let p = CanonicalPath::new("src/../../escape.ts");
        assert_eq!(p.as_str(), "escape.ts");
    }

    #[test]
    fn normalize_dot_dot_deep_escape() {
        let p = CanonicalPath::new("../../../etc/passwd");
        assert_eq!(p.as_str(), "etc/passwd");
    }

    #[test]
    fn normalize_only_dot_dot() {
        let p = CanonicalPath::new("../../..");
        assert_eq!(p.as_str(), "");
    }

    #[test]
    fn normalize_empty_path() {
        let p = CanonicalPath::new("");
        assert_eq!(p.as_str(), "");
    }

    #[test]
    fn normalize_single_dot() {
        let p = CanonicalPath::new(".");
        assert_eq!(p.as_str(), "");
    }

    #[test]
    fn normalize_dot_slash_only() {
        let p = CanonicalPath::new("./");
        assert_eq!(p.as_str(), "");
    }

    #[test]
    fn normalize_mixed_separators_and_traversal() {
        let p = CanonicalPath::new(".\\src\\..\\lib//utils\\.\\helper.ts");
        assert_eq!(p.as_str(), "lib/utils/helper.ts");
    }

    // --- CanonicalPath helper methods ---

    #[test]
    fn parent_returns_directory() {
        let p = CanonicalPath::new("src/auth/login.ts");
        assert_eq!(p.parent(), Some("src/auth"));
    }

    #[test]
    fn parent_returns_none_for_root_file() {
        let p = CanonicalPath::new("login.ts");
        assert_eq!(p.parent(), None);
    }

    #[test]
    fn extension_returns_ext() {
        let p = CanonicalPath::new("src/auth/login.ts");
        assert_eq!(p.extension(), Some("ts"));
    }

    #[test]
    fn file_name_returns_last_segment() {
        let p = CanonicalPath::new("src/auth/login.ts");
        assert_eq!(p.file_name(), "login.ts");
    }

    // --- FileSet ---

    #[test]
    fn fileset_contains_and_len() {
        let fs = FileSet::from_iter(vec![CanonicalPath::new("a.ts"), CanonicalPath::new("b.ts")]);
        assert_eq!(fs.len(), 2);
        assert!(fs.contains(&CanonicalPath::new("a.ts")));
        assert!(!fs.contains(&CanonicalPath::new("c.ts")));
    }

    #[test]
    fn fileset_deterministic_order() {
        let fs = FileSet::from_iter(vec![
            CanonicalPath::new("z.ts"),
            CanonicalPath::new("a.ts"),
            CanonicalPath::new("m.ts"),
        ]);
        let paths: Vec<&str> = fs.iter().map(|p| p.as_str()).collect();
        assert_eq!(paths, vec!["a.ts", "m.ts", "z.ts"]);
    }
}
