mod helpers;

use std::path::PathBuf;

use ariadne_graph::detect::framework::detect_dotnet_framework;
use ariadne_graph::model::types::CanonicalPath;
use ariadne_graph::parser;
use ariadne_graph::semantic::dotnet::DotnetBoundaryExtractor;
use ariadne_graph::semantic::BoundaryExtractor;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn fixture_dir(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

fn read_fixture_file(fixture: &str, path: &str) -> String {
    let full_path = fixture_dir(fixture).join(path);
    std::fs::read_to_string(&full_path)
        .unwrap_or_else(|e| panic!("failed to read {}: {}", full_path.display(), e))
}

fn parse_csharp(source: &str) -> tree_sitter::Tree {
    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&tree_sitter::Language::from(tree_sitter_c_sharp::LANGUAGE))
        .unwrap();
    parser.parse(source.as_bytes(), None).unwrap()
}

// ---------------------------------------------------------------------------
// dotnet-webapi: ASP.NET Web API with controllers, DI, MinimalAPI
// ---------------------------------------------------------------------------

#[test]
fn dotnet_webapi_pipeline_runs() {
    let output = helpers::build_fixture("dotnet-webapi");
    // Should find .cs files (Program.cs, WeatherController.cs, WeatherForecast.cs,
    // IWeatherService.cs, WeatherService.cs) plus .csproj and .sln
    assert!(
        output.file_count >= 5,
        "expected at least 5 files, got {}",
        output.file_count
    );
}

#[test]
fn dotnet_webapi_detects_controller() {
    let source = read_fixture_file("dotnet-webapi", "MyApi/Controllers/WeatherController.cs");
    let tree = parse_csharp(&source);
    let hints = detect_dotnet_framework(&tree, source.as_bytes());
    assert!(
        hints.is_aspnet_controller,
        "WeatherController should be detected as ASP.NET controller"
    );
}

#[test]
fn dotnet_webapi_detects_minimal_api() {
    let source = read_fixture_file("dotnet-webapi", "MyApi/Program.cs");
    let tree = parse_csharp(&source);
    let hints = detect_dotnet_framework(&tree, source.as_bytes());
    // W4: Program.cs contains app.MapGet("/health", ...) — MinimalAPI pattern
    assert!(
        hints.is_minimal_api,
        "Program.cs should be detected as MinimalAPI (app.MapGet)"
    );
    assert!(
        hints.is_di_registration,
        "Program.cs should detect DI registration (builder.Services.AddScoped)"
    );
}

#[test]
fn dotnet_webapi_extracts_imports() {
    let source = read_fixture_file("dotnet-webapi", "MyApi/Controllers/WeatherController.cs");
    let parser = parser::csharp_parser();
    let tree = parse_csharp(&source);
    let imports = parser.extract_imports(&tree, source.as_bytes());
    let import_paths: Vec<&str> = imports.iter().map(|i| i.path.as_str()).collect();
    assert!(
        import_paths.contains(&"MyApi.Services"),
        "WeatherController should import MyApi.Services; got: {:?}",
        import_paths
    );
    assert!(
        import_paths.contains(&"MyApi.Models"),
        "WeatherController should import MyApi.Models; got: {:?}",
        import_paths
    );
}

#[test]
fn dotnet_webapi_graph_has_edges() {
    let output = helpers::build_fixture("dotnet-webapi");
    assert!(
        output.edge_count > 0,
        "expected edges from C# imports, got 0"
    );
}

// ---------------------------------------------------------------------------
// dotnet-blazor: Blazor app with .razor files
// ---------------------------------------------------------------------------

#[test]
fn dotnet_blazor_pipeline_runs() {
    let output = helpers::build_fixture("dotnet-blazor");
    // Should find .razor files (Index.razor, Counter.razor, MainLayout.razor)
    // plus WeatherService.cs, .csproj, .sln
    assert!(
        output.file_count >= 4,
        "expected at least 4 files, got {}",
        output.file_count
    );
}

#[test]
fn dotnet_blazor_extracts_razor_imports() {
    // .razor files use raw_parse (D-145), bypassing tree-sitter
    let source = read_fixture_file("dotnet-blazor", "BlazorApp/Pages/Index.razor");
    let parser = parser::csharp_parser();
    let path = CanonicalPath::new("BlazorApp/Pages/Index.razor".to_string());
    let outcome = parser.raw_parse(source.as_bytes(), "razor", &path);
    let imports = match outcome {
        Some(parser::ParseOutcome::Ok(imports, _, _, _)) => imports,
        _ => panic!("expected ParseOutcome::Ok for .razor file"),
    };
    let import_paths: Vec<&str> = imports.iter().map(|i| i.path.as_str()).collect();
    assert!(
        import_paths.contains(&"BlazorApp.Data"),
        "Index.razor should import BlazorApp.Data via @using; got: {:?}",
        import_paths
    );
    assert!(
        import_paths.contains(&"WeatherService"),
        "Index.razor should import WeatherService via @inject; got: {:?}",
        import_paths
    );
}

#[test]
fn dotnet_blazor_extracts_inherits() {
    // .razor files use raw_parse (D-145), bypassing tree-sitter
    let source = read_fixture_file("dotnet-blazor", "BlazorApp/Shared/MainLayout.razor");
    let parser = parser::csharp_parser();
    let path = CanonicalPath::new("BlazorApp/Shared/MainLayout.razor".to_string());
    let outcome = parser.raw_parse(source.as_bytes(), "razor", &path);
    let imports = match outcome {
        Some(parser::ParseOutcome::Ok(imports, _, _, _)) => imports,
        _ => panic!("expected ParseOutcome::Ok for .razor file"),
    };
    let import_paths: Vec<&str> = imports.iter().map(|i| i.path.as_str()).collect();
    assert!(
        import_paths.contains(&"LayoutComponentBase"),
        "MainLayout.razor should import LayoutComponentBase via @inherits; got: {:?}",
        import_paths
    );
}

#[test]
fn dotnet_blazor_counter_no_imports() {
    // Counter.razor has @page but no @using/@inject/@inherits
    let source = read_fixture_file("dotnet-blazor", "BlazorApp/Pages/Counter.razor");
    let parser = parser::csharp_parser();
    let tree = parse_csharp(&source);
    let imports = parser.extract_imports(&tree, source.as_bytes());
    assert!(
        imports.is_empty(),
        "Counter.razor should have no imports (only @page); got: {:?}",
        imports.iter().map(|i| &i.path).collect::<Vec<_>>()
    );
}

// ---------------------------------------------------------------------------
// dotnet-efcore: EF Core with DbContext, models, migrations
// ---------------------------------------------------------------------------

#[test]
fn dotnet_efcore_pipeline_runs() {
    let output = helpers::build_fixture("dotnet-efcore");
    assert!(
        output.file_count >= 4,
        "expected at least 4 files, got {}",
        output.file_count
    );
}

#[test]
fn dotnet_efcore_detects_dbcontext() {
    let source = read_fixture_file("dotnet-efcore", "DataApp/Data/AppDbContext.cs");
    let tree = parse_csharp(&source);
    let hints = detect_dotnet_framework(&tree, source.as_bytes());
    assert!(
        hints.is_ef_dbcontext,
        "AppDbContext should be detected as EF DbContext"
    );
}

#[test]
fn dotnet_efcore_detects_migration() {
    let source = read_fixture_file("dotnet-efcore", "DataApp/Migrations/20240101_Init.cs");
    let tree = parse_csharp(&source);
    let hints = detect_dotnet_framework(&tree, source.as_bytes());
    assert!(
        hints.is_ef_migration,
        "20240101_Init should be detected as EF Migration"
    );
}

#[test]
fn dotnet_efcore_extracts_dbset_boundaries() {
    let source = read_fixture_file("dotnet-efcore", "DataApp/Data/AppDbContext.cs");
    let tree = parse_csharp(&source);
    let path = CanonicalPath::new("DataApp/Data/AppDbContext.cs");
    let extractor = DotnetBoundaryExtractor;
    let boundaries = extractor.extract(&tree, source.as_bytes(), &path);
    let names: Vec<&str> = boundaries.iter().map(|b| b.name.as_str()).collect();
    assert!(
        names.contains(&"Entity:User"),
        "AppDbContext should produce Entity:User boundary; got: {:?}",
        names
    );
    assert!(
        names.contains(&"Entity:Order"),
        "AppDbContext should produce Entity:Order boundary; got: {:?}",
        names
    );
}

#[test]
fn dotnet_efcore_graph_has_edges() {
    let output = helpers::build_fixture("dotnet-efcore");
    assert!(
        output.edge_count > 0,
        "expected edges from C# imports, got 0"
    );
}

// ---------------------------------------------------------------------------
// dotnet-maui: MAUI app with ContentPage/ContentView patterns (W3)
// ---------------------------------------------------------------------------

#[test]
fn dotnet_maui_pipeline_runs() {
    let output = helpers::build_fixture("dotnet-maui");
    assert!(
        output.file_count >= 4,
        "expected at least 4 files, got {}",
        output.file_count
    );
}

#[test]
fn dotnet_maui_detects_content_page() {
    let source = read_fixture_file("dotnet-maui", "MauiApp/Pages/MainPage.cs");
    let tree = parse_csharp(&source);
    let hints = detect_dotnet_framework(&tree, source.as_bytes());
    assert!(
        hints.is_maui_page,
        "MainPage should be detected as MAUI ContentPage"
    );
}

#[test]
fn dotnet_maui_detects_content_view() {
    let source = read_fixture_file("dotnet-maui", "MauiApp/Pages/SettingsPage.cs");
    let tree = parse_csharp(&source);
    let hints = detect_dotnet_framework(&tree, source.as_bytes());
    assert!(
        hints.is_maui_page,
        "SettingsPage should be detected as MAUI ContentView"
    );
}

#[test]
fn dotnet_maui_graph_has_edges() {
    let output = helpers::build_fixture("dotnet-maui");
    assert!(
        output.edge_count > 0,
        "expected edges from C# imports, got 0"
    );
}
