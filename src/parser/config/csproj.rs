use std::path::{Path, PathBuf};

use crate::diagnostic::{DiagnosticCollector, Warning, WarningCode};
use crate::model::CanonicalPath;

/// Parsed .csproj configuration.
#[derive(Clone, Debug)]
pub struct CsprojConfig {
    /// Path to the .csproj file (project-relative).
    pub project_path: PathBuf,
    /// Directory containing the .csproj file.
    pub project_dir: PathBuf,
    /// `<TargetFramework>` or first `<TargetFrameworks>` entry.
    pub target_framework: Option<String>,
    /// `<RootNamespace>` — defaults to None (resolver handles fallback to project name).
    pub root_namespace: Option<String>,
    /// `<AssemblyName>` — defaults to project filename without extension.
    pub assembly_name: Option<String>,
    /// `<ProjectReference Include="...">` entries, paths normalized to forward slashes.
    pub project_references: Vec<ProjectRef>,
    /// `<PackageReference Include="...">` entries.
    pub package_references: Vec<PackageRef>,
}

/// A `<ProjectReference>` entry from a .csproj file.
#[derive(Clone, Debug)]
pub struct ProjectRef {
    /// Relative path to the referenced .csproj (normalized, forward slashes).
    pub path: PathBuf,
    /// Resolved project-relative path to the referenced .csproj.
    pub resolved_path: Option<PathBuf>,
}

/// A `<PackageReference>` entry from a .csproj file.
#[derive(Clone, Debug)]
pub struct PackageRef {
    /// NuGet package name.
    pub name: String,
    /// Version string (may be range).
    pub version: Option<String>,
}

/// Parsed .sln solution structure.
#[derive(Clone, Debug)]
pub struct DotnetSolutionInfo {
    /// Path to the .sln file (project-relative).
    pub solution_path: PathBuf,
    /// Project entries from the .sln file.
    pub projects: Vec<SlnProjectEntry>,
}

/// A project entry parsed from a .sln file.
#[derive(Clone, Debug)]
pub struct SlnProjectEntry {
    /// Display name from .sln.
    pub name: String,
    /// Relative path to .csproj from .sln directory.
    pub relative_path: PathBuf,
    /// Project type GUID (for distinguishing C# projects from solution folders).
    pub type_guid: String,
}

/// Solution folder type GUID — these entries are filtered out.
const SOLUTION_FOLDER_GUID: &str = "{2150E333-8FDC-42A3-9474-1A3956D46DE8}";

/// Parse a .csproj file content string into `CsprojConfig`.
///
/// Uses roxmltree for XML parsing. If the XML is malformed, emits W034 and returns None.
pub fn parse_csproj(
    content: &str,
    csproj_path: &Path,
    diagnostics: &DiagnosticCollector,
) -> Option<CsprojConfig> {
    let doc = match roxmltree::Document::parse(content) {
        Ok(d) => d,
        Err(e) => {
            diagnostics.warn(Warning {
                code: WarningCode::W034CsprojParseError,
                path: CanonicalPath::new(csproj_path.to_string_lossy().to_string()),
                message: format!("failed to parse .csproj: {e}"),
                detail: None,
            });
            return None;
        }
    };

    let mut target_framework: Option<String> = None;
    let mut root_namespace: Option<String> = None;
    let mut assembly_name: Option<String> = None;
    let mut project_references: Vec<ProjectRef> = Vec::new();
    let mut package_references: Vec<PackageRef> = Vec::new();

    for node in doc.descendants() {
        if !node.is_element() {
            continue;
        }

        match node.tag_name().name() {
            "TargetFramework" => {
                if target_framework.is_none() {
                    if let Some(text) = node.text() {
                        let trimmed = text.trim();
                        if !trimmed.is_empty() {
                            target_framework = Some(trimmed.to_string());
                        }
                    }
                }
            }
            "TargetFrameworks" => {
                if target_framework.is_none() {
                    if let Some(text) = node.text() {
                        // Semicolon-delimited, take first entry
                        let first = text.split(';').next().unwrap_or("").trim();
                        if !first.is_empty() {
                            target_framework = Some(first.to_string());
                        }
                    }
                }
            }
            "RootNamespace" => {
                if root_namespace.is_none() {
                    if let Some(text) = node.text() {
                        let trimmed = text.trim();
                        if !trimmed.is_empty() {
                            root_namespace = Some(trimmed.to_string());
                        }
                    }
                }
            }
            "AssemblyName" => {
                if assembly_name.is_none() {
                    if let Some(text) = node.text() {
                        let trimmed = text.trim();
                        if !trimmed.is_empty() {
                            assembly_name = Some(trimmed.to_string());
                        }
                    }
                }
            }
            "ProjectReference" => {
                if let Some(include) = node.attribute("Include") {
                    let normalized = include.replace('\\', "/");
                    let path = PathBuf::from(normalized);
                    // Resolve relative to csproj directory
                    let csproj_dir = csproj_path.parent().unwrap_or(Path::new(""));
                    let resolved = normalize_relative_path(&csproj_dir.join(&path));
                    project_references.push(ProjectRef {
                        path,
                        resolved_path: Some(resolved),
                    });
                }
            }
            "PackageReference" => {
                if let Some(include) = node.attribute("Include") {
                    let name = include.trim().to_string();
                    let version = node.attribute("Version").map(|v| v.trim().to_string());
                    if !name.is_empty() {
                        package_references.push(PackageRef { name, version });
                    }
                }
            }
            _ => {}
        }
    }

    let project_dir = csproj_path
        .parent()
        .unwrap_or(Path::new(""))
        .to_path_buf();

    Some(CsprojConfig {
        project_path: csproj_path.to_path_buf(),
        project_dir,
        target_framework,
        root_namespace,
        assembly_name,
        project_references,
        package_references,
    })
}

/// Normalize a relative path by resolving `..` and `.` components.
///
/// This is a purely lexical normalization (no filesystem access).
fn normalize_relative_path(path: &Path) -> PathBuf {
    let mut components: Vec<&std::ffi::OsStr> = Vec::new();
    for component in path.components() {
        match component {
            std::path::Component::ParentDir => {
                components.pop();
            }
            std::path::Component::CurDir => {}
            std::path::Component::Normal(c) => {
                components.push(c);
            }
            _ => {}
        }
    }
    components.iter().collect()
}

/// Parse a .sln file content string into `DotnetSolutionInfo`.
///
/// Uses line-based parsing. Project lines follow the pattern:
/// `Project("{TYPE_GUID}") = "NAME", "PATH", "PROJECT_GUID"`
///
/// Solution folder entries (type GUID `{2150E333-...}`) are filtered out.
/// If parsing fails entirely, emits W035 and returns None.
pub fn parse_sln(
    content: &str,
    sln_path: &Path,
    _diagnostics: &DiagnosticCollector,
) -> Option<DotnetSolutionInfo> {
    let mut projects = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with("Project(\"") {
            continue;
        }

        if let Some(entry) = parse_sln_project_line(trimmed) {
            // Filter out solution folders
            if entry.type_guid.eq_ignore_ascii_case(SOLUTION_FOLDER_GUID) {
                continue;
            }
            projects.push(entry);
        }
    }

    Some(DotnetSolutionInfo {
        solution_path: sln_path.to_path_buf(),
        projects,
    })
}

/// Parse a single Project(...) line from a .sln file.
///
/// Expected format:
/// `Project("{TYPE_GUID}") = "NAME", "PATH", "PROJECT_GUID"`
fn parse_sln_project_line(line: &str) -> Option<SlnProjectEntry> {
    // Extract type GUID: between first pair of quotes after Project(
    let after_project = line.strip_prefix("Project(\"")?;
    let type_guid_end = after_project.find('"')?;
    let type_guid = after_project[..type_guid_end].to_string();

    // Find " = " separator
    let rest = &after_project[type_guid_end..];
    let eq_pos = rest.find(" = ")?;
    let after_eq = &rest[eq_pos + 3..];

    // Extract quoted fields: "NAME", "PATH", "GUID"
    let fields = extract_quoted_fields(after_eq);
    if fields.len() < 2 {
        return None;
    }

    let name = fields[0].clone();
    let raw_path = fields[1].replace('\\', "/");
    let relative_path = PathBuf::from(raw_path);

    Some(SlnProjectEntry {
        name,
        relative_path,
        type_guid,
    })
}

/// Extract quoted string fields from a comma-separated line.
fn extract_quoted_fields(s: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut rest = s;

    while let Some(pos) = rest.find('"') {
        rest = &rest[pos + 1..];

        // Find closing quote
        let end = match rest.find('"') {
            Some(pos) => pos,
            None => break,
        };
        fields.push(rest[..end].to_string());
        rest = &rest[end + 1..];
    }

    fields
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostic::DiagnosticCollector;

    fn make_diagnostics() -> DiagnosticCollector {
        DiagnosticCollector::new()
    }

    // --- parse_csproj tests ---

    #[test]
    fn test_parse_csproj_basic() {
        let diag = make_diagnostics();
        let content = r#"<Project Sdk="Microsoft.NET.Sdk">
  <PropertyGroup>
    <TargetFramework>net8.0</TargetFramework>
    <RootNamespace>MyApp.Core</RootNamespace>
    <AssemblyName>MyApp.Core</AssemblyName>
  </PropertyGroup>
  <ItemGroup>
    <ProjectReference Include="..\MyApp.Data\MyApp.Data.csproj" />
  </ItemGroup>
  <ItemGroup>
    <PackageReference Include="Newtonsoft.Json" Version="13.0.3" />
    <PackageReference Include="Serilog" Version="3.1.1" />
  </ItemGroup>
</Project>"#;

        let config = parse_csproj(content, Path::new("src/MyApp.Core/MyApp.Core.csproj"), &diag)
            .expect("should parse valid csproj");

        assert_eq!(config.target_framework.as_deref(), Some("net8.0"));
        assert_eq!(config.root_namespace.as_deref(), Some("MyApp.Core"));
        assert_eq!(config.assembly_name.as_deref(), Some("MyApp.Core"));
        assert_eq!(config.project_references.len(), 1);
        assert_eq!(
            config.project_references[0].path,
            PathBuf::from("../MyApp.Data/MyApp.Data.csproj")
        );
        assert_eq!(config.package_references.len(), 2);
        assert_eq!(config.package_references[0].name, "Newtonsoft.Json");
        assert_eq!(
            config.package_references[0].version.as_deref(),
            Some("13.0.3")
        );
        assert_eq!(config.package_references[1].name, "Serilog");
    }

    #[test]
    fn test_parse_csproj_minimal() {
        let diag = make_diagnostics();
        let content = r#"<Project Sdk="Microsoft.NET.Sdk">
  <PropertyGroup>
    <TargetFramework>net6.0</TargetFramework>
  </PropertyGroup>
</Project>"#;

        let config = parse_csproj(content, Path::new("MyApp.csproj"), &diag)
            .expect("should parse minimal csproj");

        assert_eq!(config.target_framework.as_deref(), Some("net6.0"));
        assert!(config.root_namespace.is_none());
        assert!(config.assembly_name.is_none());
        assert!(config.project_references.is_empty());
        assert!(config.package_references.is_empty());
    }

    #[test]
    fn test_parse_csproj_malformed() {
        let diag = make_diagnostics();
        let content = "this is not xml at all < > & broken";

        let result = parse_csproj(content, Path::new("bad.csproj"), &diag);
        assert!(result.is_none());

        let report = diag.drain();
        assert!(report
            .warnings
            .iter()
            .any(|w| w.code == WarningCode::W034CsprojParseError));
    }

    #[test]
    fn test_parse_csproj_project_references() {
        let diag = make_diagnostics();
        let content = r#"<Project Sdk="Microsoft.NET.Sdk">
  <PropertyGroup>
    <TargetFramework>net8.0</TargetFramework>
  </PropertyGroup>
  <ItemGroup>
    <ProjectReference Include="..\Lib1\Lib1.csproj" />
    <ProjectReference Include="..\Lib2\Lib2.csproj" />
    <ProjectReference Include="..\..\Shared\Shared.csproj" />
  </ItemGroup>
</Project>"#;

        let config = parse_csproj(content, Path::new("src/App/App.csproj"), &diag)
            .expect("should parse csproj with multiple project references");

        assert_eq!(config.project_references.len(), 3);
        // Paths should have backslashes normalized to forward slashes
        assert_eq!(
            config.project_references[0].path,
            PathBuf::from("../Lib1/Lib1.csproj")
        );
        assert_eq!(
            config.project_references[1].path,
            PathBuf::from("../Lib2/Lib2.csproj")
        );
        assert_eq!(
            config.project_references[2].path,
            PathBuf::from("../../Shared/Shared.csproj")
        );
    }

    #[test]
    fn test_parse_csproj_package_references() {
        let diag = make_diagnostics();
        let content = r#"<Project Sdk="Microsoft.NET.Sdk">
  <PropertyGroup>
    <TargetFramework>net8.0</TargetFramework>
  </PropertyGroup>
  <ItemGroup>
    <PackageReference Include="MediatR" Version="12.1.1" />
    <PackageReference Include="AutoMapper" />
  </ItemGroup>
</Project>"#;

        let config = parse_csproj(content, Path::new("App.csproj"), &diag)
            .expect("should parse package references");

        assert_eq!(config.package_references.len(), 2);
        assert_eq!(config.package_references[0].name, "MediatR");
        assert_eq!(
            config.package_references[0].version.as_deref(),
            Some("12.1.1")
        );
        assert_eq!(config.package_references[1].name, "AutoMapper");
        assert!(config.package_references[1].version.is_none());
    }

    #[test]
    fn test_parse_csproj_target_frameworks_semicolon() {
        let diag = make_diagnostics();
        let content = r#"<Project Sdk="Microsoft.NET.Sdk">
  <PropertyGroup>
    <TargetFrameworks>net6.0;net7.0;net8.0</TargetFrameworks>
  </PropertyGroup>
</Project>"#;

        let config = parse_csproj(content, Path::new("Multi.csproj"), &diag)
            .expect("should parse multi-target frameworks");

        // Should take first entry
        assert_eq!(config.target_framework.as_deref(), Some("net6.0"));
    }

    // --- parse_sln tests ---

    #[test]
    fn test_parse_sln_basic() {
        let diag = make_diagnostics();
        let content = r#"
Microsoft Visual Studio Solution File, Format Version 12.00
# Visual Studio Version 17
Project("{FAE04EC0-301F-11D3-BF4B-00C04F79EFBC}") = "MyApp.Core", "src\MyApp.Core\MyApp.Core.csproj", "{GUID1}"
EndProject
Project("{FAE04EC0-301F-11D3-BF4B-00C04F79EFBC}") = "MyApp.Web", "src\MyApp.Web\MyApp.Web.csproj", "{GUID2}"
EndProject
Global
EndGlobal
"#;

        let sln = parse_sln(content, Path::new("MyApp.sln"), &diag)
            .expect("should parse valid .sln");

        assert_eq!(sln.projects.len(), 2);
        assert_eq!(sln.projects[0].name, "MyApp.Core");
        assert_eq!(
            sln.projects[0].relative_path,
            PathBuf::from("src/MyApp.Core/MyApp.Core.csproj")
        );
        assert_eq!(sln.projects[1].name, "MyApp.Web");
        assert_eq!(
            sln.projects[1].relative_path,
            PathBuf::from("src/MyApp.Web/MyApp.Web.csproj")
        );
    }

    #[test]
    fn test_parse_sln_filters_solution_folders() {
        let diag = make_diagnostics();
        let content = r#"
Microsoft Visual Studio Solution File, Format Version 12.00
Project("{FAE04EC0-301F-11D3-BF4B-00C04F79EFBC}") = "MyApp", "src\MyApp\MyApp.csproj", "{GUID1}"
EndProject
Project("{2150E333-8FDC-42A3-9474-1A3956D46DE8}") = "Solution Items", "Solution Items", "{GUID2}"
EndProject
"#;

        let sln = parse_sln(content, Path::new("Test.sln"), &diag)
            .expect("should parse .sln and filter folders");

        assert_eq!(sln.projects.len(), 1);
        assert_eq!(sln.projects[0].name, "MyApp");
    }

    #[test]
    fn test_parse_sln_empty() {
        let diag = make_diagnostics();
        let content = r#"
Microsoft Visual Studio Solution File, Format Version 12.00
Global
EndGlobal
"#;

        let sln = parse_sln(content, Path::new("Empty.sln"), &diag)
            .expect("should parse empty .sln");

        assert!(sln.projects.is_empty());
    }
}
