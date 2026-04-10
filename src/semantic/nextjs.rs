//! Next.js semantic boundary extraction (D-152).
//!
//! Maps Next.js route convention files to HttpRoute boundaries and
//! "use client" directives to EventChannel ClientBoundary markers.

use crate::detect::js_framework::{self, RouteConvention};
use crate::model::semantic::{Boundary, BoundaryKind, BoundaryRole};
use crate::model::CanonicalPath;
use crate::semantic::BoundaryExtractor;

/// Extracts Next.js route and client boundary markers.
pub struct NextBoundaryExtractor;

impl BoundaryExtractor for NextBoundaryExtractor {
    fn extensions(&self) -> &[&str] {
        &["ts", "tsx", "js", "jsx"]
    }

    fn extract(
        &self,
        tree: &tree_sitter::Tree,
        source: &[u8],
        path: &CanonicalPath,
    ) -> Vec<Boundary> {
        let mut boundaries = Vec::new();

        // Route convention → HttpRoute boundary
        if let Some(convention) = js_framework::classify_route_convention(path) {
            let route_path = derive_route_path(path, &convention);
            match convention {
                RouteConvention::NextPage
                | RouteConvention::NextLayout
                | RouteConvention::NextLoading
                | RouteConvention::NextError => {
                    boundaries.push(Boundary {
                        kind: BoundaryKind::HttpRoute,
                        name: route_path,
                        role: BoundaryRole::Producer,
                        file: path.clone(),
                        line: 0,
                        framework: Some("nextjs".to_string()),
                        method: None,
                    });
                }
                RouteConvention::NextApiRoute => {
                    boundaries.push(Boundary {
                        kind: BoundaryKind::HttpRoute,
                        name: route_path,
                        role: BoundaryRole::Producer,
                        file: path.clone(),
                        line: 0,
                        framework: Some("nextjs".to_string()),
                        method: None,
                    });
                }
                RouteConvention::NextMiddleware => {
                    boundaries.push(Boundary {
                        kind: BoundaryKind::HttpRoute,
                        name: "Middleware:/".to_string(),
                        role: BoundaryRole::Producer,
                        file: path.clone(),
                        line: 0,
                        framework: Some("nextjs".to_string()),
                        method: None,
                    });
                }
            }
        }

        // "use client" → ClientBoundary EventChannel
        if js_framework::has_use_client_directive(tree, source) {
            boundaries.push(Boundary {
                kind: BoundaryKind::EventChannel,
                name: "ClientBoundary".to_string(),
                role: BoundaryRole::Producer,
                file: path.clone(),
                line: 0,
                framework: Some("nextjs".to_string()),
                method: None,
            });
        }

        boundaries
    }
}

/// Derive a route path from a file's CanonicalPath and its convention.
fn derive_route_path(path: &CanonicalPath, convention: &RouteConvention) -> String {
    let path_str = path.as_str();

    // App Router: strip app/ prefix (or */app/) and the filename
    if let Some(rest) = strip_prefix(path_str, "app/") {
        let dir = rest.rsplit_once('/').map(|(d, _)| d).unwrap_or("");
        let route = if dir.is_empty() { "/" } else { &format!("/{}", dir) };
        return match convention {
            RouteConvention::NextApiRoute => format!("API:{}", route),
            _ => route.to_string(),
        };
    }

    // Pages Router: strip pages/ prefix, strip extension, convert to route
    if let Some(rest) = strip_prefix(path_str, "pages/") {
        let without_ext = rest.rsplit_once('.').map(|(s, _)| s).unwrap_or(rest);
        // API routes
        if without_ext.starts_with("api/") {
            return format!("API:/{}", without_ext);
        }
        // index → /
        if without_ext == "index" {
            return "/".to_string();
        }
        if let Some(prefix) = without_ext.strip_suffix("/index") {
            return format!("/{}", prefix);
        }
        return format!("/{}", without_ext);
    }

    // Middleware or fallback
    "/".to_string()
}

/// Strip `prefix/` or `*/prefix/` from a path.
fn strip_prefix<'a>(path: &'a str, prefix: &str) -> Option<&'a str> {
    if let Some(rest) = path.strip_prefix(prefix) {
        return Some(rest);
    }
    let search = format!("/{}", prefix);
    if let Some(pos) = path.find(&search) {
        return Some(&path[pos + search.len()..]);
    }
    None
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

    fn extract(source: &str, path: &str) -> Vec<Boundary> {
        let tree = parse_tsx(source);
        let cp = CanonicalPath::new(path.to_string());
        NextBoundaryExtractor.extract(&tree, source.as_bytes(), &cp)
    }

    // --- SC-26: HttpRoute for page ---

    #[test]
    fn app_page_produces_http_route() {
        let boundaries = extract(
            "export default function Page() { return <div/>; }",
            "app/dashboard/page.tsx",
        );
        let route = boundaries.iter().find(|b| b.kind == BoundaryKind::HttpRoute).unwrap();
        assert_eq!(route.name, "/dashboard");
        assert_eq!(route.role, BoundaryRole::Producer);
        assert_eq!(route.framework.as_deref(), Some("nextjs"));
    }

    // --- SC-27: HttpRoute for API route ---

    #[test]
    fn app_api_route_produces_http_route() {
        let boundaries = extract(
            "export async function GET() { return Response.json({}); }",
            "app/api/users/route.ts",
        );
        let route = boundaries.iter().find(|b| b.kind == BoundaryKind::HttpRoute).unwrap();
        assert_eq!(route.name, "API:/api/users");
    }

    #[test]
    fn app_layout_produces_http_route() {
        let boundaries = extract(
            "export default function Layout({ children }) { return <div>{children}</div>; }",
            "app/dashboard/layout.tsx",
        );
        assert_eq!(boundaries.len(), 1);
        assert_eq!(boundaries[0].name, "/dashboard");
    }

    #[test]
    fn middleware_produces_http_route() {
        let boundaries = extract(
            "export function middleware(req) { return NextResponse.next(); }",
            "middleware.ts",
        );
        assert_eq!(boundaries.len(), 1);
        assert_eq!(boundaries[0].name, "Middleware:/");
    }

    #[test]
    fn use_client_produces_client_boundary() {
        let boundaries = extract(
            "\"use client\";\nexport default function Counter() { return <div/>; }",
            "app/components/Counter.tsx",
        );
        let client = boundaries.iter().find(|b| b.kind == BoundaryKind::EventChannel).unwrap();
        assert_eq!(client.name, "ClientBoundary");
        assert_eq!(client.role, BoundaryRole::Producer);
    }

    #[test]
    fn page_with_use_client_has_both_boundaries() {
        let boundaries = extract(
            "\"use client\";\nexport default function Page() { return <div/>; }",
            "app/dashboard/page.tsx",
        );
        assert_eq!(boundaries.len(), 2);
        assert!(boundaries.iter().any(|b| b.kind == BoundaryKind::HttpRoute));
        assert!(boundaries.iter().any(|b| b.kind == BoundaryKind::EventChannel));
    }

    #[test]
    fn regular_file_no_boundaries() {
        let boundaries = extract(
            "export function add(a: number, b: number) { return a + b; }",
            "src/utils.ts",
        );
        assert!(boundaries.is_empty());
    }

    #[test]
    fn pages_router_page() {
        let boundaries = extract(
            "export default function About() { return <div/>; }",
            "pages/about.tsx",
        );
        assert_eq!(boundaries.len(), 1);
        assert_eq!(boundaries[0].name, "/about");
    }

    #[test]
    fn pages_router_api() {
        let boundaries = extract(
            "export default function handler(req, res) { res.json({}); }",
            "pages/api/users.ts",
        );
        assert_eq!(boundaries.len(), 1);
        assert_eq!(boundaries[0].name, "API:/api/users");
    }

    #[test]
    fn root_page() {
        let boundaries = extract(
            "export default function Home() { return <div/>; }",
            "app/page.tsx",
        );
        assert_eq!(boundaries[0].name, "/");
    }
}
