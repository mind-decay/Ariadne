use crate::model::semantic::{Boundary, BoundaryKind, BoundaryRole};
use crate::model::types::CanonicalPath;
use crate::semantic::BoundaryExtractor;

/// Maximum boundaries per file before overflow guard triggers (EC-12).
const MAX_BOUNDARIES_PER_FILE: usize = 500;

/// HTTP route boundary extractor.
///
/// Detects HTTP route definitions (producers) and HTTP client calls (consumers)
/// across multiple frameworks: Express/Koa, FastAPI, Spring, Go net/http, Gin,
/// ASP.NET, fetch, and axios.
pub struct HttpRouteExtractor;

impl BoundaryExtractor for HttpRouteExtractor {
    fn extensions(&self) -> &[&str] {
        &[
            "ts", "tsx", "js", "jsx", "mjs", "cjs", // JS/TS
            "py", "pyi",                             // Python
            "java",                                  // Java
            "go",                                    // Go
            "cs",                                    // C#
        ]
    }

    fn extract(
        &self,
        tree: &tree_sitter::Tree,
        source: &[u8],
        path: &CanonicalPath,
    ) -> Vec<Boundary> {
        let ext = path
            .as_str()
            .rsplit('.')
            .next()
            .unwrap_or("");

        match ext {
            "ts" | "tsx" | "js" | "jsx" | "mjs" | "cjs" => {
                extract_js_routes(tree, source, path)
            }
            "py" | "pyi" => extract_python_routes(tree, source, path),
            "java" => extract_java_routes(tree, source, path),
            "go" => extract_go_routes(tree, source, path),
            "cs" => extract_csharp_routes(tree, source, path),
            _ => Vec::new(),
        }
    }
}

/// HTTP methods recognized for Express/Koa/Gin style `obj.METHOD(path)` patterns.
const HTTP_METHODS: &[&str] = &["get", "post", "put", "delete", "patch"];

/// Strip surrounding quotes from a string literal node's text.
fn strip_quotes(text: &str) -> &str {
    if text.len() >= 2 {
        let bytes = text.as_bytes();
        let first = bytes[0];
        let last = bytes[bytes.len() - 1];
        if (first == b'"' || first == b'\'') && first == last {
            return &text[1..text.len() - 1];
        }
    }
    text
}

// ---------------------------------------------------------------------------
// JS/TS: Express/Koa producers + fetch/axios consumers
// ---------------------------------------------------------------------------

fn extract_js_routes(
    tree: &tree_sitter::Tree,
    source: &[u8],
    path: &CanonicalPath,
) -> Vec<Boundary> {
    let mut boundaries = Vec::new();
    walk_js_node(&tree.root_node(), source, path, &mut boundaries);
    boundaries
}

fn walk_js_node(
    node: &tree_sitter::Node,
    source: &[u8],
    path: &CanonicalPath,
    boundaries: &mut Vec<Boundary>,
) {
    if boundaries.len() >= MAX_BOUNDARIES_PER_FILE {
        return;
    }

    if node.kind() == "call_expression" {
        try_extract_js_call(node, source, path, boundaries);
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        walk_js_node(&child, source, path, boundaries);
        if boundaries.len() >= MAX_BOUNDARIES_PER_FILE {
            return;
        }
    }
}

fn try_extract_js_call(
    node: &tree_sitter::Node,
    source: &[u8],
    path: &CanonicalPath,
    boundaries: &mut Vec<Boundary>,
) {
    // The callee is the first child of call_expression.
    let callee = match node.child(0) {
        Some(c) => c,
        None => return,
    };

    // Arguments node is typically the second child.
    let args = match node.child(1).or_else(|| node.child(2)) {
        Some(a) if a.kind() == "arguments" => a,
        _ => return,
    };

    // --- fetch("/<path>") consumer ---
    if callee.kind() == "identifier" {
        if let Ok(name) = callee.utf8_text(source) {
            if name == "fetch" {
                if let Some(route) = first_string_arg(&args, source) {
                    if route.starts_with('/') {
                        boundaries.push(Boundary {
                            kind: BoundaryKind::HttpRoute,
                            name: route.to_string(),
                            role: BoundaryRole::Consumer,
                            file: path.clone(),
                            line: node.start_position().row as u32 + 1,
                            framework: Some("fetch".to_string()),
                            method: None,
                        });
                    }
                }
                return;
            }
        }
    }

    // --- member_expression: obj.method(...) ---
    if callee.kind() == "member_expression" {
        let property = match callee.child_by_field_name("property") {
            Some(p) => p,
            None => return,
        };
        let method_name = match property.utf8_text(source) {
            Ok(m) => m,
            Err(_) => return,
        };

        let object = match callee.child_by_field_name("object") {
            Some(o) => o,
            None => return,
        };

        // --- axios.get/post/put/delete("/<path>") consumer ---
        if object.kind() == "identifier" {
            if let Ok(obj_name) = object.utf8_text(source) {
                if obj_name == "axios" {
                    let upper = method_name.to_uppercase();
                    if HTTP_METHODS.contains(&method_name.to_lowercase().as_str()) {
                        if let Some(route) = first_string_arg(&args, source) {
                            boundaries.push(Boundary {
                                kind: BoundaryKind::HttpRoute,
                                name: route.to_string(),
                                role: BoundaryRole::Consumer,
                                file: path.clone(),
                                line: node.start_position().row as u32 + 1,
                                framework: Some("axios".to_string()),
                                method: Some(upper),
                            });
                        }
                    }
                    return;
                }
            }
        }

        // --- Express/Koa: app.get/post/put/delete/patch/use("/path", ...) producer ---
        let lower_method = method_name.to_lowercase();
        if HTTP_METHODS.contains(&lower_method.as_str()) {
            if let Some(route) = first_string_arg(&args, source) {
                boundaries.push(Boundary {
                    kind: BoundaryKind::HttpRoute,
                    name: route.to_string(),
                    role: BoundaryRole::Producer,
                    file: path.clone(),
                    line: node.start_position().row as u32 + 1,
                    framework: Some("express".to_string()),
                    method: Some(lower_method.to_uppercase()),
                });
            }
        } else if lower_method == "use" {
            // Middleware: app.use("/api", ...)
            if let Some(route) = first_string_arg(&args, source) {
                boundaries.push(Boundary {
                    kind: BoundaryKind::HttpRoute,
                    name: route.to_string(),
                    role: BoundaryRole::Both,
                    file: path.clone(),
                    line: node.start_position().row as u32 + 1,
                    framework: Some("express".to_string()),
                    method: None,
                });
            }
        }
    }
}

/// Get the first string literal argument from an arguments node.
/// Returns None for template_string / template_literal (EC-3).
fn first_string_arg<'a>(
    args_node: &tree_sitter::Node,
    source: &'a [u8],
) -> Option<&'a str> {
    let mut cursor = args_node.walk();
    for child in args_node.children(&mut cursor) {
        match child.kind() {
            "string" | "string_literal" => {
                return child.utf8_text(source).ok().map(strip_quotes);
            }
            // Skip template strings entirely (EC-3)
            "template_string" | "template_literal" => return None,
            _ => {}
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Python: FastAPI decorators
// ---------------------------------------------------------------------------

fn extract_python_routes(
    tree: &tree_sitter::Tree,
    source: &[u8],
    path: &CanonicalPath,
) -> Vec<Boundary> {
    let mut boundaries = Vec::new();
    walk_python_node(&tree.root_node(), source, path, &mut boundaries);
    boundaries
}

fn walk_python_node(
    node: &tree_sitter::Node,
    source: &[u8],
    path: &CanonicalPath,
    boundaries: &mut Vec<Boundary>,
) {
    if boundaries.len() >= MAX_BOUNDARIES_PER_FILE {
        return;
    }

    // FastAPI: decorated_definition -> decorator -> call with attribute callee
    if node.kind() == "decorator" {
        try_extract_python_decorator(node, source, path, boundaries);
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        walk_python_node(&child, source, path, boundaries);
        if boundaries.len() >= MAX_BOUNDARIES_PER_FILE {
            return;
        }
    }
}

fn try_extract_python_decorator(
    node: &tree_sitter::Node,
    source: &[u8],
    path: &CanonicalPath,
    boundaries: &mut Vec<Boundary>,
) {
    // Decorator structure: "@" followed by a call or attribute
    // We look for a child that is a `call` node (e.g., @app.get("/path"))
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "call" {
            // The function being called is the first child of `call`
            let callee = match child.child(0) {
                Some(c) => c,
                None => continue,
            };

            // Expect an attribute: obj.method
            if callee.kind() == "attribute" {
                let attr_name = match callee.child_by_field_name("attribute") {
                    Some(a) => a,
                    None => continue,
                };
                let method_name = match attr_name.utf8_text(source) {
                    Ok(m) => m,
                    Err(_) => continue,
                };

                let lower = method_name.to_lowercase();
                if !HTTP_METHODS.contains(&lower.as_str()) {
                    continue;
                }

                // Extract the argument list
                let arg_list = match child.child_by_field_name("arguments") {
                    Some(a) => a,
                    None => continue,
                };

                if let Some(route) = first_python_string_arg(&arg_list, source) {
                    boundaries.push(Boundary {
                        kind: BoundaryKind::HttpRoute,
                        name: route.to_string(),
                        role: BoundaryRole::Producer,
                        file: path.clone(),
                        line: node.start_position().row as u32 + 1,
                        framework: Some("fastapi".to_string()),
                        method: Some(lower.to_uppercase()),
                    });
                }
            }
        }
    }
}

/// Get the first string literal from a Python argument_list node.
fn first_python_string_arg<'a>(
    args_node: &tree_sitter::Node,
    source: &'a [u8],
) -> Option<&'a str> {
    let mut cursor = args_node.walk();
    for child in args_node.children(&mut cursor) {
        if child.kind() == "string" {
            // Python strings: "..." or '...' — tree-sitter includes quotes
            return child.utf8_text(source).ok().map(strip_quotes);
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Java: Spring annotations
// ---------------------------------------------------------------------------

fn extract_java_routes(
    tree: &tree_sitter::Tree,
    source: &[u8],
    path: &CanonicalPath,
) -> Vec<Boundary> {
    let mut boundaries = Vec::new();
    walk_java_node(&tree.root_node(), source, path, &mut boundaries);
    boundaries
}

fn walk_java_node(
    node: &tree_sitter::Node,
    source: &[u8],
    path: &CanonicalPath,
    boundaries: &mut Vec<Boundary>,
) {
    if boundaries.len() >= MAX_BOUNDARIES_PER_FILE {
        return;
    }

    if node.kind() == "annotation" || node.kind() == "marker_annotation" {
        try_extract_java_annotation(node, source, path, boundaries);
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        walk_java_node(&child, source, path, boundaries);
        if boundaries.len() >= MAX_BOUNDARIES_PER_FILE {
            return;
        }
    }
}

/// Spring annotation mapping names to HTTP methods.
fn spring_method(annotation_name: &str) -> Option<Option<&'static str>> {
    match annotation_name {
        "GetMapping" => Some(Some("GET")),
        "PostMapping" => Some(Some("POST")),
        "PutMapping" => Some(Some("PUT")),
        "DeleteMapping" => Some(Some("DELETE")),
        "PatchMapping" => Some(Some("PATCH")),
        "RequestMapping" => Some(None), // method not determinable from annotation name alone
        _ => None,
    }
}

fn try_extract_java_annotation(
    node: &tree_sitter::Node,
    source: &[u8],
    path: &CanonicalPath,
    boundaries: &mut Vec<Boundary>,
) {
    // annotation: "@" identifier [annotation_argument_list]
    let mut name_node = None;
    let mut arg_list = None;

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "identifier" => name_node = Some(child),
            "annotation_argument_list" => arg_list = Some(child),
            _ => {}
        }
    }

    let ann_name = match name_node.and_then(|n| n.utf8_text(source).ok()) {
        Some(n) => n,
        None => return,
    };

    let method = match spring_method(ann_name) {
        Some(m) => m,
        None => return,
    };

    // Extract string literal from annotation arguments
    if let Some(args) = arg_list {
        if let Some(route) = first_java_string_arg(&args, source) {
            boundaries.push(Boundary {
                kind: BoundaryKind::HttpRoute,
                name: route.to_string(),
                role: BoundaryRole::Producer,
                file: path.clone(),
                line: node.start_position().row as u32 + 1,
                framework: Some("spring".to_string()),
                method: method.map(|m| m.to_string()),
            });
        }
    }
}

/// Get the first string literal from a Java annotation_argument_list.
fn first_java_string_arg<'a>(
    args_node: &tree_sitter::Node,
    source: &'a [u8],
) -> Option<&'a str> {
    // Walk all descendants looking for a string_literal
    let mut cursor = args_node.walk();
    loop {
        let node = cursor.node();
        if node.kind() == "string_literal" {
            return node.utf8_text(source).ok().map(strip_quotes);
        }
        // Depth-first traversal
        if cursor.goto_first_child() {
            continue;
        }
        loop {
            if cursor.goto_next_sibling() {
                break;
            }
            if !cursor.goto_parent() {
                return None;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Go: net/http HandleFunc + Gin
// ---------------------------------------------------------------------------

fn extract_go_routes(
    tree: &tree_sitter::Tree,
    source: &[u8],
    path: &CanonicalPath,
) -> Vec<Boundary> {
    let mut boundaries = Vec::new();
    walk_go_node(&tree.root_node(), source, path, &mut boundaries);
    boundaries
}

fn walk_go_node(
    node: &tree_sitter::Node,
    source: &[u8],
    path: &CanonicalPath,
    boundaries: &mut Vec<Boundary>,
) {
    if boundaries.len() >= MAX_BOUNDARIES_PER_FILE {
        return;
    }

    if node.kind() == "call_expression" {
        try_extract_go_call(node, source, path, boundaries);
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        walk_go_node(&child, source, path, boundaries);
        if boundaries.len() >= MAX_BOUNDARIES_PER_FILE {
            return;
        }
    }
}

fn try_extract_go_call(
    node: &tree_sitter::Node,
    source: &[u8],
    path: &CanonicalPath,
    boundaries: &mut Vec<Boundary>,
) {
    // Go call_expression: function + argument_list
    let callee = match node.child(0) {
        Some(c) => c,
        None => return,
    };

    let args = match node.child(1) {
        Some(a) if a.kind() == "argument_list" => a,
        _ => return,
    };

    // selector_expression: object.method
    if callee.kind() == "selector_expression" {
        let field = match callee.child_by_field_name("field") {
            Some(f) => f,
            None => return,
        };
        let method_name = match field.utf8_text(source) {
            Ok(m) => m,
            Err(_) => return,
        };

        // --- net/http: HandleFunc ---
        if method_name == "HandleFunc" {
            if let Some(route) = first_go_string_arg(&args, source) {
                boundaries.push(Boundary {
                    kind: BoundaryKind::HttpRoute,
                    name: route.to_string(),
                    role: BoundaryRole::Producer,
                    file: path.clone(),
                    line: node.start_position().row as u32 + 1,
                    framework: Some("go_http".to_string()),
                    method: None, // HandleFunc doesn't specify method
                });
            }
            return;
        }

        // --- Gin: r.GET/POST/PUT/DELETE/PATCH ---
        let upper = method_name.to_uppercase();
        if HTTP_METHODS
            .iter()
            .any(|m| m.to_uppercase() == upper)
        {
            if let Some(route) = first_go_string_arg(&args, source) {
                boundaries.push(Boundary {
                    kind: BoundaryKind::HttpRoute,
                    name: route.to_string(),
                    role: BoundaryRole::Producer,
                    file: path.clone(),
                    line: node.start_position().row as u32 + 1,
                    framework: Some("gin".to_string()),
                    method: Some(upper),
                });
            }
        }
    }
}

/// Get the first interpreted_string_literal from a Go argument_list.
fn first_go_string_arg<'a>(
    args_node: &tree_sitter::Node,
    source: &'a [u8],
) -> Option<&'a str> {
    let mut cursor = args_node.walk();
    for child in args_node.children(&mut cursor) {
        if child.kind() == "interpreted_string_literal" {
            return child.utf8_text(source).ok().map(strip_quotes);
        }
    }
    None
}

// ---------------------------------------------------------------------------
// C#: ASP.NET attributes
// ---------------------------------------------------------------------------

fn extract_csharp_routes(
    tree: &tree_sitter::Tree,
    source: &[u8],
    path: &CanonicalPath,
) -> Vec<Boundary> {
    let mut boundaries = Vec::new();
    walk_csharp_node(&tree.root_node(), source, path, &mut boundaries);
    boundaries
}

fn walk_csharp_node(
    node: &tree_sitter::Node,
    source: &[u8],
    path: &CanonicalPath,
    boundaries: &mut Vec<Boundary>,
) {
    if boundaries.len() >= MAX_BOUNDARIES_PER_FILE {
        return;
    }

    if node.kind() == "attribute" {
        try_extract_csharp_attribute(node, source, path, boundaries);
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        walk_csharp_node(&child, source, path, boundaries);
        if boundaries.len() >= MAX_BOUNDARIES_PER_FILE {
            return;
        }
    }
}

/// ASP.NET attribute names to HTTP methods.
fn aspnet_method(attr_name: &str) -> Option<Option<&'static str>> {
    match attr_name {
        "HttpGet" => Some(Some("GET")),
        "HttpPost" => Some(Some("POST")),
        "HttpPut" => Some(Some("PUT")),
        "HttpDelete" => Some(Some("DELETE")),
        "HttpPatch" => Some(Some("PATCH")),
        "Route" => Some(None),
        _ => None,
    }
}

fn try_extract_csharp_attribute(
    node: &tree_sitter::Node,
    source: &[u8],
    path: &CanonicalPath,
    boundaries: &mut Vec<Boundary>,
) {
    // attribute: name [attribute_argument_list]
    let mut name_node = None;
    let mut arg_list = None;

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "identifier" => name_node = Some(child),
            "attribute_argument_list" => arg_list = Some(child),
            _ => {}
        }
    }

    let attr_name = match name_node.and_then(|n| n.utf8_text(source).ok()) {
        Some(n) => n,
        None => return,
    };

    let method = match aspnet_method(attr_name) {
        Some(m) => m,
        None => return,
    };

    if let Some(args) = arg_list {
        if let Some(route) = first_csharp_string_arg(&args, source) {
            boundaries.push(Boundary {
                kind: BoundaryKind::HttpRoute,
                name: route.to_string(),
                role: BoundaryRole::Producer,
                file: path.clone(),
                line: node.start_position().row as u32 + 1,
                framework: Some("aspnet".to_string()),
                method: method.map(|m| m.to_string()),
            });
        }
    }
}

/// Get the first string_literal from a C# attribute_argument_list.
fn first_csharp_string_arg<'a>(
    args_node: &tree_sitter::Node,
    source: &'a [u8],
) -> Option<&'a str> {
    let mut cursor = args_node.walk();
    loop {
        let node = cursor.node();
        if node.kind() == "string_literal" {
            return node.utf8_text(source).ok().map(strip_quotes);
        }
        if cursor.goto_first_child() {
            continue;
        }
        loop {
            if cursor.goto_next_sibling() {
                break;
            }
            if !cursor.goto_parent() {
                return None;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_and_extract(
        source: &str,
        ext: &str,
        lang: tree_sitter::Language,
    ) -> Vec<Boundary> {
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&lang).unwrap();
        let tree = parser.parse(source.as_bytes(), None).unwrap();
        let path = CanonicalPath::new(format!("test.{ext}"));
        let extractor = HttpRouteExtractor;
        extractor.extract(&tree, source.as_bytes(), &path)
    }

    #[test]
    fn express_get_route() {
        let source = r#"app.get("/api/users", handler);"#;
        let lang = tree_sitter::Language::from(tree_sitter_typescript::LANGUAGE_TYPESCRIPT);
        let boundaries = parse_and_extract(source, "ts", lang);
        assert_eq!(boundaries.len(), 1);
        assert_eq!(boundaries[0].name, "/api/users");
        assert_eq!(boundaries[0].role, BoundaryRole::Producer);
        assert_eq!(boundaries[0].framework.as_deref(), Some("express"));
        assert_eq!(boundaries[0].method.as_deref(), Some("GET"));
    }

    #[test]
    fn express_middleware_use() {
        let source = r#"app.use("/api", router);"#;
        let lang = tree_sitter::Language::from(tree_sitter_typescript::LANGUAGE_TYPESCRIPT);
        let boundaries = parse_and_extract(source, "ts", lang);
        assert_eq!(boundaries.len(), 1);
        assert_eq!(boundaries[0].name, "/api");
        assert_eq!(boundaries[0].role, BoundaryRole::Both);
        assert_eq!(boundaries[0].method, None);
    }

    #[test]
    fn fetch_consumer() {
        let source = r#"fetch("/api/users");"#;
        let lang = tree_sitter::Language::from(tree_sitter_typescript::LANGUAGE_TYPESCRIPT);
        let boundaries = parse_and_extract(source, "ts", lang);
        assert_eq!(boundaries.len(), 1);
        assert_eq!(boundaries[0].name, "/api/users");
        assert_eq!(boundaries[0].role, BoundaryRole::Consumer);
        assert_eq!(boundaries[0].framework.as_deref(), Some("fetch"));
    }

    #[test]
    fn fetch_non_path_ignored() {
        let source = r#"fetch("https://example.com/api");"#;
        let lang = tree_sitter::Language::from(tree_sitter_typescript::LANGUAGE_TYPESCRIPT);
        let boundaries = parse_and_extract(source, "ts", lang);
        assert!(boundaries.is_empty());
    }

    #[test]
    fn axios_consumer() {
        let source = r#"axios.get("/api/users");"#;
        let lang = tree_sitter::Language::from(tree_sitter_typescript::LANGUAGE_TYPESCRIPT);
        let boundaries = parse_and_extract(source, "ts", lang);
        assert_eq!(boundaries.len(), 1);
        assert_eq!(boundaries[0].name, "/api/users");
        assert_eq!(boundaries[0].role, BoundaryRole::Consumer);
        assert_eq!(boundaries[0].framework.as_deref(), Some("axios"));
        assert_eq!(boundaries[0].method.as_deref(), Some("GET"));
    }

    #[test]
    fn template_literal_skipped() {
        let source = "app.get(`/api/users/${id}`, handler);";
        let lang = tree_sitter::Language::from(tree_sitter_typescript::LANGUAGE_TYPESCRIPT);
        let boundaries = parse_and_extract(source, "ts", lang);
        assert!(boundaries.is_empty());
    }

    #[test]
    fn fastapi_decorator() {
        let source = r#"
@app.get("/users")
def list_users():
    pass
"#;
        let lang = tree_sitter::Language::from(tree_sitter_python::LANGUAGE);
        let boundaries = parse_and_extract(source, "py", lang);
        assert_eq!(boundaries.len(), 1);
        assert_eq!(boundaries[0].name, "/users");
        assert_eq!(boundaries[0].framework.as_deref(), Some("fastapi"));
        assert_eq!(boundaries[0].method.as_deref(), Some("GET"));
    }

    #[test]
    fn spring_get_mapping() {
        let source = r#"
@GetMapping("/api/users")
public List<User> getUsers() { }
"#;
        let lang = tree_sitter::Language::from(tree_sitter_java::LANGUAGE);
        let boundaries = parse_and_extract(source, "java", lang);
        assert_eq!(boundaries.len(), 1);
        assert_eq!(boundaries[0].name, "/api/users");
        assert_eq!(boundaries[0].framework.as_deref(), Some("spring"));
        assert_eq!(boundaries[0].method.as_deref(), Some("GET"));
    }

    #[test]
    fn go_handle_func() {
        let source = r#"http.HandleFunc("/api/users", handler)"#;
        let lang = tree_sitter::Language::from(tree_sitter_go::LANGUAGE);
        let boundaries = parse_and_extract(source, "go", lang);
        assert_eq!(boundaries.len(), 1);
        assert_eq!(boundaries[0].name, "/api/users");
        assert_eq!(boundaries[0].framework.as_deref(), Some("go_http"));
        assert_eq!(boundaries[0].method, None);
    }

    #[test]
    fn gin_routes() {
        let source = r#"
r.GET("/users", listUsers)
r.POST("/users", createUser)
"#;
        let lang = tree_sitter::Language::from(tree_sitter_go::LANGUAGE);
        let boundaries = parse_and_extract(source, "go", lang);
        assert_eq!(boundaries.len(), 2);
        assert_eq!(boundaries[0].framework.as_deref(), Some("gin"));
        assert_eq!(boundaries[0].method.as_deref(), Some("GET"));
        assert_eq!(boundaries[1].method.as_deref(), Some("POST"));
    }

    #[test]
    fn aspnet_attribute() {
        let source = r#"
[HttpGet("/api/users")]
public IActionResult GetUsers() { }
"#;
        let lang = tree_sitter::Language::from(tree_sitter_c_sharp::LANGUAGE);
        let boundaries = parse_and_extract(source, "cs", lang);
        assert_eq!(boundaries.len(), 1);
        assert_eq!(boundaries[0].name, "/api/users");
        assert_eq!(boundaries[0].framework.as_deref(), Some("aspnet"));
        assert_eq!(boundaries[0].method.as_deref(), Some("GET"));
    }

    #[test]
    fn aspnet_route_attribute() {
        let source = r#"
[Route("/api/products")]
public class ProductsController { }
"#;
        let lang = tree_sitter::Language::from(tree_sitter_c_sharp::LANGUAGE);
        let boundaries = parse_and_extract(source, "cs", lang);
        assert_eq!(boundaries.len(), 1);
        assert_eq!(boundaries[0].name, "/api/products");
        assert_eq!(boundaries[0].framework.as_deref(), Some("aspnet"));
        assert_eq!(boundaries[0].method, None);
    }

    #[test]
    fn empty_for_unknown_extension() {
        let extractor = HttpRouteExtractor;
        let path = CanonicalPath::new("test.rb".to_string());
        // Create a minimal tree (won't match anything)
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter::Language::from(
                tree_sitter_typescript::LANGUAGE_TYPESCRIPT,
            ))
            .unwrap();
        let tree = parser.parse(b"x", None).unwrap();
        let result = extractor.extract(&tree, b"x", &path);
        assert!(result.is_empty());
    }

    #[test]
    fn parameterized_route_preserved() {
        let source = r#"app.get("/api/users/:id", handler);"#;
        let lang = tree_sitter::Language::from(tree_sitter_typescript::LANGUAGE_TYPESCRIPT);
        let boundaries = parse_and_extract(source, "ts", lang);
        assert_eq!(boundaries.len(), 1);
        assert_eq!(boundaries[0].name, "/api/users/:id");
    }

    #[test]
    fn multiple_routes_in_one_file() {
        let source = r#"
app.get("/api/users", listUsers);
app.post("/api/users", createUser);
app.delete("/api/users/:id", deleteUser);
"#;
        let lang = tree_sitter::Language::from(tree_sitter_typescript::LANGUAGE_TYPESCRIPT);
        let boundaries = parse_and_extract(source, "ts", lang);
        assert_eq!(boundaries.len(), 3);
        assert_eq!(boundaries[0].method.as_deref(), Some("GET"));
        assert_eq!(boundaries[1].method.as_deref(), Some("POST"));
        assert_eq!(boundaries[2].method.as_deref(), Some("DELETE"));
    }
}
