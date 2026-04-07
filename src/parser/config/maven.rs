use std::path::{Path, PathBuf};

use crate::diagnostic::{DiagnosticCollector, Warning, WarningCode};
use crate::model::CanonicalPath;

/// Parsed Maven POM configuration.
#[derive(Clone, Debug)]
pub struct MavenConfig {
    pub config_path: PathBuf,
    pub config_dir: PathBuf,
    pub group_id: Option<String>,
    pub artifact_id: Option<String>,
    pub version: Option<String>,
    pub packaging: Option<String>,
    pub source_directory: Option<String>,
    pub test_source_directory: Option<String>,
    pub modules: Vec<String>,
    pub dependencies: Vec<MavenDep>,
    pub parent: Option<MavenParentRef>,
}

/// A `<dependency>` entry from a POM file.
#[derive(Clone, Debug)]
pub struct MavenDep {
    pub group_id: String,
    pub artifact_id: String,
    pub version: Option<String>,
    pub scope: Option<String>,
}

/// A `<parent>` reference from a POM file.
#[derive(Clone, Debug)]
pub struct MavenParentRef {
    pub group_id: String,
    pub artifact_id: String,
    pub version: Option<String>,
    pub relative_path: Option<String>,
}

/// Helper: extract the text content of the first child element with the given name.
fn child_text(node: &roxmltree::Node, name: &str) -> Option<String> {
    node.children()
        .find(|n| n.is_element() && n.tag_name().name() == name)
        .and_then(|n| n.text())
        .map(|t| t.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Parse a `pom.xml` file.
///
/// Uses roxmltree for XML parsing. If the XML is malformed, emits W039 and returns None.
pub fn parse_pom_xml(
    content: &str,
    pom_path: &Path,
    diag: &DiagnosticCollector,
) -> Option<MavenConfig> {
    let doc = match roxmltree::Document::parse(content) {
        Ok(d) => d,
        Err(e) => {
            diag.warn(Warning {
                code: WarningCode::W039MavenParseError,
                path: CanonicalPath::new(pom_path.to_string_lossy().to_string()),
                message: format!("failed to parse pom.xml: {e}"),
                detail: None,
            });
            return None;
        }
    };

    let root = doc.root_element();

    let group_id = child_text(&root, "groupId");
    let artifact_id = child_text(&root, "artifactId");
    let version = child_text(&root, "version");
    let packaging = child_text(&root, "packaging");

    // Parse <parent>
    let parent = root
        .children()
        .find(|n| n.is_element() && n.tag_name().name() == "parent")
        .and_then(|parent_node| {
            let gid = child_text(&parent_node, "groupId")?;
            let aid = child_text(&parent_node, "artifactId")?;
            Some(MavenParentRef {
                group_id: gid,
                artifact_id: aid,
                version: child_text(&parent_node, "version"),
                relative_path: child_text(&parent_node, "relativePath"),
            })
        });

    // Parse <modules>
    let mut modules = Vec::new();
    if let Some(modules_node) = root
        .children()
        .find(|n| n.is_element() && n.tag_name().name() == "modules")
    {
        for child in modules_node.children() {
            if child.is_element() && child.tag_name().name() == "module" {
                if let Some(text) = child.text() {
                    let trimmed = text.trim();
                    if !trimmed.is_empty() {
                        modules.push(trimmed.to_string());
                    }
                }
            }
        }
    }

    // Parse <dependencies>
    let mut dependencies = Vec::new();
    if let Some(deps_node) = root
        .children()
        .find(|n| n.is_element() && n.tag_name().name() == "dependencies")
    {
        for child in deps_node.children() {
            if child.is_element() && child.tag_name().name() == "dependency" {
                if let (Some(gid), Some(aid)) =
                    (child_text(&child, "groupId"), child_text(&child, "artifactId"))
                {
                    dependencies.push(MavenDep {
                        group_id: gid,
                        artifact_id: aid,
                        version: child_text(&child, "version"),
                        scope: child_text(&child, "scope"),
                    });
                }
            }
        }
    }

    // Parse <build> for source directories
    let mut source_directory = None;
    let mut test_source_directory = None;
    if let Some(build_node) = root
        .children()
        .find(|n| n.is_element() && n.tag_name().name() == "build")
    {
        source_directory = child_text(&build_node, "sourceDirectory");
        test_source_directory = child_text(&build_node, "testSourceDirectory");
    }

    let config_dir = pom_path
        .parent()
        .unwrap_or(Path::new(""))
        .to_path_buf();

    Some(MavenConfig {
        config_path: pom_path.to_path_buf(),
        config_dir,
        group_id,
        artifact_id,
        version,
        packaging,
        source_directory,
        test_source_directory,
        modules,
        dependencies,
        parent,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostic::DiagnosticCollector;

    fn make_diagnostics() -> DiagnosticCollector {
        DiagnosticCollector::new()
    }

    #[test]
    fn test_parse_pom_xml_basic() {
        let diag = make_diagnostics();
        let content = r#"<?xml version="1.0" encoding="UTF-8"?>
<project xmlns="http://maven.apache.org/POM/4.0.0">
    <groupId>com.example</groupId>
    <artifactId>my-app</artifactId>
    <version>1.0.0</version>
    <packaging>jar</packaging>
</project>"#;

        let config = parse_pom_xml(content, Path::new("pom.xml"), &diag)
            .expect("should parse basic pom");

        assert_eq!(config.group_id.as_deref(), Some("com.example"));
        assert_eq!(config.artifact_id.as_deref(), Some("my-app"));
        assert_eq!(config.version.as_deref(), Some("1.0.0"));
        assert_eq!(config.packaging.as_deref(), Some("jar"));
        assert!(config.modules.is_empty());
        assert!(config.dependencies.is_empty());
        assert!(config.parent.is_none());
    }

    #[test]
    fn test_parse_pom_xml_with_modules() {
        let diag = make_diagnostics();
        let content = r#"<?xml version="1.0" encoding="UTF-8"?>
<project>
    <groupId>com.example</groupId>
    <artifactId>parent</artifactId>
    <version>1.0.0</version>
    <packaging>pom</packaging>
    <modules>
        <module>core</module>
        <module>web</module>
        <module>api</module>
    </modules>
</project>"#;

        let config = parse_pom_xml(content, Path::new("pom.xml"), &diag)
            .expect("should parse pom with modules");

        assert_eq!(config.modules, vec!["core", "web", "api"]);
    }

    #[test]
    fn test_parse_pom_xml_with_parent() {
        let diag = make_diagnostics();
        let content = r#"<?xml version="1.0" encoding="UTF-8"?>
<project>
    <parent>
        <groupId>org.springframework.boot</groupId>
        <artifactId>spring-boot-starter-parent</artifactId>
        <version>3.1.0</version>
        <relativePath>../pom.xml</relativePath>
    </parent>
    <artifactId>my-service</artifactId>
</project>"#;

        let config = parse_pom_xml(content, Path::new("service/pom.xml"), &diag)
            .expect("should parse pom with parent");

        let parent = config.parent.expect("should have parent ref");
        assert_eq!(parent.group_id, "org.springframework.boot");
        assert_eq!(parent.artifact_id, "spring-boot-starter-parent");
        assert_eq!(parent.version.as_deref(), Some("3.1.0"));
        assert_eq!(parent.relative_path.as_deref(), Some("../pom.xml"));
        assert_eq!(config.artifact_id.as_deref(), Some("my-service"));
    }

    #[test]
    fn test_parse_pom_xml_dependencies() {
        let diag = make_diagnostics();
        let content = r#"<?xml version="1.0" encoding="UTF-8"?>
<project>
    <groupId>com.example</groupId>
    <artifactId>app</artifactId>
    <dependencies>
        <dependency>
            <groupId>org.springframework</groupId>
            <artifactId>spring-core</artifactId>
            <version>6.0.0</version>
        </dependency>
        <dependency>
            <groupId>com.fasterxml.jackson.core</groupId>
            <artifactId>jackson-databind</artifactId>
            <version>2.15.0</version>
            <scope>compile</scope>
        </dependency>
        <dependency>
            <groupId>junit</groupId>
            <artifactId>junit</artifactId>
            <version>4.13.2</version>
            <scope>test</scope>
        </dependency>
    </dependencies>
</project>"#;

        let config = parse_pom_xml(content, Path::new("pom.xml"), &diag)
            .expect("should parse pom with dependencies");

        assert_eq!(config.dependencies.len(), 3);

        assert_eq!(config.dependencies[0].group_id, "org.springframework");
        assert_eq!(config.dependencies[0].artifact_id, "spring-core");
        assert_eq!(config.dependencies[0].version.as_deref(), Some("6.0.0"));
        assert!(config.dependencies[0].scope.is_none());

        assert_eq!(
            config.dependencies[1].group_id,
            "com.fasterxml.jackson.core"
        );
        assert_eq!(config.dependencies[1].artifact_id, "jackson-databind");
        assert_eq!(config.dependencies[1].scope.as_deref(), Some("compile"));

        assert_eq!(config.dependencies[2].group_id, "junit");
        assert_eq!(config.dependencies[2].scope.as_deref(), Some("test"));
    }

    #[test]
    fn test_parse_pom_xml_malformed() {
        let diag = make_diagnostics();
        let content = "not xml";

        let result = parse_pom_xml(content, Path::new("bad/pom.xml"), &diag);
        assert!(result.is_none());

        let report = diag.drain();
        assert!(report
            .warnings
            .iter()
            .any(|w| w.code == WarningCode::W039MavenParseError));
    }
}
