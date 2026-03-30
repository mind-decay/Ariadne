mod helpers;

use std::collections::HashSet;

// ---------------------------------------------------------------------------
// Phase 5: Config-aware import resolution integration tests
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// tsconfig_project: TypeScript path alias resolution via tsconfig.json
// ---------------------------------------------------------------------------

#[test]
fn tsconfig_project_resolves_path_aliases() {
    let output = helpers::build_fixture("tsconfig_project");

    // Should find all 4 TypeScript files + tsconfig.json
    assert!(
        output.file_count >= 4,
        "expected at least 4 files, got {}",
        output.file_count
    );

    // Should have edges from alias-based imports
    assert!(
        output.edge_count > 0,
        "expected edges from alias imports, got 0"
    );

    // Read graph.json to verify specific edges
    let json_str = std::fs::read_to_string(&output.graph_path).expect("read graph.json");
    let graph: serde_json::Value = serde_json::from_str(&json_str).expect("parse JSON");
    let edges = graph["edges"].as_array().expect("edges should be an array");
    let nodes = graph["nodes"].as_object().expect("nodes should be an object");
    let node_keys: HashSet<&str> = nodes.keys().map(|k| k.as_str()).collect();

    // Verify all source files are present as nodes
    let expected_files = ["app.ts", "index.ts", "Button.ts", "utils.ts"];
    for file in &expected_files {
        assert!(
            node_keys.iter().any(|k| k.contains(file)),
            "{} should be a node; found: {:?}",
            file,
            node_keys
        );
    }

    // Verify: app.ts -> Button.ts (via @/components/Button alias)
    let has_button_edge = edges.iter().any(|e| {
        let arr = e.as_array().unwrap();
        let from = arr[0].as_str().unwrap_or("");
        let to = arr[1].as_str().unwrap_or("");
        from.contains("app.ts") && to.contains("Button.ts")
    });
    assert!(
        has_button_edge,
        "expected edge from app.ts to Button.ts (via @/components/Button alias); edges: {:?}",
        edges
    );

    // Verify: app.ts -> utils.ts (via shared/lib/utils alias)
    let has_utils_edge = edges.iter().any(|e| {
        let arr = e.as_array().unwrap();
        let from = arr[0].as_str().unwrap_or("");
        let to = arr[1].as_str().unwrap_or("");
        from.contains("app.ts") && to.contains("utils.ts")
    });
    assert!(
        has_utils_edge,
        "expected edge from app.ts to utils.ts (via shared/lib/utils alias); edges: {:?}",
        edges
    );

    // Verify: index.ts -> app.ts (via ./app extensionless relative import)
    let has_app_edge = edges.iter().any(|e| {
        let arr = e.as_array().unwrap();
        let from = arr[0].as_str().unwrap_or("");
        let to = arr[1].as_str().unwrap_or("");
        from.contains("index.ts") && to.contains("app.ts")
    });
    assert!(
        has_app_edge,
        "expected edge from index.ts to app.ts (via ./app extensionless); edges: {:?}",
        edges
    );
}

// ---------------------------------------------------------------------------
// tsconfig_extends: TypeScript config inheritance via extends
// ---------------------------------------------------------------------------

#[test]
fn tsconfig_extends_resolves_inherited_aliases() {
    let output = helpers::build_fixture("tsconfig_extends");

    // Should find source files
    assert!(
        output.file_count >= 3,
        "expected at least 3 files, got {}",
        output.file_count
    );

    // Read graph.json
    let json_str = std::fs::read_to_string(&output.graph_path).expect("read graph.json");
    let graph: serde_json::Value = serde_json::from_str(&json_str).expect("parse JSON");
    let edges = graph["edges"].as_array().expect("edges should be an array");
    let nodes = graph["nodes"].as_object().expect("nodes should be an object");
    let node_keys: HashSet<&str> = nodes.keys().map(|k| k.as_str()).collect();

    // Verify source files are present
    assert!(
        node_keys.iter().any(|k| k.contains("main.ts")),
        "main.ts should be a node; found: {:?}",
        node_keys
    );
    assert!(
        node_keys.iter().any(|k| k.contains("core.ts")),
        "core.ts should be a node; found: {:?}",
        node_keys
    );
    assert!(
        node_keys.iter().any(|k| k.contains("service.ts")),
        "service.ts should be a node; found: {:?}",
        node_keys
    );

    // Verify: main.ts -> core.ts (via @base/core alias from base config)
    let has_base_edge = edges.iter().any(|e| {
        let arr = e.as_array().unwrap();
        let from = arr[0].as_str().unwrap_or("");
        let to = arr[1].as_str().unwrap_or("");
        from.contains("main.ts") && to.contains("core.ts")
    });
    assert!(
        has_base_edge,
        "expected edge from main.ts to core.ts (via @base/core inherited alias); edges: {:?}",
        edges
    );

    // Verify: main.ts -> service.ts (via @src/service alias from local config)
    let has_local_edge = edges.iter().any(|e| {
        let arr = e.as_array().unwrap();
        let from = arr[0].as_str().unwrap_or("");
        let to = arr[1].as_str().unwrap_or("");
        from.contains("main.ts") && to.contains("service.ts")
    });
    assert!(
        has_local_edge,
        "expected edge from main.ts to service.ts (via @src/service local alias); edges: {:?}",
        edges
    );

    // Should have at least 2 edges (the two alias-based imports)
    assert!(
        output.edge_count >= 2,
        "expected at least 2 edges from alias imports, got {}",
        output.edge_count
    );
}

// ---------------------------------------------------------------------------
// gomod_project: Go module-aware import resolution via go.mod
// ---------------------------------------------------------------------------

#[test]
fn gomod_project_resolves_module_imports() {
    let output = helpers::build_fixture("gomod_project");

    // Should find at least 2 .go files
    assert!(
        output.file_count >= 2,
        "expected at least 2 files, got {}",
        output.file_count
    );

    // Read graph.json
    let json_str = std::fs::read_to_string(&output.graph_path).expect("read graph.json");
    let graph: serde_json::Value = serde_json::from_str(&json_str).expect("parse JSON");
    let edges = graph["edges"].as_array().expect("edges should be an array");
    let nodes = graph["nodes"].as_object().expect("nodes should be an object");
    let node_keys: HashSet<&str> = nodes.keys().map(|k| k.as_str()).collect();

    // Verify both files are present
    assert!(
        node_keys.iter().any(|k| k.contains("main.go")),
        "main.go should be a node; found: {:?}",
        node_keys
    );
    assert!(
        node_keys.iter().any(|k| k.contains("auth.go")),
        "auth.go should be a node; found: {:?}",
        node_keys
    );

    // Verify: main.go -> auth.go (via github.com/example/myproject/internal/auth)
    let has_auth_edge = edges.iter().any(|e| {
        let arr = e.as_array().unwrap();
        let from = arr[0].as_str().unwrap_or("");
        let to = arr[1].as_str().unwrap_or("");
        from.contains("main.go") && to.contains("auth")
    });
    assert!(
        has_auth_edge,
        "expected edge from main.go to auth.go (via module-qualified import); edges: {:?}",
        edges
    );
}

// ---------------------------------------------------------------------------
// python_src_layout: Python src-layout import resolution via pyproject.toml
// ---------------------------------------------------------------------------

#[test]
fn python_src_layout_resolves_package_imports() {
    let output = helpers::build_fixture("python_src_layout");

    // Should find at least 3 Python files
    assert!(
        output.file_count >= 3,
        "expected at least 3 files, got {}",
        output.file_count
    );

    // Read graph.json
    let json_str = std::fs::read_to_string(&output.graph_path).expect("read graph.json");
    let graph: serde_json::Value = serde_json::from_str(&json_str).expect("parse JSON");
    let edges = graph["edges"].as_array().expect("edges should be an array");
    let nodes = graph["nodes"].as_object().expect("nodes should be an object");
    let node_keys: HashSet<&str> = nodes.keys().map(|k| k.as_str()).collect();

    // Verify files are present
    assert!(
        node_keys.iter().any(|k| k.contains("main.py")),
        "main.py should be a node; found: {:?}",
        node_keys
    );
    assert!(
        node_keys.iter().any(|k| k.contains("utils.py")),
        "utils.py should be a node; found: {:?}",
        node_keys
    );

    // Verify: main.py -> utils.py (via mypackage.utils with src-layout)
    let has_utils_edge = edges.iter().any(|e| {
        let arr = e.as_array().unwrap();
        let from = arr[0].as_str().unwrap_or("");
        let to = arr[1].as_str().unwrap_or("");
        from.contains("main.py") && to.contains("utils.py")
    });
    assert!(
        has_utils_edge,
        "expected edge from main.py to utils.py (via mypackage.utils with src-layout); edges: {:?}",
        edges
    );
}

// ---------------------------------------------------------------------------
// Regression: existing fixtures still build correctly
// ---------------------------------------------------------------------------

#[test]
fn regression_typescript_app_still_works() {
    let output = helpers::build_fixture("typescript-app");
    assert!(output.file_count > 0, "typescript-app should have file nodes");
    assert!(output.edge_count > 0, "typescript-app should have edges");
}

#[test]
fn regression_go_service_still_works() {
    let output = helpers::build_fixture("go-service");
    assert!(output.file_count > 0, "go-service should have file nodes");
}

#[test]
fn regression_python_package_still_works() {
    let output = helpers::build_fixture("python-package");
    assert!(output.file_count > 0, "python-package should have file nodes");
}

#[test]
fn regression_workspace_project_still_works() {
    let output = helpers::build_fixture("workspace-project");
    assert!(output.file_count >= 6, "workspace-project should have at least 6 files");
    assert!(output.edge_count >= 1, "workspace-project should have cross-package edges");
}

#[test]
fn regression_edge_cases_still_works() {
    let output = helpers::build_fixture("edge-cases");
    assert!(output.file_count > 0, "edge-cases should have file nodes");
}
