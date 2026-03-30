/// Parsed go.mod configuration.
#[derive(Clone)]
pub struct GoModConfig {
    /// The module path (e.g., `github.com/user/repo`).
    pub module_path: String,
}

/// Parse a go.mod file to extract the module path.
///
/// Finds the first line starting with `module ` (case-sensitive) and extracts
/// the module path. Returns `None` if no module directive is found.
pub fn parse_go_mod(content: &str) -> Option<GoModConfig> {
    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("module ") {
            // Strip inline comments and whitespace
            let module_path = rest
                .split("//")
                .next()
                .unwrap_or("")
                .trim()
                .to_string();
            if !module_path.is_empty() {
                return Some(GoModConfig { module_path });
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_go_mod() {
        let content = "module github.com/user/repo\n\ngo 1.21\n";
        let config = parse_go_mod(content).unwrap();
        assert_eq!(config.module_path, "github.com/user/repo");
    }

    #[test]
    fn go_mod_with_require_replace() {
        let content = r#"module github.com/user/repo

go 1.21

require (
    github.com/pkg/errors v0.9.1
)

replace github.com/old => github.com/new v1.0.0
"#;
        let config = parse_go_mod(content).unwrap();
        assert_eq!(config.module_path, "github.com/user/repo");
    }

    #[test]
    fn empty_content() {
        assert!(parse_go_mod("").is_none());
    }

    #[test]
    fn no_module_line() {
        let content = "go 1.21\nrequire github.com/pkg/errors v0.9.1\n";
        assert!(parse_go_mod(content).is_none());
    }

    #[test]
    fn module_path_with_version_suffix() {
        let content = "module github.com/user/repo/v2\n\ngo 1.21\n";
        let config = parse_go_mod(content).unwrap();
        assert_eq!(config.module_path, "github.com/user/repo/v2");
    }

    #[test]
    fn module_with_inline_comment() {
        let content = "module github.com/user/repo // main module\n";
        let config = parse_go_mod(content).unwrap();
        assert_eq!(config.module_path, "github.com/user/repo");
    }
}
