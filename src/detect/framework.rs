//! .NET framework detection from tree-sitter C# AST.
//!
//! Detects common .NET framework patterns (ASP.NET controllers, EF Core DbContext,
//! Blazor components, MAUI pages, middleware, Minimal API, DI registration) by
//! walking the parsed tree-sitter tree.

/// Hints about which .NET framework patterns are present in a source file.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DotnetFrameworkHints {
    pub is_aspnet_controller: bool,
    pub is_aspnet_middleware: bool,
    pub is_minimal_api: bool,
    pub is_ef_dbcontext: bool,
    pub is_ef_migration: bool,
    pub is_blazor_component: bool,
    pub is_maui_page: bool,
    pub is_di_registration: bool,
}

/// Detect .NET framework patterns from a parsed C# tree-sitter tree.
pub fn detect_dotnet_framework(tree: &tree_sitter::Tree, source: &[u8]) -> DotnetFrameworkHints {
    let mut hints = DotnetFrameworkHints::default();
    walk_node(&tree.root_node(), source, &mut hints);
    hints
}

fn walk_node(node: &tree_sitter::Node, source: &[u8], hints: &mut DotnetFrameworkHints) {
    match node.kind() {
        "class_declaration" => detect_class_patterns(node, source, hints),
        "invocation_expression" => detect_invocation_patterns(node, source, hints),
        _ => {}
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        walk_node(&child, source, hints);
    }
}

/// Detect framework patterns from a class declaration.
fn detect_class_patterns(
    node: &tree_sitter::Node,
    source: &[u8],
    hints: &mut DotnetFrameworkHints,
) {
    // tree-sitter-c-sharp AST for class_declaration:
    //   class_declaration
    //     attribute_list (child, if present)
    //       attribute
    //         identifier: "ApiController"
    //     modifier: "public"
    //     class (keyword)
    //     identifier: "ClassName"
    //     base_list (child kind, NOT a field name)
    //       : (punctuation)
    //       identifier: "BaseClass"
    //     declaration_list (body)
    //       { ... method_declaration ... }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "base_list" => {
                let base_text = child.utf8_text(source).unwrap_or("");
                check_base_class(base_text, hints);
            }
            "attribute_list" => {
                check_attribute_list_node(&child, source, hints);
            }
            "declaration_list" => {
                check_class_methods(&child, source, hints);
            }
            _ => {}
        }
    }

    // Also check preceding sibling attribute_list nodes.
    // In tree-sitter-c-sharp, [ApiController] above a class may be a sibling
    // of the class_declaration in the compilation_unit, not a child.
    let mut sibling = node.prev_sibling();
    while let Some(sib) = sibling {
        if sib.kind() == "attribute_list" {
            check_attribute_list_node(&sib, source, hints);
            sibling = sib.prev_sibling();
        } else {
            break;
        }
    }
}

/// Check base class names for known framework types.
fn check_base_class(base_text: &str, hints: &mut DotnetFrameworkHints) {
    for entry in base_text.split(',') {
        let trimmed = entry.trim().trim_start_matches(':').trim();
        let name = trimmed.split('<').next().unwrap_or(trimmed).trim();

        match name {
            "ControllerBase" | "Controller" => hints.is_aspnet_controller = true,
            "DbContext" => hints.is_ef_dbcontext = true,
            "Migration" => hints.is_ef_migration = true,
            "ComponentBase" => hints.is_blazor_component = true,
            "ContentPage" | "ContentView" => hints.is_maui_page = true,
            _ => {}
        }
    }
}

fn check_attribute_list_node(
    node: &tree_sitter::Node,
    source: &[u8],
    hints: &mut DotnetFrameworkHints,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "attribute" {
            let mut attr_cursor = child.walk();
            for attr_child in child.children(&mut attr_cursor) {
                if attr_child.kind() == "identifier" {
                    let attr_name = attr_child.utf8_text(source).unwrap_or("");
                    if attr_name == "ApiController" {
                        hints.is_aspnet_controller = true;
                    }
                    break;
                }
            }
        }
    }
}

/// Check class methods for middleware and migration patterns.
fn check_class_methods(body: &tree_sitter::Node, source: &[u8], hints: &mut DotnetFrameworkHints) {
    let mut has_invoke = false;
    let mut has_httpcontext_param = false;
    let mut has_up = false;
    let mut has_down = false;

    let mut cursor = body.walk();
    for child in body.children(&mut cursor) {
        if child.kind() == "method_declaration" {
            if let Some(name_node) = child.child_by_field_name("name") {
                let name = name_node.utf8_text(source).unwrap_or("");
                match name {
                    "Invoke" | "InvokeAsync" => {
                        has_invoke = true;
                        if let Some(params) = child.child_by_field_name("parameters") {
                            let params_text = params.utf8_text(source).unwrap_or("");
                            if params_text.contains("HttpContext") {
                                has_httpcontext_param = true;
                            }
                        }
                    }
                    "Up" => has_up = true,
                    "Down" => has_down = true,
                    _ => {}
                }
            }
        }
    }

    if has_invoke && has_httpcontext_param {
        hints.is_aspnet_middleware = true;
    }
    // Validate migration has both Up/Down methods (base class already checked)
    if has_up && has_down && hints.is_ef_migration {
        // Already set from base_list
    }
}

/// Detect top-level invocation patterns:
/// - MinimalAPI: app.MapGet/MapPost/MapPut/MapDelete/MapPatch
/// - DI Registration: *.Services.AddScoped/AddTransient/AddSingleton/AddHostedService
fn detect_invocation_patterns(
    node: &tree_sitter::Node,
    source: &[u8],
    hints: &mut DotnetFrameworkHints,
) {
    let callee = match node.child(0) {
        Some(c) => c,
        None => return,
    };

    if callee.kind() != "member_access_expression" {
        return;
    }

    // In tree-sitter-c-sharp, the member name can be:
    //   - identifier (e.g., "MapGet")
    //   - generic_name (e.g., "AddScoped<IFoo, Foo>") containing an identifier child
    // Extract the simple method name from either form.
    let member_name = extract_member_name(&callee, source);
    let member_name = match member_name {
        Some(n) => n,
        None => return,
    };

    // MinimalAPI patterns
    match member_name {
        "MapGet" | "MapPost" | "MapPut" | "MapDelete" | "MapPatch" => {
            hints.is_minimal_api = true;
            return;
        }
        _ => {}
    }

    // DI Registration patterns
    match member_name {
        "AddScoped" | "AddTransient" | "AddSingleton" | "AddHostedService" => {
            let callee_text = callee.utf8_text(source).unwrap_or("");
            if callee_text.contains("Services") || callee_text.contains("services") {
                hints.is_di_registration = true;
            }
        }
        _ => {}
    }
}

/// Extract the simple method name from a member_access_expression's name child.
///
/// The `name` field in tree-sitter-c-sharp's member_access_expression can be either:
/// - An `identifier` node (plain method: `app.MapGet`)
/// - A `generic_name` node (`builder.Services.AddScoped<T>`) containing an `identifier` child
fn extract_member_name<'a>(
    member_access: &tree_sitter::Node,
    source: &'a [u8],
) -> Option<&'a str> {
    let name_node = member_access.child_by_field_name("name")?;
    match name_node.kind() {
        "identifier" => name_node.utf8_text(source).ok(),
        "generic_name" => {
            // generic_name contains: identifier + type_argument_list
            let mut cursor = name_node.walk();
            for child in name_node.children(&mut cursor) {
                if child.kind() == "identifier" {
                    return child.utf8_text(source).ok();
                }
            }
            None
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_and_detect(source: &str) -> DotnetFrameworkHints {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter::Language::from(tree_sitter_c_sharp::LANGUAGE))
            .unwrap();
        let tree = parser.parse(source.as_bytes(), None).unwrap();
        detect_dotnet_framework(&tree, source.as_bytes())
    }

    #[test]
    fn test_attribute_parsing_works() {
        // Verify tree-sitter-c-sharp exposes attribute nodes in the AST
        let source = r#"
[ApiController]
[Route("api/[controller]")]
public class TestController : ControllerBase { }
"#;
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter::Language::from(tree_sitter_c_sharp::LANGUAGE))
            .unwrap();
        let tree = parser.parse(source.as_bytes(), None).unwrap();

        let root = tree.root_node();
        let mut found_attribute = false;
        fn find_attr(node: &tree_sitter::Node, found: &mut bool) {
            if node.kind() == "attribute" {
                *found = true;
            }
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                find_attr(&child, found);
            }
        }
        find_attr(&root, &mut found_attribute);
        assert!(
            found_attribute,
            "tree-sitter-c-sharp should expose attribute nodes in the AST"
        );

        // Also verify that the framework detection picks this up
        let hints = detect_dotnet_framework(&tree, source.as_bytes());
        assert!(hints.is_aspnet_controller);
    }

    #[test]
    fn test_detects_aspnet_controller() {
        let source = r#"
using Microsoft.AspNetCore.Mvc;

[ApiController]
[Route("api/[controller]")]
public class WeatherController : ControllerBase
{
    [HttpGet]
    public IActionResult Get() { return Ok(); }
}
"#;
        let hints = parse_and_detect(source);
        assert!(hints.is_aspnet_controller);
    }

    #[test]
    fn test_detects_aspnet_controller_by_base_class_only() {
        let source = r#"
public class HomeController : Controller
{
    public IActionResult Index() { return View(); }
}
"#;
        let hints = parse_and_detect(source);
        assert!(hints.is_aspnet_controller);
    }

    #[test]
    fn test_detects_ef_dbcontext() {
        let source = r#"
using Microsoft.EntityFrameworkCore;

public class AppDbContext : DbContext
{
    public DbSet<User> Users { get; set; }
    public DbSet<Order> Orders { get; set; }
}
"#;
        let hints = parse_and_detect(source);
        assert!(hints.is_ef_dbcontext);
    }

    #[test]
    fn test_detects_blazor_component() {
        let source = r#"
using Microsoft.AspNetCore.Components;

public class Counter : ComponentBase
{
    private int count = 0;
}
"#;
        let hints = parse_and_detect(source);
        assert!(hints.is_blazor_component);
    }

    #[test]
    fn test_detects_middleware() {
        let source = r#"
using Microsoft.AspNetCore.Http;

public class LoggingMiddleware
{
    public async Task InvokeAsync(HttpContext context)
    {
        await _next(context);
    }
}
"#;
        let hints = parse_and_detect(source);
        assert!(hints.is_aspnet_middleware);
    }

    #[test]
    fn test_detects_minimal_api() {
        let source = r#"
var app = builder.Build();

app.MapGet("/", () => "Hello World!");
app.MapPost("/api/items", (Item item) => Results.Ok(item));
"#;
        let hints = parse_and_detect(source);
        assert!(hints.is_minimal_api);
    }

    #[test]
    fn test_detects_di_registration() {
        let source = r#"
var builder = WebApplication.CreateBuilder(args);

builder.Services.AddScoped<IWeatherService, WeatherService>();
builder.Services.AddSingleton<ICache, MemoryCache>();
"#;
        let hints = parse_and_detect(source);
        assert!(hints.is_di_registration);
    }

    #[test]
    fn test_detects_maui_page() {
        let source = r#"
using Microsoft.Maui.Controls;

public class MainPage : ContentPage
{
    public MainPage()
    {
        Content = new StackLayout();
    }
}
"#;
        let hints = parse_and_detect(source);
        assert!(hints.is_maui_page);
    }

    #[test]
    fn test_detects_ef_migration() {
        let source = r#"
using Microsoft.EntityFrameworkCore.Migrations;

public class InitialCreate : Migration
{
    protected override void Up(MigrationBuilder migrationBuilder)
    {
        migrationBuilder.CreateTable("Users", table => new { });
    }

    protected override void Down(MigrationBuilder migrationBuilder)
    {
        migrationBuilder.DropTable("Users");
    }
}
"#;
        let hints = parse_and_detect(source);
        assert!(hints.is_ef_migration);
    }

    #[test]
    fn test_plain_class_no_hints() {
        let source = r#"
namespace MyApp.Models;

public class User
{
    public int Id { get; set; }
    public string Name { get; set; }
}
"#;
        let hints = parse_and_detect(source);
        assert_eq!(hints, DotnetFrameworkHints::default());
    }
}
