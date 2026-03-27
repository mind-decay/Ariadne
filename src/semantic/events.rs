use crate::model::semantic::{Boundary, BoundaryKind, BoundaryRole};
use crate::model::types::CanonicalPath;
use crate::semantic::BoundaryExtractor;

/// Maximum boundaries per file before overflow guard triggers (EC-12).
const MAX_BOUNDARIES_PER_FILE: usize = 500;

/// DOM events to skip — these are browser events, not semantic boundaries.
/// Sorted for binary search.
const DOM_EVENTS: &[&str] = &[
    "DOMContentLoaded",
    "animationend",
    "animationstart",
    "beforeunload",
    "blur",
    "change",
    "click",
    "contextmenu",
    "dblclick",
    "dragend",
    "dragover",
    "dragstart",
    "drop",
    "error",
    "focus",
    "input",
    "keydown",
    "keypress",
    "keyup",
    "load",
    "mousedown",
    "mouseout",
    "mouseover",
    "mouseup",
    "pointerdown",
    "pointerup",
    "readystatechange",
    "resize",
    "scroll",
    "submit",
    "touchend",
    "touchmove",
    "touchstart",
    "transitionend",
    "unload",
    "visibilitychange",
    "wheel",
];

/// Returns true if the event name is a DOM event that should be skipped.
fn is_dom_event(name: &str) -> bool {
    DOM_EVENTS.binary_search(&name).is_ok()
}

/// Event emitter/listener boundary extractor.
///
/// Detects event patterns in TypeScript/JavaScript and Python:
/// - Producer: emit, publish
/// - Consumer: on, addEventListener, subscribe
pub struct EventExtractor;

impl BoundaryExtractor for EventExtractor {
    fn extensions(&self) -> &[&str] {
        &["ts", "tsx", "js", "jsx", "mjs", "cjs", "py", "pyi"]
    }

    fn extract(
        &self,
        tree: &tree_sitter::Tree,
        source: &[u8],
        path: &CanonicalPath,
    ) -> Vec<Boundary> {
        let ext = path.as_str().rsplit('.').next().unwrap_or("");

        match ext {
            "ts" | "tsx" | "js" | "jsx" | "mjs" | "cjs" => {
                extract_js_events(tree, source, path)
            }
            "py" | "pyi" => extract_python_events(tree, source, path),
            _ => Vec::new(),
        }
    }
}

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

/// Event function names and their roles for JS/TS.
fn js_event_role(name: &str) -> Option<BoundaryRole> {
    match name {
        "emit" => Some(BoundaryRole::Producer),
        "publish" => Some(BoundaryRole::Producer),
        "on" => Some(BoundaryRole::Consumer),
        "addEventListener" => Some(BoundaryRole::Consumer),
        "subscribe" => Some(BoundaryRole::Consumer),
        _ => None,
    }
}

/// Framework label based on the function name.
fn js_event_framework(name: &str) -> &'static str {
    match name {
        "emit" | "on" | "addEventListener" => "node_events",
        "subscribe" | "publish" => "generic",
        _ => "generic",
    }
}

// ---------------------------------------------------------------------------
// JS/TS event extraction
// ---------------------------------------------------------------------------

fn extract_js_events(
    tree: &tree_sitter::Tree,
    source: &[u8],
    path: &CanonicalPath,
) -> Vec<Boundary> {
    let mut boundaries = Vec::new();
    walk_js_event_node(&tree.root_node(), source, path, &mut boundaries);
    boundaries
}

fn walk_js_event_node(
    node: &tree_sitter::Node,
    source: &[u8],
    path: &CanonicalPath,
    boundaries: &mut Vec<Boundary>,
) {
    if boundaries.len() >= MAX_BOUNDARIES_PER_FILE {
        return;
    }

    if node.kind() == "call_expression" {
        try_extract_js_event_call(node, source, path, boundaries);
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        walk_js_event_node(&child, source, path, boundaries);
        if boundaries.len() >= MAX_BOUNDARIES_PER_FILE {
            return;
        }
    }
}

fn try_extract_js_event_call(
    node: &tree_sitter::Node,
    source: &[u8],
    path: &CanonicalPath,
    boundaries: &mut Vec<Boundary>,
) {
    let callee = match node.child(0) {
        Some(c) => c,
        None => return,
    };

    let args = match node.child(1).or_else(|| node.child(2)) {
        Some(a) if a.kind() == "arguments" => a,
        _ => return,
    };

    // Bare function call: emit("event"), subscribe("event"), publish("event")
    if callee.kind() == "identifier" {
        let func_name = match callee.utf8_text(source) {
            Ok(n) => n,
            Err(_) => return,
        };

        if let Some(role) = js_event_role(func_name) {
            if let Some(event_name) = first_string_arg(&args, source) {
                if !is_dom_event(event_name) {
                    boundaries.push(Boundary {
                        kind: BoundaryKind::EventChannel,
                        name: event_name.to_string(),
                        role,
                        file: path.clone(),
                        line: node.start_position().row as u32 + 1,
                        framework: Some(js_event_framework(func_name).to_string()),
                        method: None,
                    });
                }
            }
        }
        return;
    }

    // Member expression: obj.emit("event"), emitter.on("event"), etc.
    if callee.kind() == "member_expression" {
        let property = match callee.child_by_field_name("property") {
            Some(p) => p,
            None => return,
        };
        let method_name = match property.utf8_text(source) {
            Ok(m) => m,
            Err(_) => return,
        };

        if let Some(role) = js_event_role(method_name) {
            if let Some(event_name) = first_string_arg(&args, source) {
                if !is_dom_event(event_name) {
                    boundaries.push(Boundary {
                        kind: BoundaryKind::EventChannel,
                        name: event_name.to_string(),
                        role,
                        file: path.clone(),
                        line: node.start_position().row as u32 + 1,
                        framework: Some(js_event_framework(method_name).to_string()),
                        method: None,
                    });
                }
            }
        }
    }
}

/// Get the first string literal argument from an arguments node.
/// Returns None for template_string / template_literal or non-string args (EC-9).
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
            // Skip template strings entirely
            "template_string" | "template_literal" => return None,
            _ => {}
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Python event extraction
// ---------------------------------------------------------------------------

/// Event function names and their roles for Python.
fn python_event_role(name: &str) -> Option<BoundaryRole> {
    match name {
        "emit" => Some(BoundaryRole::Producer),
        "publish" => Some(BoundaryRole::Producer),
        "on" => Some(BoundaryRole::Consumer),
        "subscribe" => Some(BoundaryRole::Consumer),
        _ => None,
    }
}

/// Framework label for Python event functions.
fn python_event_framework(name: &str) -> &'static str {
    match name {
        "subscribe" | "publish" => "generic",
        _ => "node_events",
    }
}

fn extract_python_events(
    tree: &tree_sitter::Tree,
    source: &[u8],
    path: &CanonicalPath,
) -> Vec<Boundary> {
    let mut boundaries = Vec::new();
    walk_python_event_node(&tree.root_node(), source, path, &mut boundaries);
    boundaries
}

fn walk_python_event_node(
    node: &tree_sitter::Node,
    source: &[u8],
    path: &CanonicalPath,
    boundaries: &mut Vec<Boundary>,
) {
    if boundaries.len() >= MAX_BOUNDARIES_PER_FILE {
        return;
    }

    if node.kind() == "call" {
        try_extract_python_event_call(node, source, path, boundaries);
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        walk_python_event_node(&child, source, path, boundaries);
        if boundaries.len() >= MAX_BOUNDARIES_PER_FILE {
            return;
        }
    }
}

fn try_extract_python_event_call(
    node: &tree_sitter::Node,
    source: &[u8],
    path: &CanonicalPath,
    boundaries: &mut Vec<Boundary>,
) {
    // Python call: function(args) — the function is the first child
    let callee = match node.child(0) {
        Some(c) => c,
        None => return,
    };

    // We want attribute calls: obj.emit("event"), bus.publish("event")
    if callee.kind() != "attribute" {
        return;
    }

    let attr_name = match callee.child_by_field_name("attribute") {
        Some(a) => a,
        None => return,
    };
    let method_name = match attr_name.utf8_text(source) {
        Ok(m) => m,
        Err(_) => return,
    };

    let role = match python_event_role(method_name) {
        Some(r) => r,
        None => return,
    };

    // Extract argument list
    let arg_list = match node.child_by_field_name("arguments") {
        Some(a) => a,
        None => return,
    };

    if let Some(event_name) = first_python_string_arg(&arg_list, source) {
        if !is_dom_event(event_name) {
            boundaries.push(Boundary {
                kind: BoundaryKind::EventChannel,
                name: event_name.to_string(),
                role,
                file: path.clone(),
                line: node.start_position().row as u32 + 1,
                framework: Some(python_event_framework(method_name).to_string()),
                method: None,
            });
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
            return child.utf8_text(source).ok().map(strip_quotes);
        }
    }
    None
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
        let extractor = EventExtractor;
        extractor.extract(&tree, source.as_bytes(), &path)
    }

    // --- JS/TS Producer tests ---

    #[test]
    fn js_emit_producer() {
        let source = r#"emitter.emit("user:created", data);"#;
        let lang = tree_sitter::Language::from(tree_sitter_typescript::LANGUAGE_TYPESCRIPT);
        let boundaries = parse_and_extract(source, "ts", lang);
        assert_eq!(boundaries.len(), 1);
        assert_eq!(boundaries[0].name, "user:created");
        assert_eq!(boundaries[0].role, BoundaryRole::Producer);
        assert_eq!(boundaries[0].kind, BoundaryKind::EventChannel);
        assert_eq!(boundaries[0].framework.as_deref(), Some("node_events"));
        assert_eq!(boundaries[0].method, None);
    }

    #[test]
    fn js_bare_emit_producer() {
        let source = r#"emit("order:placed");"#;
        let lang = tree_sitter::Language::from(tree_sitter_typescript::LANGUAGE_TYPESCRIPT);
        let boundaries = parse_and_extract(source, "ts", lang);
        assert_eq!(boundaries.len(), 1);
        assert_eq!(boundaries[0].name, "order:placed");
        assert_eq!(boundaries[0].role, BoundaryRole::Producer);
    }

    #[test]
    fn js_publish_producer() {
        let source = r#"bus.publish("notification:sent");"#;
        let lang = tree_sitter::Language::from(tree_sitter_typescript::LANGUAGE_TYPESCRIPT);
        let boundaries = parse_and_extract(source, "ts", lang);
        assert_eq!(boundaries.len(), 1);
        assert_eq!(boundaries[0].name, "notification:sent");
        assert_eq!(boundaries[0].role, BoundaryRole::Producer);
        assert_eq!(boundaries[0].framework.as_deref(), Some("generic"));
    }

    // --- JS/TS Consumer tests ---

    #[test]
    fn js_on_consumer() {
        let source = r#"emitter.on("user:created", handler);"#;
        let lang = tree_sitter::Language::from(tree_sitter_typescript::LANGUAGE_TYPESCRIPT);
        let boundaries = parse_and_extract(source, "ts", lang);
        assert_eq!(boundaries.len(), 1);
        assert_eq!(boundaries[0].name, "user:created");
        assert_eq!(boundaries[0].role, BoundaryRole::Consumer);
        assert_eq!(boundaries[0].framework.as_deref(), Some("node_events"));
    }

    #[test]
    fn js_addeventlistener_consumer() {
        let source = r#"target.addEventListener("message", handler);"#;
        let lang = tree_sitter::Language::from(tree_sitter_typescript::LANGUAGE_TYPESCRIPT);
        let boundaries = parse_and_extract(source, "ts", lang);
        assert_eq!(boundaries.len(), 1);
        assert_eq!(boundaries[0].name, "message");
        assert_eq!(boundaries[0].role, BoundaryRole::Consumer);
        assert_eq!(boundaries[0].framework.as_deref(), Some("node_events"));
    }

    #[test]
    fn js_subscribe_consumer() {
        let source = r#"bus.subscribe("order:placed", handler);"#;
        let lang = tree_sitter::Language::from(tree_sitter_typescript::LANGUAGE_TYPESCRIPT);
        let boundaries = parse_and_extract(source, "ts", lang);
        assert_eq!(boundaries.len(), 1);
        assert_eq!(boundaries[0].name, "order:placed");
        assert_eq!(boundaries[0].role, BoundaryRole::Consumer);
        assert_eq!(boundaries[0].framework.as_deref(), Some("generic"));
    }

    // --- DOM event skip tests ---

    #[test]
    fn dom_click_skipped() {
        let source = r#"element.addEventListener("click", handler);"#;
        let lang = tree_sitter::Language::from(tree_sitter_typescript::LANGUAGE_TYPESCRIPT);
        let boundaries = parse_and_extract(source, "ts", lang);
        assert!(boundaries.is_empty());
    }

    #[test]
    fn dom_submit_skipped() {
        let source = r#"form.on("submit", handler);"#;
        let lang = tree_sitter::Language::from(tree_sitter_typescript::LANGUAGE_TYPESCRIPT);
        let boundaries = parse_and_extract(source, "ts", lang);
        assert!(boundaries.is_empty());
    }

    #[test]
    fn dom_load_skipped() {
        let source = r#"window.addEventListener("load", handler);"#;
        let lang = tree_sitter::Language::from(tree_sitter_typescript::LANGUAGE_TYPESCRIPT);
        let boundaries = parse_and_extract(source, "ts", lang);
        assert!(boundaries.is_empty());
    }

    // --- Edge cases ---

    #[test]
    fn variable_event_name_not_detected() {
        let source = r#"emitter.emit(eventName, data);"#;
        let lang = tree_sitter::Language::from(tree_sitter_typescript::LANGUAGE_TYPESCRIPT);
        let boundaries = parse_and_extract(source, "ts", lang);
        assert!(boundaries.is_empty());
    }

    #[test]
    fn template_literal_event_not_detected() {
        let source = "emitter.emit(`user:${action}`, data);";
        let lang = tree_sitter::Language::from(tree_sitter_typescript::LANGUAGE_TYPESCRIPT);
        let boundaries = parse_and_extract(source, "ts", lang);
        assert!(boundaries.is_empty());
    }

    #[test]
    fn same_event_both_roles_in_one_file() {
        let source = r#"
emitter.emit("sync:complete", data);
emitter.on("sync:complete", handler);
"#;
        let lang = tree_sitter::Language::from(tree_sitter_typescript::LANGUAGE_TYPESCRIPT);
        let boundaries = parse_and_extract(source, "ts", lang);
        assert_eq!(boundaries.len(), 2);
        assert_eq!(boundaries[0].role, BoundaryRole::Producer);
        assert_eq!(boundaries[1].role, BoundaryRole::Consumer);
    }

    #[test]
    fn multiple_events_in_one_file() {
        let source = r#"
emitter.emit("user:created", data);
emitter.on("order:placed", handler);
bus.subscribe("payment:received", callback);
"#;
        let lang = tree_sitter::Language::from(tree_sitter_typescript::LANGUAGE_TYPESCRIPT);
        let boundaries = parse_and_extract(source, "ts", lang);
        assert_eq!(boundaries.len(), 3);
    }

    // --- Python tests ---

    #[test]
    fn python_emit_producer() {
        let source = r#"emitter.emit("user_created", data)"#;
        let lang = tree_sitter::Language::from(tree_sitter_python::LANGUAGE);
        let boundaries = parse_and_extract(source, "py", lang);
        assert_eq!(boundaries.len(), 1);
        assert_eq!(boundaries[0].name, "user_created");
        assert_eq!(boundaries[0].role, BoundaryRole::Producer);
        assert_eq!(boundaries[0].framework.as_deref(), Some("node_events"));
    }

    #[test]
    fn python_publish_producer() {
        let source = r#"bus.publish("order_placed")"#;
        let lang = tree_sitter::Language::from(tree_sitter_python::LANGUAGE);
        let boundaries = parse_and_extract(source, "py", lang);
        assert_eq!(boundaries.len(), 1);
        assert_eq!(boundaries[0].name, "order_placed");
        assert_eq!(boundaries[0].role, BoundaryRole::Producer);
        assert_eq!(boundaries[0].framework.as_deref(), Some("generic"));
    }

    #[test]
    fn python_subscribe_consumer() {
        let source = r#"bus.subscribe("order_placed", handler)"#;
        let lang = tree_sitter::Language::from(tree_sitter_python::LANGUAGE);
        let boundaries = parse_and_extract(source, "py", lang);
        assert_eq!(boundaries.len(), 1);
        assert_eq!(boundaries[0].name, "order_placed");
        assert_eq!(boundaries[0].role, BoundaryRole::Consumer);
        assert_eq!(boundaries[0].framework.as_deref(), Some("generic"));
    }

    #[test]
    fn python_on_consumer() {
        let source = r#"emitter.on("data_ready", handler)"#;
        let lang = tree_sitter::Language::from(tree_sitter_python::LANGUAGE);
        let boundaries = parse_and_extract(source, "py", lang);
        assert_eq!(boundaries.len(), 1);
        assert_eq!(boundaries[0].name, "data_ready");
        assert_eq!(boundaries[0].role, BoundaryRole::Consumer);
    }

    #[test]
    fn python_dom_event_skipped() {
        let source = r#"widget.on("click", handler)"#;
        let lang = tree_sitter::Language::from(tree_sitter_python::LANGUAGE);
        let boundaries = parse_and_extract(source, "py", lang);
        assert!(boundaries.is_empty());
    }

    #[test]
    fn python_variable_event_not_detected() {
        let source = r#"emitter.emit(event_name, data)"#;
        let lang = tree_sitter::Language::from(tree_sitter_python::LANGUAGE);
        let boundaries = parse_and_extract(source, "py", lang);
        assert!(boundaries.is_empty());
    }

    // --- Extension tests ---

    #[test]
    fn jsx_extension_supported() {
        let source = r#"emitter.emit("render:done");"#;
        let lang = tree_sitter::Language::from(tree_sitter_typescript::LANGUAGE_TSX);
        let boundaries = parse_and_extract(source, "jsx", lang);
        assert_eq!(boundaries.len(), 1);
    }

    #[test]
    fn empty_for_unknown_extension() {
        let extractor = EventExtractor;
        let path = CanonicalPath::new("test.rb".to_string());
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
    fn line_numbers_are_one_based() {
        let source = r#"emitter.emit("test:event");"#;
        let lang = tree_sitter::Language::from(tree_sitter_typescript::LANGUAGE_TYPESCRIPT);
        let boundaries = parse_and_extract(source, "ts", lang);
        assert_eq!(boundaries[0].line, 1);
    }
}
