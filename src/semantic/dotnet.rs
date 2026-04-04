//! .NET semantic boundary extractor.
//!
//! Detects EF Core DbSet properties (entity registration), DI service registrations,
//! and @inject directives as semantic boundaries.

use crate::model::semantic::{Boundary, BoundaryKind, BoundaryRole};
use crate::model::types::CanonicalPath;
use crate::semantic::BoundaryExtractor;

/// Maximum boundaries per file before overflow guard triggers (EC-12).
const MAX_BOUNDARIES_PER_FILE: usize = 500;

/// .NET boundary extractor for EF Core and DI patterns.
pub struct DotnetBoundaryExtractor;

impl BoundaryExtractor for DotnetBoundaryExtractor {
    fn extensions(&self) -> &[&str] {
        &["cs"]
    }

    fn extract(
        &self,
        tree: &tree_sitter::Tree,
        source: &[u8],
        path: &CanonicalPath,
    ) -> Vec<Boundary> {
        let mut boundaries = Vec::new();
        walk_node(&tree.root_node(), source, path, &mut boundaries);
        boundaries
    }
}

fn walk_node(
    node: &tree_sitter::Node,
    source: &[u8],
    path: &CanonicalPath,
    boundaries: &mut Vec<Boundary>,
) {
    if boundaries.len() >= MAX_BOUNDARIES_PER_FILE {
        return;
    }

    match node.kind() {
        "property_declaration" => {
            try_extract_dbset_property(node, source, path, boundaries);
        }
        "invocation_expression" => {
            try_extract_di_registration(node, source, path, boundaries);
        }
        _ => {}
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        walk_node(&child, source, path, boundaries);
        if boundaries.len() >= MAX_BOUNDARIES_PER_FILE {
            return;
        }
    }
}

/// Extract EF Core `DbSet<T>` property declarations as entity boundaries.
///
/// Pattern: `public DbSet<EntityType> PropertyName { get; set; }`
/// Produces: Boundary { name: "Entity:EntityType", kind: EventChannel, role: Producer }
fn try_extract_dbset_property(
    node: &tree_sitter::Node,
    source: &[u8],
    path: &CanonicalPath,
    boundaries: &mut Vec<Boundary>,
) {
    // property_declaration has a type child that may be generic_name
    let type_node = match node.child_by_field_name("type") {
        Some(t) => t,
        None => return,
    };

    // Check if type is DbSet<T> via generic_name
    if type_node.kind() != "generic_name" {
        return;
    }

    // generic_name contains: identifier("DbSet") + type_argument_list("<T>")
    let mut is_dbset = false;
    let mut entity_type: Option<String> = None;

    let mut cursor = type_node.walk();
    for child in type_node.children(&mut cursor) {
        match child.kind() {
            "identifier" => {
                if child.utf8_text(source).unwrap_or("") == "DbSet" {
                    is_dbset = true;
                }
            }
            "type_argument_list" => {
                // Extract the type argument (first identifier inside < >)
                let mut inner_cursor = child.walk();
                for inner in child.children(&mut inner_cursor) {
                    if inner.is_named() && inner.kind() != "type_argument_list" {
                        let text = inner.utf8_text(source).unwrap_or("");
                        if !text.is_empty() && text != "<" && text != ">" && text != "," {
                            entity_type = Some(text.to_string());
                            break;
                        }
                    }
                }
            }
            _ => {}
        }
    }

    if is_dbset {
        if let Some(entity) = entity_type {
            boundaries.push(Boundary {
                kind: BoundaryKind::EventChannel,
                name: format!("Entity:{}", entity),
                role: BoundaryRole::Producer,
                file: path.clone(),
                line: node.start_position().row as u32 + 1,
                framework: Some("efcore".to_string()),
                method: None,
            });
        }
    }
}

/// Extract DI service registration calls as boundaries.
///
/// Patterns:
/// - `builder.Services.AddScoped<IService, Implementation>()`
/// - `builder.Services.AddTransient<IService, Implementation>()`
/// - `builder.Services.AddSingleton<IService, Implementation>()`
///
/// Produces: Boundary { name: "DI:IService", kind: EventChannel, role: Producer }
fn try_extract_di_registration(
    node: &tree_sitter::Node,
    source: &[u8],
    path: &CanonicalPath,
    boundaries: &mut Vec<Boundary>,
) {
    let callee = match node.child(0) {
        Some(c) if c.kind() == "member_access_expression" => c,
        _ => return,
    };

    // Extract method name (may be generic_name for AddScoped<T>)
    let name_node = match callee.child_by_field_name("name") {
        Some(n) => n,
        None => return,
    };

    let (method_name, service_type) = match name_node.kind() {
        "generic_name" => {
            let mut method = None;
            let mut svc_type = None;
            let mut cursor = name_node.walk();
            for child in name_node.children(&mut cursor) {
                match child.kind() {
                    "identifier" => method = child.utf8_text(source).ok(),
                    "type_argument_list" => {
                        // First type argument is the service interface
                        let mut inner_cursor = child.walk();
                        for inner in child.children(&mut inner_cursor) {
                            if inner.is_named()
                                && inner.kind() != "type_argument_list"
                                && inner.kind() != ","
                            {
                                let text = inner.utf8_text(source).unwrap_or("");
                                if !text.is_empty() {
                                    svc_type = Some(text.to_string());
                                    break;
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
            (method, svc_type)
        }
        "identifier" => {
            let method = name_node.utf8_text(source).ok();
            (method, None)
        }
        _ => return,
    };

    let method_name = match method_name {
        Some(n) => n,
        None => return,
    };

    // Only match DI registration methods
    match method_name {
        "AddScoped" | "AddTransient" | "AddSingleton" | "AddHostedService" => {}
        _ => return,
    }

    // Verify "Services" is in the call chain
    let callee_text = callee.utf8_text(source).unwrap_or("");
    if !callee_text.contains("Services") && !callee_text.contains("services") {
        return;
    }

    if let Some(svc) = service_type {
        boundaries.push(Boundary {
            kind: BoundaryKind::EventChannel,
            name: format!("DI:{}", svc),
            role: BoundaryRole::Producer,
            file: path.clone(),
            line: node.start_position().row as u32 + 1,
            framework: Some("dotnet_di".to_string()),
            method: None,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_and_extract(source: &str) -> Vec<Boundary> {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter::Language::from(tree_sitter_c_sharp::LANGUAGE))
            .unwrap();
        let tree = parser.parse(source.as_bytes(), None).unwrap();
        let path = CanonicalPath::new("test.cs");
        let extractor = DotnetBoundaryExtractor;
        extractor.extract(&tree, source.as_bytes(), &path)
    }

    #[test]
    fn test_extracts_ef_dbset_boundaries() {
        let source = r#"
using Microsoft.EntityFrameworkCore;

public class AppDbContext : DbContext
{
    public DbSet<User> Users { get; set; }
    public DbSet<Order> Orders { get; set; }
}
"#;
        let boundaries = parse_and_extract(source);
        assert_eq!(boundaries.len(), 2);
        assert_eq!(boundaries[0].name, "Entity:User");
        assert_eq!(boundaries[0].kind, BoundaryKind::EventChannel);
        assert_eq!(boundaries[0].role, BoundaryRole::Producer);
        assert_eq!(boundaries[0].framework.as_deref(), Some("efcore"));
        assert_eq!(boundaries[1].name, "Entity:Order");
    }

    #[test]
    fn test_extracts_di_registration_boundaries() {
        let source = r#"
var builder = WebApplication.CreateBuilder(args);

builder.Services.AddScoped<IWeatherService, WeatherService>();
builder.Services.AddSingleton<ICache, MemoryCache>();
builder.Services.AddTransient<ILogger, ConsoleLogger>();
"#;
        let boundaries = parse_and_extract(source);
        assert_eq!(boundaries.len(), 3);
        assert_eq!(boundaries[0].name, "DI:IWeatherService");
        assert_eq!(boundaries[0].role, BoundaryRole::Producer);
        assert_eq!(boundaries[0].framework.as_deref(), Some("dotnet_di"));
        assert_eq!(boundaries[1].name, "DI:ICache");
        assert_eq!(boundaries[2].name, "DI:ILogger");
    }

    #[test]
    fn test_empty_class_no_boundaries() {
        let source = r#"
namespace MyApp.Models;

public class User
{
    public int Id { get; set; }
    public string Name { get; set; }
}
"#;
        let boundaries = parse_and_extract(source);
        assert!(boundaries.is_empty());
    }
}
