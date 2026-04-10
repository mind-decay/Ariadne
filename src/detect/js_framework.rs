//! JS/TS framework detection from tree-sitter TypeScript/TSX AST.
//!
//! Detects React component patterns, hooks, context providers/consumers,
//! client/server component directives, and Next.js route conventions.

use crate::model::CanonicalPath;

/// Route convention classification for Next.js file-system routing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RouteConvention {
    NextPage,
    NextLayout,
    NextApiRoute,
    NextMiddleware,
    NextLoading,
    NextError,
}

/// Hints about which JS/TS framework patterns are present in a source file.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct JsFrameworkHints {
    pub react_component: bool,
    pub react_hook: bool,
    pub context_provider: Vec<String>,
    pub context_consumer: Vec<String>,
    pub server_component: bool,
    pub client_component: bool,
    pub route_convention: Option<RouteConvention>,
}

/// Detect JS/TS framework patterns from a parsed tree-sitter tree.
pub fn detect_js_framework(
    tree: &tree_sitter::Tree,
    source: &[u8],
    path: &CanonicalPath,
) -> JsFrameworkHints {
    let mut hints = JsFrameworkHints::default();

    let has_use_client = has_use_client_directive(tree, source);
    let has_use_server = has_use_server_directive(tree, source);

    hints.client_component = has_use_client;

    // Server component: under app/ without "use client" and not "use server" action file
    let path_str = path.as_str();
    if !has_use_client && !has_use_server && is_under_app_dir(path_str) {
        hints.server_component = true;
    }

    hints.route_convention = classify_route_convention(path);
    hints.context_provider = find_create_context_calls(tree, source);
    hints.context_consumer = find_use_context_calls(tree, source);

    // Walk the AST for component and hook detection
    let root = tree.root_node();
    walk_for_hints(&root, source, &mut hints);

    hints
}

// ---------------------------------------------------------------------------
// Public helpers for C7/C9 boundary extractors
// ---------------------------------------------------------------------------

/// Find all `createContext(...)` calls and return the variable names they are assigned to.
pub fn find_create_context_calls(tree: &tree_sitter::Tree, source: &[u8]) -> Vec<String> {
    let mut names = Vec::new();
    let root = tree.root_node();
    find_create_context_recursive(&root, source, &mut names);
    names.sort();
    names
}

/// Find all `useContext(X)` calls and return the argument names.
pub fn find_use_context_calls(tree: &tree_sitter::Tree, source: &[u8]) -> Vec<String> {
    let mut names = Vec::new();
    let root = tree.root_node();
    find_use_context_recursive(&root, source, &mut names);
    names.sort();
    names
}

/// Check if the file starts with `"use client"` directive.
pub fn has_use_client_directive(tree: &tree_sitter::Tree, source: &[u8]) -> bool {
    has_directive(tree, source, "use client")
}

/// Secondary extensions that disqualify a file from being a route.
const NON_ROUTE_SECONDARY_EXTS: &[&str] = &[
    "styles", "style", "css", "module",
    "test", "spec", "mock", "fixture", "snap",
    "stories", "story",
    "d",
];

/// Classify a file path into a Next.js route convention.
pub fn classify_route_convention(path: &CanonicalPath) -> Option<RouteConvention> {
    let path_str = path.as_str();
    let file_name = path_str.rsplit('/').next().unwrap_or(path_str);

    // Filter out secondary extensions: Login.styles.ts, page.test.tsx, etc.
    if has_non_route_secondary_ext(file_name) {
        return None;
    }

    let stem = file_name.split('.').next().unwrap_or("");

    // middleware.ts at any level
    if stem == "middleware" {
        return Some(RouteConvention::NextMiddleware);
    }

    // App Router conventions (under app/)
    if is_under_app_dir(path_str) {
        // API routes: app/api/**/route.{ts,js}
        if stem == "route" && path_str.contains("/api/") {
            return Some(RouteConvention::NextApiRoute);
        }
        return match stem {
            "page" => Some(RouteConvention::NextPage),
            "layout" => Some(RouteConvention::NextLayout),
            "loading" => Some(RouteConvention::NextLoading),
            "error" => Some(RouteConvention::NextError),
            "route" => Some(RouteConvention::NextApiRoute),
            _ => None,
        };
    }

    // Pages Router conventions (under pages/)
    if is_under_pages_dir(path_str) {
        // pages/api/**/*.ts → API route
        if path_str.contains("/api/") || path_str.starts_with("pages/api/") {
            return Some(RouteConvention::NextApiRoute);
        }
        // pages/**/*.tsx → page
        return Some(RouteConvention::NextPage);
    }

    None
}

fn has_non_route_secondary_ext(file_name: &str) -> bool {
    let parts: Vec<&str> = file_name.split('.').collect();
    if parts.len() >= 3 {
        for part in &parts[1..parts.len() - 1] {
            if NON_ROUTE_SECONDARY_EXTS.contains(part) {
                return true;
            }
        }
    }
    false
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

fn has_use_server_directive(tree: &tree_sitter::Tree, source: &[u8]) -> bool {
    has_directive(tree, source, "use server")
}

/// Check if the first expression statement in the program is a string literal directive.
fn has_directive(tree: &tree_sitter::Tree, source: &[u8], directive: &str) -> bool {
    let root = tree.root_node();
    for i in 0..root.child_count() {
        let child = match root.child(i) {
            Some(c) => c,
            None => continue,
        };
        // Skip comments and import statements
        match child.kind() {
            "comment" | "hash_bang_line" => continue,
            "expression_statement" => {
                // Check if it's a string literal matching the directive
                if let Some(expr) = child.child(0) {
                    if expr.kind() == "string" {
                        let text = expr.utf8_text(source).unwrap_or("");
                        let unquoted = text.trim_matches(|c| c == '\'' || c == '"');
                        return unquoted == directive;
                    }
                }
                return false;
            }
            _ => return false,
        }
    }
    false
}

fn is_under_app_dir(path: &str) -> bool {
    path.starts_with("app/") || path.contains("/app/")
}

fn is_under_pages_dir(path: &str) -> bool {
    path.starts_with("pages/") || path.contains("/pages/")
}

/// Walk AST nodes detecting React component and hook patterns.
fn walk_for_hints(node: &tree_sitter::Node, source: &[u8], hints: &mut JsFrameworkHints) {
    match node.kind() {
        "jsx_element" | "jsx_self_closing_element" | "jsx_fragment" => {
            hints.react_component = true;
        }
        "function_declaration" => {
            if let Some(name_node) = node.child_by_field_name("name") {
                let name = name_node.utf8_text(source).unwrap_or("");
                if name.starts_with("use") && name.len() > 3 && name.as_bytes()[3].is_ascii_uppercase() {
                    hints.react_hook = true;
                }
            }
        }
        "lexical_declaration" | "variable_declaration" => {
            // const useFoo = () => { ... } or const useFoo = function() { ... }
            check_variable_hook(node, source, hints);
        }
        "export_statement" => {
            // export function useFoo() or export default function useFoo()
            // The function_declaration child will be visited recursively
        }
        _ => {}
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        walk_for_hints(&child, source, hints);
    }
}

/// Check variable declarations for hook patterns: `const useFoo = () => ...`
fn check_variable_hook(node: &tree_sitter::Node, source: &[u8], hints: &mut JsFrameworkHints) {
    for i in 0..node.child_count() {
        let child = match node.child(i) {
            Some(c) => c,
            None => continue,
        };
        if child.kind() == "variable_declarator" {
            if let Some(name_node) = child.child_by_field_name("name") {
                let name = name_node.utf8_text(source).unwrap_or("");
                if name.starts_with("use") && name.len() > 3 && name.as_bytes()[3].is_ascii_uppercase() {
                    // Check if the value is a function expression or arrow function
                    if let Some(value) = child.child_by_field_name("value") {
                        if value.kind() == "arrow_function" || value.kind() == "function" {
                            hints.react_hook = true;
                        }
                    }
                }
            }
        }
    }
}

/// Recursively find `createContext(...)` calls and extract the variable name.
fn find_create_context_recursive(
    node: &tree_sitter::Node,
    source: &[u8],
    names: &mut Vec<String>,
) {
    // Pattern: const X = createContext(...) or const X = React.createContext(...)
    if node.kind() == "variable_declarator" {
        if let Some(value) = node.child_by_field_name("value") {
            if value.kind() == "call_expression" {
                if let Some(func) = value.child_by_field_name("function") {
                    let func_text = func.utf8_text(source).unwrap_or("");
                    if func_text == "createContext" || func_text == "React.createContext" {
                        if let Some(name_node) = node.child_by_field_name("name") {
                            let name = name_node.utf8_text(source).unwrap_or("");
                            if !name.is_empty() {
                                names.push(name.to_string());
                            }
                        }
                    }
                }
            }
        }
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        find_create_context_recursive(&child, source, names);
    }
}

/// Recursively find `useContext(X)` calls and extract the argument name.
fn find_use_context_recursive(
    node: &tree_sitter::Node,
    source: &[u8],
    names: &mut Vec<String>,
) {
    if node.kind() == "call_expression" {
        if let Some(func) = node.child_by_field_name("function") {
            let func_text = func.utf8_text(source).unwrap_or("");
            if func_text == "useContext" {
                if let Some(args) = node.child_by_field_name("arguments") {
                    // First non-punctuation child is the context argument
                    for i in 0..args.child_count() {
                        if let Some(arg) = args.child(i) {
                            if arg.kind() != "(" && arg.kind() != ")" && arg.kind() != "," {
                                let arg_text = arg.utf8_text(source).unwrap_or("");
                                if !arg_text.is_empty() {
                                    names.push(arg_text.to_string());
                                }
                                break;
                            }
                        }
                    }
                }
            }
        }
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        find_use_context_recursive(&child, source, names);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_tsx(source: &str) -> tree_sitter::Tree {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter::Language::from(
                tree_sitter_typescript::LANGUAGE_TSX,
            ))
            .unwrap();
        parser.parse(source, None).unwrap()
    }

    fn detect(source: &str, path: &str) -> JsFrameworkHints {
        let tree = parse_tsx(source);
        let cp = CanonicalPath::new(path.to_string());
        detect_js_framework(&tree, source.as_bytes(), &cp)
    }

    // --- SC-13: React component detection ---

    #[test]
    fn react_component_detected() {
        let hints = detect(
            "function Button() { return <button>Click</button> }",
            "src/Button.tsx",
        );
        assert!(hints.react_component);
    }

    #[test]
    fn react_jsx_fragment_detected() {
        let hints = detect(
            "function App() { return <>Hello</> }",
            "src/App.tsx",
        );
        assert!(hints.react_component);
    }

    #[test]
    fn non_react_file_not_component() {
        let hints = detect(
            "export function add(a: number, b: number) { return a + b; }",
            "src/utils.ts",
        );
        assert!(!hints.react_component);
    }

    // --- SC-14: React hook detection ---

    #[test]
    fn react_hook_function_declaration() {
        let hints = detect(
            "function useAuth() { return { user: null }; }",
            "src/hooks/useAuth.ts",
        );
        assert!(hints.react_hook);
    }

    #[test]
    fn react_hook_arrow_function() {
        let hints = detect(
            "const useTheme = () => { return 'dark'; };",
            "src/hooks/useTheme.ts",
        );
        assert!(hints.react_hook);
    }

    #[test]
    fn not_a_hook_wrong_prefix() {
        let hints = detect(
            "function userService() { return {}; }",
            "src/services.ts",
        );
        assert!(!hints.react_hook);
    }

    #[test]
    fn not_a_hook_lowercase_after_use() {
        let hints = detect(
            "function useless() { return null; }",
            "src/utils.ts",
        );
        assert!(!hints.react_hook);
    }

    // --- SC-15: Context provider detection ---

    #[test]
    fn context_provider_create_context() {
        let hints = detect(
            "const ThemeContext = createContext(defaultTheme);",
            "src/contexts/theme.ts",
        );
        assert_eq!(hints.context_provider, vec!["ThemeContext"]);
    }

    #[test]
    fn context_provider_react_create_context() {
        let hints = detect(
            "const AuthContext = React.createContext(null);",
            "src/contexts/auth.ts",
        );
        assert_eq!(hints.context_provider, vec!["AuthContext"]);
    }

    // --- SC-16: Context consumer detection ---

    #[test]
    fn context_consumer_use_context() {
        let hints = detect(
            "const theme = useContext(ThemeContext);",
            "src/components/Themed.tsx",
        );
        assert_eq!(hints.context_consumer, vec!["ThemeContext"]);
    }

    #[test]
    fn context_consumer_multiple() {
        let hints = detect(
            r#"
const theme = useContext(ThemeContext);
const auth = useContext(AuthContext);
"#,
            "src/components/App.tsx",
        );
        assert_eq!(hints.context_consumer, vec!["AuthContext", "ThemeContext"]);
    }

    // --- SC-17: Client component detection ---

    #[test]
    fn client_component_use_client() {
        let hints = detect(
            "\"use client\";\nimport React from 'react';",
            "app/components/Counter.tsx",
        );
        assert!(hints.client_component);
        assert!(!hints.server_component);
    }

    #[test]
    fn client_component_single_quotes() {
        let hints = detect(
            "'use client';\nimport React from 'react';",
            "app/components/Counter.tsx",
        );
        assert!(hints.client_component);
    }

    // --- SC-18: Server component detection ---

    #[test]
    fn server_component_under_app_no_directive() {
        let hints = detect(
            "import { db } from '@/lib/db';\nexport default function Page() { return <div/>; }",
            "app/dashboard/page.tsx",
        );
        assert!(hints.server_component);
        assert!(!hints.client_component);
    }

    #[test]
    fn not_server_component_outside_app() {
        let hints = detect(
            "export default function Component() { return <div/>; }",
            "src/components/Foo.tsx",
        );
        assert!(!hints.server_component);
    }

    #[test]
    fn not_server_component_with_use_client() {
        let hints = detect(
            "\"use client\";\nexport default function Counter() { return <div/>; }",
            "app/components/Counter.tsx",
        );
        assert!(!hints.server_component);
        assert!(hints.client_component);
    }

    // --- Route convention classification ---

    #[test]
    fn route_app_page() {
        let path = CanonicalPath::new("app/dashboard/page.tsx".to_string());
        assert_eq!(
            classify_route_convention(&path),
            Some(RouteConvention::NextPage)
        );
    }

    #[test]
    fn route_app_layout() {
        let path = CanonicalPath::new("app/dashboard/layout.tsx".to_string());
        assert_eq!(
            classify_route_convention(&path),
            Some(RouteConvention::NextLayout)
        );
    }

    #[test]
    fn route_app_api() {
        let path = CanonicalPath::new("app/api/users/route.ts".to_string());
        assert_eq!(
            classify_route_convention(&path),
            Some(RouteConvention::NextApiRoute)
        );
    }

    #[test]
    fn route_middleware() {
        let path = CanonicalPath::new("middleware.ts".to_string());
        assert_eq!(
            classify_route_convention(&path),
            Some(RouteConvention::NextMiddleware)
        );
    }

    #[test]
    fn route_app_loading() {
        let path = CanonicalPath::new("app/dashboard/loading.tsx".to_string());
        assert_eq!(
            classify_route_convention(&path),
            Some(RouteConvention::NextLoading)
        );
    }

    #[test]
    fn route_app_error() {
        let path = CanonicalPath::new("app/dashboard/error.tsx".to_string());
        assert_eq!(
            classify_route_convention(&path),
            Some(RouteConvention::NextError)
        );
    }

    #[test]
    fn route_pages_router_page() {
        let path = CanonicalPath::new("pages/about.tsx".to_string());
        assert_eq!(
            classify_route_convention(&path),
            Some(RouteConvention::NextPage)
        );
    }

    #[test]
    fn route_pages_router_api() {
        let path = CanonicalPath::new("pages/api/users.ts".to_string());
        assert_eq!(
            classify_route_convention(&path),
            Some(RouteConvention::NextApiRoute)
        );
    }

    #[test]
    fn route_regular_file_no_convention() {
        let path = CanonicalPath::new("src/utils/helpers.ts".to_string());
        assert_eq!(classify_route_convention(&path), None);
    }

    #[test]
    fn route_styles_file_not_a_route() {
        let path = CanonicalPath::new("pages/NotFound/NotFound.styles.ts".to_string());
        assert_eq!(classify_route_convention(&path), None);
    }

    #[test]
    fn route_test_file_not_a_route() {
        let path = CanonicalPath::new("app/dashboard/page.test.tsx".to_string());
        assert_eq!(classify_route_convention(&path), None);
    }

    // --- Combined detection ---

    #[test]
    fn full_react_page_detection() {
        let source = r#""use client";
import { useContext } from 'react';
import { ThemeContext } from '../contexts/theme';

export default function Dashboard() {
    const theme = useContext(ThemeContext);
    return <div className={theme}>Dashboard</div>;
}
"#;
        let hints = detect(source, "app/dashboard/page.tsx");
        assert!(hints.react_component);
        assert!(hints.client_component);
        assert!(!hints.server_component);
        assert_eq!(hints.context_consumer, vec!["ThemeContext"]);
        assert_eq!(hints.route_convention, Some(RouteConvention::NextPage));
    }
}
