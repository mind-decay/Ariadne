/// Parsed pyproject.toml configuration.
#[derive(Clone)]
pub struct PyProjectConfig {
    /// Whether the project uses src-layout (packages under `src/`).
    pub src_layout: bool,
    /// The package name from `[project]` section, if found.
    pub package_name: Option<String>,
}

/// Parse a pyproject.toml file to detect src-layout and extract package name.
///
/// Uses simple string parsing (no TOML crate dependency).
/// Returns `None` if the content is empty or not parseable.
pub fn parse_pyproject(content: &str) -> Option<PyProjectConfig> {
    if content.trim().is_empty() {
        return None;
    }

    let src_layout = detect_src_layout(content);
    let package_name = extract_package_name(content);

    Some(PyProjectConfig {
        src_layout,
        package_name,
    })
}

/// Detect src-layout by looking for setuptools configuration with `where = ["src"]`.
fn detect_src_layout(content: &str) -> bool {
    let mut in_packages_find = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed == "[tool.setuptools.packages.find]" {
            in_packages_find = true;
            continue;
        }
        if trimmed.starts_with('[') && in_packages_find {
            // Entered a new section
            in_packages_find = false;
        }
        if in_packages_find && trimmed.starts_with("where") {
            if trimmed.contains("\"src\"") || trimmed.contains("'src'") {
                return true;
            }
        }
    }
    false
}

/// Extract the package name from the `[project]` section.
fn extract_package_name(content: &str) -> Option<String> {
    let mut in_project = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed == "[project]" {
            in_project = true;
            continue;
        }
        if trimmed.starts_with('[') && in_project {
            in_project = false;
        }
        if in_project && trimmed.starts_with("name") {
            if let Some(eq_pos) = trimmed.find('=') {
                let value = trimmed[eq_pos + 1..].trim();
                let name = value
                    .trim_matches('"')
                    .trim_matches('\'')
                    .trim()
                    .to_string();
                if !name.is_empty() {
                    return Some(name);
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pyproject_with_src_layout() {
        let content = r#"
[project]
name = "mypackage"

[tool.setuptools.packages.find]
where = ["src"]
"#;
        let config = parse_pyproject(content).unwrap();
        assert!(config.src_layout);
        assert_eq!(config.package_name.as_deref(), Some("mypackage"));
    }

    #[test]
    fn pyproject_without_src_layout() {
        let content = r#"
[project]
name = "mypackage"
version = "1.0.0"

[build-system]
requires = ["setuptools"]
"#;
        let config = parse_pyproject(content).unwrap();
        assert!(!config.src_layout);
    }

    #[test]
    fn pyproject_with_package_name() {
        let content = r#"
[project]
name = "my-cool-package"
version = "0.1.0"
"#;
        let config = parse_pyproject(content).unwrap();
        assert_eq!(config.package_name.as_deref(), Some("my-cool-package"));
    }

    #[test]
    fn empty_content() {
        assert!(parse_pyproject("").is_none());
    }

    #[test]
    fn whitespace_only() {
        assert!(parse_pyproject("   \n\n  ").is_none());
    }

    #[test]
    fn no_project_section() {
        let content = r#"
[build-system]
requires = ["setuptools"]
build-backend = "setuptools.build_meta"
"#;
        let config = parse_pyproject(content).unwrap();
        assert!(!config.src_layout);
        assert!(config.package_name.is_none());
    }
}
