use std::collections::BTreeSet;

use crate::model::workspace::WorkspaceInfo;
use crate::model::{CanonicalPath, FileSet};
use crate::parser::traits::{ImportKind, ImportResolver, LanguageParser, RawExport, RawImport};

/// Markdown parser — extracts file link references from `.md` files.
struct MarkdownParser;

impl LanguageParser for MarkdownParser {
    fn language(&self) -> &str {
        "markdown"
    }

    fn extensions(&self) -> &[&str] {
        &["md"]
    }

    fn tree_sitter_language(&self) -> tree_sitter::Language {
        tree_sitter::Language::from(tree_sitter_md::LANGUAGE)
    }

    fn extract_imports(&self, tree: &tree_sitter::Tree, source: &[u8]) -> Vec<RawImport> {
        let mut seen = BTreeSet::new();
        let root = tree.root_node();
        collect_links(&root, source, &mut seen);
        seen.into_iter()
            .map(|path| RawImport {
                path,
                symbols: vec![],
                is_type_only: false,
                kind: ImportKind::Link,
            })
            .collect()
    }

    fn extract_exports(&self, _tree: &tree_sitter::Tree, _source: &[u8]) -> Vec<RawExport> {
        Vec::new()
    }
}

/// Recursively walk the block-level AST and collect link destinations.
fn collect_links(node: &tree_sitter::Node, source: &[u8], seen: &mut BTreeSet<String>) {
    // link_reference_definition contains link_destination children
    if node.kind() == "link_reference_definition" {
        for i in 0..node.child_count() {
            if let Some(child) = node.child(i) {
                if child.kind() == "link_destination" {
                    let text = node_text(&child, source);
                    if let Some(cleaned) = clean_link(&text) {
                        seen.insert(cleaned);
                    }
                }
            }
        }
    }

    // inline nodes contain raw text — extract [text](url) and ![alt](url) patterns
    if node.kind() == "inline" {
        let text = node_text(node, source);
        extract_inline_links(&text, seen);
    }

    // Recurse into children
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            collect_links(&child, source, seen);
        }
    }
}

/// Extract inline link destinations from raw text using pattern matching.
/// Matches `[text](url)` and `![alt](url)` patterns.
fn extract_inline_links(text: &str, seen: &mut BTreeSet<String>) {
    let bytes = text.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i < len {
        // Look for `](` which signals the boundary between link text and destination
        if i + 1 < len && bytes[i] == b']' && bytes[i + 1] == b'(' {
            // Find the matching closing paren
            let start = i + 2;
            if let Some(end) = find_closing_paren(bytes, start) {
                let url = &text[start..end];
                if let Some(cleaned) = clean_link(url.trim()) {
                    seen.insert(cleaned);
                }
                i = end + 1;
                continue;
            }
        }
        i += 1;
    }
}

/// Find the closing `)` for a link destination, handling nested parens.
fn find_closing_paren(bytes: &[u8], start: usize) -> Option<usize> {
    let mut depth = 1;
    let mut i = start;
    while i < bytes.len() {
        match bytes[i] {
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            b'\n' => return None, // link destinations don't span lines
            _ => {}
        }
        i += 1;
    }
    None
}

/// Filter and clean a link destination. Returns None for URLs, empty, fragment-only.
fn clean_link(raw: &str) -> Option<String> {
    let trimmed = raw.trim();

    // Skip empty
    if trimmed.is_empty() {
        return None;
    }

    // Skip fragment-only links
    if trimmed.starts_with('#') {
        return None;
    }

    // Skip scheme URLs
    if trimmed.starts_with("http://")
        || trimmed.starts_with("https://")
        || trimmed.starts_with("ftp://")
        || trimmed.starts_with("mailto:")
        || trimmed.starts_with("data:")
    {
        return None;
    }

    // Strip angle brackets if present (e.g., <./path>)
    let path = if trimmed.starts_with('<') && trimmed.ends_with('>') {
        &trimmed[1..trimmed.len() - 1]
    } else {
        trimmed
    };

    // Strip fragment
    let path = match path.find('#') {
        Some(pos) => &path[..pos],
        None => path,
    };

    // Strip query string
    let path = match path.find('?') {
        Some(pos) => &path[..pos],
        None => path,
    };

    // After stripping, might be empty
    if path.is_empty() {
        return None;
    }

    Some(path.to_string())
}

fn node_text(node: &tree_sitter::Node, source: &[u8]) -> String {
    node.utf8_text(source).unwrap_or("").to_string()
}

/// Markdown import resolver — resolves relative links to known project files.
struct MarkdownResolver;

impl ImportResolver for MarkdownResolver {
    fn resolve(
        &self,
        import: &RawImport,
        from_file: &CanonicalPath,
        known_files: &FileSet,
        _workspace: Option<&WorkspaceInfo>,
    ) -> Option<CanonicalPath> {
        let link_path = &import.path;

        // Get parent directory of the source file
        let from_str = from_file.as_str();
        let parent = match from_str.rfind('/') {
            Some(pos) => &from_str[..pos],
            None => "",
        };

        // Resolve relative to parent
        let resolved = if parent.is_empty() {
            link_path.to_string()
        } else {
            format!("{}/{}", parent, link_path)
        };

        // Normalize path segments (handle ../ and ./)
        let normalized = normalize_path(&resolved);

        let candidate = CanonicalPath::new(&normalized);
        if known_files.contains(&candidate) {
            return Some(candidate);
        }

        None
    }
}

/// Normalize a path by resolving `.` and `..` segments.
fn normalize_path(path: &str) -> String {
    let mut parts: Vec<&str> = Vec::new();
    for segment in path.split('/') {
        match segment {
            "." | "" => {
                // Skip current-dir markers and empty segments (from leading /)
                // But preserve leading empty for absolute paths
                if parts.is_empty() && segment.is_empty() && path.starts_with('/') {
                    parts.push("");
                }
            }
            ".." => {
                if let Some(last) = parts.last() {
                    if *last != ".." && !last.is_empty() {
                        parts.pop();
                    } else {
                        parts.push("..");
                    }
                } else {
                    parts.push("..");
                }
            }
            other => parts.push(other),
        }
    }
    if parts.is_empty() {
        ".".to_string()
    } else {
        parts.join("/")
    }
}

pub(crate) fn parser() -> Box<dyn LanguageParser> {
    Box::new(MarkdownParser)
}

pub(crate) fn resolver() -> Box<dyn ImportResolver> {
    Box::new(MarkdownResolver)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(source: &str) -> tree_sitter::Tree {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter::Language::from(tree_sitter_md::LANGUAGE))
            .unwrap();
        parser.parse(source, None).unwrap()
    }

    fn md_imports(source: &str) -> Vec<RawImport> {
        let tree = parse(source);
        MarkdownParser.extract_imports(&tree, source.as_bytes())
    }

    fn md_exports(source: &str) -> Vec<RawExport> {
        let tree = parse(source);
        MarkdownParser.extract_exports(&tree, source.as_bytes())
    }

    // ---- Import extraction tests ----

    #[test]
    fn inline_link() {
        let source = "See [guide](./docs/guide.md) for details.\n";
        let result = md_imports(source);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].path, "./docs/guide.md");
        assert_eq!(result[0].kind, ImportKind::Link);
    }

    #[test]
    fn image_link() {
        let source = "Logo: ![logo](./assets/logo.png)\n";
        let result = md_imports(source);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].path, "./assets/logo.png");
    }

    #[test]
    fn link_reference_definition() {
        let source = "[ref1]: ./src/lib.rs \"Library\"\n";
        let result = md_imports(source);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].path, "./src/lib.rs");
    }

    #[test]
    fn filters_http_urls() {
        let source = "Visit [site](https://example.com) and [other](http://foo.bar).\n";
        let result = md_imports(source);
        assert!(result.is_empty());
    }

    #[test]
    fn filters_mailto_and_data() {
        let source = "Email [us](mailto:hi@example.com). Data [img](data:image/png;base64,abc).\n";
        let result = md_imports(source);
        assert!(result.is_empty());
    }

    #[test]
    fn filters_fragment_only() {
        let source = "See [section](#heading-1) for more.\n";
        let result = md_imports(source);
        assert!(result.is_empty());
    }

    #[test]
    fn strips_fragment_from_path() {
        let source = "See [docs](./README.md#section) for more.\n";
        let result = md_imports(source);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].path, "./README.md");
    }

    #[test]
    fn strips_query_string() {
        let source = "See [docs](./README.md?v=2) for more.\n";
        let result = md_imports(source);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].path, "./README.md");
    }

    #[test]
    fn deduplicates_links() {
        let source = "See [a](./foo.md) and [b](./foo.md) and [c](./bar.md).\n";
        let result = md_imports(source);
        assert_eq!(result.len(), 2);
        let paths: Vec<&str> = result.iter().map(|r| r.path.as_str()).collect();
        assert!(paths.contains(&"./foo.md"));
        assert!(paths.contains(&"./bar.md"));
    }

    #[test]
    fn multiple_link_types() {
        let source = "# Project\n\nSee [guide](./docs/guide.md) and ![logo](./assets/logo.png).\n\n[ref]: ./src/lib.rs\n";
        let result = md_imports(source);
        assert_eq!(result.len(), 3);
    }

    #[test]
    fn empty_source() {
        let result = md_imports("");
        assert!(result.is_empty());
    }

    #[test]
    fn empty_link_destination() {
        let source = "Click [here]() for nothing.\n";
        let result = md_imports(source);
        assert!(result.is_empty());
    }

    // ---- Export tests ----

    #[test]
    fn exports_always_empty() {
        let result = md_exports("# Heading\n\nSome content.\n");
        assert!(result.is_empty());
    }

    // ---- Resolver tests ----

    #[test]
    fn resolve_relative_link() {
        let files: FileSet = vec![
            CanonicalPath::new("docs/guide.md"),
            CanonicalPath::new("docs/README.md"),
        ]
        .into_iter()
        .collect();

        let import = RawImport {
            path: "./guide.md".to_string(),
            symbols: vec![],
            is_type_only: false,
            kind: ImportKind::Link,
        };
        let from = CanonicalPath::new("docs/README.md");
        let resolved = MarkdownResolver.resolve(&import, &from, &files, None);
        assert_eq!(resolved, Some(CanonicalPath::new("docs/guide.md")));
    }

    #[test]
    fn resolve_parent_relative_link() {
        let files: FileSet = vec![
            CanonicalPath::new("README.md"),
            CanonicalPath::new("docs/guide.md"),
        ]
        .into_iter()
        .collect();

        let import = RawImport {
            path: "../README.md".to_string(),
            symbols: vec![],
            is_type_only: false,
            kind: ImportKind::Link,
        };
        let from = CanonicalPath::new("docs/guide.md");
        let resolved = MarkdownResolver.resolve(&import, &from, &files, None);
        assert_eq!(resolved, Some(CanonicalPath::new("README.md")));
    }

    #[test]
    fn resolve_unresolvable_link() {
        let files = FileSet::new();
        let import = RawImport {
            path: "./nonexistent.md".to_string(),
            symbols: vec![],
            is_type_only: false,
            kind: ImportKind::Link,
        };
        let from = CanonicalPath::new("docs/README.md");
        let resolved = MarkdownResolver.resolve(&import, &from, &files, None);
        assert_eq!(resolved, None);
    }

    // ---- Additional import property tests ----

    #[test]
    fn relative_path_preserved() {
        let source = "Go to [sibling](../sibling.md) file.\n";
        let result = md_imports(source);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].path, "../sibling.md");
    }

    #[test]
    fn mixed_valid_and_filtered() {
        let source = "See [local](./local.md) and [ext](https://example.com) and [other](other.md) and [frag](#top).\n";
        let result = md_imports(source);
        assert_eq!(result.len(), 2);
        let paths: Vec<&str> = result.iter().map(|r| r.path.as_str()).collect();
        assert!(paths.contains(&"./local.md"));
        assert!(paths.contains(&"other.md"));
    }

    #[test]
    fn no_links_no_imports() {
        let source = "# Hello World\n\nJust plain text with no links at all.\n";
        let result = md_imports(source);
        assert!(result.is_empty());
    }

    #[test]
    fn import_symbols_empty() {
        let source = "See [guide](guide.md) for details.\n";
        let result = md_imports(source);
        assert_eq!(result.len(), 1);
        assert!(result[0].symbols.is_empty());
    }

    #[test]
    fn import_not_type_only() {
        let source = "See [guide](guide.md) for details.\n";
        let result = md_imports(source);
        assert_eq!(result.len(), 1);
        assert!(!result[0].is_type_only);
    }

    #[test]
    fn link_with_fragment_stripped() {
        let source = "See [section](file.md#section) for more.\n";
        let result = md_imports(source);
        assert_eq!(result.len(), 1);
        assert!(!result[0].path.contains("#section"));
        assert_eq!(result[0].path, "file.md");
    }

    // ---- Path normalization tests ----

    #[test]
    fn normalize_dot_segments() {
        assert_eq!(normalize_path("a/./b"), "a/b");
        assert_eq!(normalize_path("a/../b"), "b");
        assert_eq!(normalize_path("a/b/../c/./d"), "a/c/d");
    }

    #[test]
    fn normalize_leading_dotdot() {
        assert_eq!(normalize_path("../a"), "../a");
    }
}
