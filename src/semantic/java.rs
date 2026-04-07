//! Java semantic boundary extractor.
//!
//! Detects HTTP routes from Spring Boot, Jakarta EE (JAX-RS), Micronaut, and Quarkus
//! annotations, plus DI boundaries from @Component/@Service/@Repository/@Autowired/@Inject.

use crate::model::semantic::{Boundary, BoundaryKind, BoundaryRole};
use crate::model::types::CanonicalPath;
use crate::semantic::BoundaryExtractor;

/// Maximum boundaries per file before overflow guard triggers (EC-12).
const MAX_BOUNDARIES_PER_FILE: usize = 500;

/// Java boundary extractor for Spring, Jakarta, Micronaut, and Quarkus patterns.
pub struct JavaBoundaryExtractor;

impl BoundaryExtractor for JavaBoundaryExtractor {
    fn extensions(&self) -> &[&str] {
        &["java"]
    }

    fn extract(
        &self,
        tree: &tree_sitter::Tree,
        source: &[u8],
        path: &CanonicalPath,
    ) -> Vec<Boundary> {
        let mut boundaries = Vec::new();
        walk_node(
            tree.root_node(),
            source,
            path,
            &mut boundaries,
            &mut ClassContext::default(),
        );
        if boundaries.len() > MAX_BOUNDARIES_PER_FILE {
            boundaries.truncate(MAX_BOUNDARIES_PER_FILE);
        }
        boundaries
    }
}

/// Tracks class-level annotation state while walking a class_declaration scope.
#[derive(Default, Clone)]
struct ClassContext {
    class_name: Option<String>,
    is_spring_controller: bool,
    is_jakarta_jaxrs: bool,
    is_micronaut_controller: bool,
    #[allow(dead_code)]
    is_quarkus_resource: bool,
    class_route_prefix: Option<String>,
}

fn walk_node(
    node: tree_sitter::Node,
    source: &[u8],
    path: &CanonicalPath,
    boundaries: &mut Vec<Boundary>,
    ctx: &mut ClassContext,
) {
    if boundaries.len() >= MAX_BOUNDARIES_PER_FILE {
        return;
    }

    match node.kind() {
        "class_declaration" => {
            handle_class_declaration(node, source, path, boundaries, ctx);
            return; // children handled inside handle_class_declaration
        }
        "method_declaration" => {
            handle_method_declaration(node, source, path, boundaries, ctx);
        }
        "field_declaration" => {
            handle_field_declaration(node, source, path, boundaries, ctx);
        }
        _ => {}
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        walk_node(child, source, path, boundaries, ctx);
        if boundaries.len() >= MAX_BOUNDARIES_PER_FILE {
            return;
        }
    }
}

/// Process a class_declaration node: extract class-level annotations then recurse into body.
fn handle_class_declaration(
    node: tree_sitter::Node,
    source: &[u8],
    path: &CanonicalPath,
    boundaries: &mut Vec<Boundary>,
    parent_ctx: &mut ClassContext,
) {
    let mut ctx = ClassContext::default();

    // Extract class name
    if let Some(name_node) = node.child_by_field_name("name") {
        ctx.class_name = name_node.utf8_text(source).ok().map(|s| s.to_string());
    }

    // Scan modifiers for annotations
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "modifiers" {
            let mut mod_cursor = child.walk();
            for mod_child in child.children(&mut mod_cursor) {
                if mod_child.kind() == "marker_annotation" || mod_child.kind() == "annotation" {
                    if let Some(ann_name) = annotation_name(mod_child, source) {
                        process_class_annotation(
                            &ann_name,
                            mod_child,
                            source,
                            path,
                            boundaries,
                            &mut ctx,
                            node,
                        );
                    }
                }
            }
        }
    }

    // Inherit parent context (not needed for top-level, but safe)
    let _ = parent_ctx;

    // Recurse into class body with the new context
    if let Some(body) = node.child_by_field_name("body") {
        let mut body_cursor = body.walk();
        for child in body.children(&mut body_cursor) {
            walk_node(child, source, path, boundaries, &mut ctx);
            if boundaries.len() >= MAX_BOUNDARIES_PER_FILE {
                return;
            }
        }
    }
}

/// Process a class-level annotation and update ClassContext accordingly.
fn process_class_annotation(
    ann_name: &str,
    ann_node: tree_sitter::Node,
    source: &[u8],
    path: &CanonicalPath,
    boundaries: &mut Vec<Boundary>,
    ctx: &mut ClassContext,
    class_node: tree_sitter::Node,
) {
    match ann_name {
        "RestController" | "Controller" => {
            ctx.is_spring_controller = true;
            // If it's just @Controller with no other Spring route annotation,
            // also emit DI boundary
            if ann_name == "Controller" {
                if let Some(ref class_name) = ctx.class_name {
                    boundaries.push(Boundary {
                        kind: BoundaryKind::EventChannel,
                        name: format!("DI:{}", class_name),
                        role: BoundaryRole::Producer,
                        file: path.clone(),
                        line: class_node.start_position().row as u32 + 1,
                        framework: Some("spring".to_string()),
                        method: None,
                    });
                }
            }
        }
        "Service" | "Repository" | "Component" | "SpringBootApplication" => {
            if let Some(ref class_name) = ctx.class_name {
                boundaries.push(Boundary {
                    kind: BoundaryKind::EventChannel,
                    name: format!("DI:{}", class_name),
                    role: BoundaryRole::Producer,
                    file: path.clone(),
                    line: class_node.start_position().row as u32 + 1,
                    framework: Some("spring".to_string()),
                    method: None,
                });
            }
        }
        "RequestMapping" => {
            ctx.is_spring_controller = true;
            ctx.class_route_prefix = extract_route_path(ann_node, source);
        }
        "Path" => {
            // Jakarta/Quarkus JAX-RS @Path on class
            ctx.is_jakarta_jaxrs = true;
            ctx.is_quarkus_resource = true;
            ctx.class_route_prefix = extract_route_path(ann_node, source);
        }
        "ApplicationScoped" | "Singleton" | "Stateless" => {
            if let Some(ref class_name) = ctx.class_name {
                boundaries.push(Boundary {
                    kind: BoundaryKind::EventChannel,
                    name: format!("DI:{}", class_name),
                    role: BoundaryRole::Producer,
                    file: path.clone(),
                    line: class_node.start_position().row as u32 + 1,
                    framework: Some("jakarta".to_string()),
                    method: None,
                });
            }
        }
        _ => {
            // Check if this could be Micronaut @Controller
            // (Micronaut uses io.micronaut.http.annotation.Controller)
            // Over-approximate: any @Controller not already handled marks micronaut
            if ann_name == "Controller" {
                ctx.is_micronaut_controller = true;
            }
        }
    }
}

/// Handle method_declaration inside a controller class -- extract HTTP routes.
fn handle_method_declaration(
    node: tree_sitter::Node,
    source: &[u8],
    path: &CanonicalPath,
    boundaries: &mut Vec<Boundary>,
    ctx: &mut ClassContext,
) {
    if !ctx.is_spring_controller && !ctx.is_jakarta_jaxrs && !ctx.is_micronaut_controller {
        return;
    }

    // Scan modifiers for HTTP annotations
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "modifiers" {
            if ctx.is_spring_controller {
                extract_spring_method_routes(child, source, path, boundaries, ctx, node);
            }
            if ctx.is_jakarta_jaxrs {
                extract_jaxrs_method_routes(child, source, path, boundaries, ctx, node);
            }
            if ctx.is_micronaut_controller {
                extract_micronaut_method_routes(child, source, path, boundaries, ctx, node);
            }
        }
    }
}

/// Extract Spring-style method HTTP route annotations.
fn extract_spring_method_routes(
    modifiers_node: tree_sitter::Node,
    source: &[u8],
    path: &CanonicalPath,
    boundaries: &mut Vec<Boundary>,
    ctx: &ClassContext,
    method_node: tree_sitter::Node,
) {
    let mut cursor = modifiers_node.walk();
    for child in modifiers_node.children(&mut cursor) {
        if child.kind() != "marker_annotation" && child.kind() != "annotation" {
            continue;
        }
        let ann_name = match annotation_name(child, source) {
            Some(n) => n,
            None => continue,
        };

        let http_method = match ann_name.as_str() {
            "GetMapping" => Some("GET"),
            "PostMapping" => Some("POST"),
            "PutMapping" => Some("PUT"),
            "DeleteMapping" => Some("DELETE"),
            "PatchMapping" => Some("PATCH"),
            "RequestMapping" => None,
            _ => continue,
        };

        let method_path = extract_route_path(child, source).unwrap_or_default();
        let full_route = combine_route(ctx.class_route_prefix.as_deref(), &method_path);

        boundaries.push(Boundary {
            kind: BoundaryKind::HttpRoute,
            name: full_route,
            role: BoundaryRole::Producer,
            file: path.clone(),
            line: method_node.start_position().row as u32 + 1,
            framework: Some("spring".to_string()),
            method: http_method.map(|m| m.to_string()),
        });
    }
}

/// Extract JAX-RS (Jakarta/Quarkus) method HTTP route annotations.
fn extract_jaxrs_method_routes(
    modifiers_node: tree_sitter::Node,
    source: &[u8],
    path: &CanonicalPath,
    boundaries: &mut Vec<Boundary>,
    ctx: &ClassContext,
    method_node: tree_sitter::Node,
) {
    let mut http_method: Option<&str> = None;
    let mut method_path: Option<String> = None;

    let mut cursor = modifiers_node.walk();
    for child in modifiers_node.children(&mut cursor) {
        if child.kind() != "marker_annotation" && child.kind() != "annotation" {
            continue;
        }
        let ann_name = match annotation_name(child, source) {
            Some(n) => n,
            None => continue,
        };

        match ann_name.as_str() {
            "GET" => http_method = Some("GET"),
            "POST" => http_method = Some("POST"),
            "PUT" => http_method = Some("PUT"),
            "DELETE" => http_method = Some("DELETE"),
            "PATCH" => http_method = Some("PATCH"),
            "Path" => method_path = extract_route_path(child, source),
            _ => {}
        }
    }

    if let Some(method) = http_method {
        let mp = method_path.unwrap_or_default();
        let full_route = combine_route(ctx.class_route_prefix.as_deref(), &mp);

        boundaries.push(Boundary {
            kind: BoundaryKind::HttpRoute,
            name: full_route,
            role: BoundaryRole::Producer,
            file: path.clone(),
            line: method_node.start_position().row as u32 + 1,
            framework: Some("jakarta".to_string()),
            method: Some(method.to_string()),
        });
    }
}

/// Extract Micronaut method HTTP route annotations.
fn extract_micronaut_method_routes(
    modifiers_node: tree_sitter::Node,
    source: &[u8],
    path: &CanonicalPath,
    boundaries: &mut Vec<Boundary>,
    ctx: &ClassContext,
    method_node: tree_sitter::Node,
) {
    let mut cursor = modifiers_node.walk();
    for child in modifiers_node.children(&mut cursor) {
        if child.kind() != "marker_annotation" && child.kind() != "annotation" {
            continue;
        }
        let ann_name = match annotation_name(child, source) {
            Some(n) => n,
            None => continue,
        };

        let http_method = match ann_name.as_str() {
            "Get" => Some("GET"),
            "Post" => Some("POST"),
            "Put" => Some("PUT"),
            "Delete" => Some("DELETE"),
            _ => continue,
        };

        if let Some(method) = http_method {
            let method_path = extract_route_path(child, source).unwrap_or_default();
            let full_route = combine_route(ctx.class_route_prefix.as_deref(), &method_path);

            boundaries.push(Boundary {
                kind: BoundaryKind::HttpRoute,
                name: full_route,
                role: BoundaryRole::Producer,
                file: path.clone(),
                line: method_node.start_position().row as u32 + 1,
                framework: Some("micronaut".to_string()),
                method: Some(method.to_string()),
            });
        }
    }
}

/// Handle field_declaration -- extract @Autowired and @Inject DI consumer boundaries.
fn handle_field_declaration(
    node: tree_sitter::Node,
    source: &[u8],
    path: &CanonicalPath,
    boundaries: &mut Vec<Boundary>,
    _ctx: &mut ClassContext,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "modifiers" {
            let mut mod_cursor = child.walk();
            for mod_child in child.children(&mut mod_cursor) {
                if mod_child.kind() != "marker_annotation" && mod_child.kind() != "annotation" {
                    continue;
                }
                let ann_name = match annotation_name(mod_child, source) {
                    Some(n) => n,
                    None => continue,
                };

                let framework = match ann_name.as_str() {
                    "Autowired" => "spring",
                    "Inject" => "jakarta",
                    _ => continue,
                };

                if let Some(field_type) = extract_field_type(node, source) {
                    boundaries.push(Boundary {
                        kind: BoundaryKind::EventChannel,
                        name: format!("DI:{}", field_type),
                        role: BoundaryRole::Consumer,
                        file: path.clone(),
                        line: node.start_position().row as u32 + 1,
                        framework: Some(framework.to_string()),
                        method: None,
                    });
                }
            }
        }
    }
}

/// Extract the annotation name from a marker_annotation or annotation node.
/// Handles both `@Foo` (marker_annotation) and `@Foo(...)` (annotation).
fn annotation_name(node: tree_sitter::Node, source: &[u8]) -> Option<String> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "identifier" {
            return child.utf8_text(source).ok().map(|s| s.to_string());
        }
        // Handle scoped annotations like @jakarta.ws.rs.Path
        if child.kind() == "scoped_identifier" {
            // Get the last identifier in the chain
            let mut inner_cursor = child.walk();
            let mut last_ident = None;
            for inner in child.children(&mut inner_cursor) {
                if inner.kind() == "identifier" {
                    last_ident = inner.utf8_text(source).ok().map(|s| s.to_string());
                }
            }
            return last_ident;
        }
    }
    None
}

/// Extract route path from an annotation's argument list.
///
/// Handles:
/// - `@GetMapping("/path")` (shorthand string literal)
/// - `@RequestMapping(value = "/path")` or `@RequestMapping(path = "/path")`
/// - `@Path("/path")` (JAX-RS)
///
/// Returns None for marker annotations with no arguments.
fn extract_route_path(annotation_node: tree_sitter::Node, source: &[u8]) -> Option<String> {
    let mut cursor = annotation_node.walk();
    for child in annotation_node.children(&mut cursor) {
        if child.kind() == "annotation_argument_list" {
            return extract_path_from_args(child, source);
        }
    }
    None
}

/// Extract path string from annotation_argument_list.
fn extract_path_from_args(args_node: tree_sitter::Node, source: &[u8]) -> Option<String> {
    let mut cursor = args_node.walk();
    for child in args_node.children(&mut cursor) {
        // Direct string literal: @GetMapping("/path")
        if child.kind() == "string_literal" {
            return child.utf8_text(source).ok().map(|s| strip_quotes(s).to_string());
        }
        // element_value_pair: value = "/path" or path = "/path"
        if child.kind() == "element_value_pair" {
            if let Some(path) = extract_from_element_value_pair(child, source) {
                return Some(path);
            }
        }
    }
    // Deep search: sometimes the string is nested in other nodes
    let mut deep_cursor = args_node.walk();
    loop {
        let n = deep_cursor.node();
        if n.kind() == "string_literal" {
            return n.utf8_text(source).ok().map(|s| strip_quotes(s).to_string());
        }
        if deep_cursor.goto_first_child() {
            continue;
        }
        loop {
            if deep_cursor.goto_next_sibling() {
                break;
            }
            if !deep_cursor.goto_parent() {
                return None;
            }
        }
    }
}

/// Extract path from an element_value_pair like `value = "/path"` or `path = "/path"`.
fn extract_from_element_value_pair(
    node: tree_sitter::Node,
    source: &[u8],
) -> Option<String> {
    let mut key: Option<&str> = None;
    let mut value: Option<String> = None;

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "identifier" {
            key = child.utf8_text(source).ok();
        }
        if child.kind() == "string_literal" {
            value = child
                .utf8_text(source)
                .ok()
                .map(|s| strip_quotes(s).to_string());
        }
    }

    match key {
        Some("value" | "path") => value,
        _ => None,
    }
}

/// Extract the type name from a field_declaration node.
fn extract_field_type(field_decl_node: tree_sitter::Node, source: &[u8]) -> Option<String> {
    // field_declaration: modifiers type_identifier declarator ;
    // The type is typically the child with kind "type_identifier" or "generic_type"
    let mut cursor = field_decl_node.walk();
    for child in field_decl_node.children(&mut cursor) {
        match child.kind() {
            "type_identifier" => {
                return child.utf8_text(source).ok().map(|s| s.to_string());
            }
            "generic_type" => {
                // For generic types like List<Foo>, extract the base type
                let mut inner_cursor = child.walk();
                for inner in child.children(&mut inner_cursor) {
                    if inner.kind() == "type_identifier" {
                        return inner.utf8_text(source).ok().map(|s| s.to_string());
                    }
                }
            }
            _ => {}
        }
    }
    None
}

/// Combine a class-level route prefix with a method-level path.
fn combine_route(prefix: Option<&str>, method_path: &str) -> String {
    match prefix {
        Some(p) if !p.is_empty() => {
            let p = p.trim_end_matches('/');
            if method_path.is_empty() {
                p.to_string()
            } else if method_path.starts_with('/') {
                format!("{}{}", p, method_path)
            } else {
                format!("{}/{}", p, method_path)
            }
        }
        _ => method_path.to_string(),
    }
}

/// Strip surrounding quotes from a string literal.
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

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_and_extract(source: &str) -> Vec<Boundary> {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter::Language::from(tree_sitter_java::LANGUAGE))
            .unwrap();
        let tree = parser.parse(source.as_bytes(), None).unwrap();
        let path = CanonicalPath::new("Test.java");
        let extractor = JavaBoundaryExtractor;
        extractor.extract(&tree, source.as_bytes(), &path)
    }

    #[test]
    fn test_spring_get_mapping() {
        let source = r#"
@RestController
@RequestMapping("/users")
public class UserController {
    @GetMapping("/{id}")
    public User getUser() { return null; }
}
"#;
        let boundaries = parse_and_extract(source);
        let routes: Vec<_> = boundaries
            .iter()
            .filter(|b| b.kind == BoundaryKind::HttpRoute)
            .collect();
        assert_eq!(routes.len(), 1);
        assert_eq!(routes[0].name, "/users/{id}");
        assert_eq!(routes[0].method.as_deref(), Some("GET"));
        assert_eq!(routes[0].framework.as_deref(), Some("spring"));
    }

    #[test]
    fn test_spring_post_mapping() {
        let source = r#"
@RestController
@RequestMapping("/users")
public class UserController {
    @PostMapping
    public User createUser() { return null; }
}
"#;
        let boundaries = parse_and_extract(source);
        let routes: Vec<_> = boundaries
            .iter()
            .filter(|b| b.kind == BoundaryKind::HttpRoute)
            .collect();
        assert_eq!(routes.len(), 1);
        assert_eq!(routes[0].name, "/users");
        assert_eq!(routes[0].method.as_deref(), Some("POST"));
        assert_eq!(routes[0].framework.as_deref(), Some("spring"));
    }

    #[test]
    fn test_spring_service_di() {
        let source = r#"
@Service
public class UserService {
    public void doWork() {}
}
"#;
        let boundaries = parse_and_extract(source);
        assert_eq!(boundaries.len(), 1);
        assert_eq!(boundaries[0].kind, BoundaryKind::EventChannel);
        assert_eq!(boundaries[0].name, "DI:UserService");
        assert_eq!(boundaries[0].role, BoundaryRole::Producer);
        assert_eq!(boundaries[0].framework.as_deref(), Some("spring"));
    }

    #[test]
    fn test_spring_autowired_field() {
        let source = r#"
@RestController
public class UserController {
    @Autowired
    UserRepository repo;
}
"#;
        let boundaries = parse_and_extract(source);
        let di_consumers: Vec<_> = boundaries
            .iter()
            .filter(|b| b.kind == BoundaryKind::EventChannel && b.role == BoundaryRole::Consumer)
            .collect();
        assert_eq!(di_consumers.len(), 1);
        assert_eq!(di_consumers[0].name, "DI:UserRepository");
        assert_eq!(di_consumers[0].role, BoundaryRole::Consumer);
        assert_eq!(di_consumers[0].framework.as_deref(), Some("spring"));
    }

    #[test]
    fn test_jakarta_jaxrs_route() {
        let source = r#"
@Path("/items")
public class ItemResource {
    @GET
    @Path("/{id}")
    public Item getItem() { return null; }
}
"#;
        let boundaries = parse_and_extract(source);
        let routes: Vec<_> = boundaries
            .iter()
            .filter(|b| b.kind == BoundaryKind::HttpRoute)
            .collect();
        assert_eq!(routes.len(), 1);
        assert_eq!(routes[0].name, "/items/{id}");
        assert_eq!(routes[0].method.as_deref(), Some("GET"));
        assert_eq!(routes[0].framework.as_deref(), Some("jakarta"));
    }

    #[test]
    fn test_jakarta_inject() {
        let source = r#"
@Path("/orders")
public class OrderResource {
    @Inject
    OrderService orderService;
}
"#;
        let boundaries = parse_and_extract(source);
        let di_consumers: Vec<_> = boundaries
            .iter()
            .filter(|b| b.kind == BoundaryKind::EventChannel && b.role == BoundaryRole::Consumer)
            .collect();
        assert_eq!(di_consumers.len(), 1);
        assert_eq!(di_consumers[0].name, "DI:OrderService");
        assert_eq!(di_consumers[0].role, BoundaryRole::Consumer);
        assert_eq!(di_consumers[0].framework.as_deref(), Some("jakarta"));
    }
}
