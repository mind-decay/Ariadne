//! Next.js filesystem routing discovery.
//!
//! Detects App Router and Pages Router conventions by scanning known files
//! for Next.js route patterns (page, layout, loading, error, template,
//! not-found, route, middleware).

use std::path::Path;

use crate::diagnostic::DiagnosticCollector;
use crate::model::{CanonicalPath, FileSet};

/// Next.js route information discovered from filesystem conventions.
#[derive(Clone, Debug)]
pub struct NextRouteInfo {
    pub routes: Vec<NextRoute>,
    pub router_type: NextRouterType,
}

/// A single discovered route.
#[derive(Clone, Debug)]
pub struct NextRoute {
    pub path: String,
    pub file: CanonicalPath,
    pub kind: NextRouteKind,
}

/// Which Next.js router is in use.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NextRouterType {
    AppRouter,
    PagesRouter,
    Both,
}

/// Classification of a Next.js route file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NextRouteKind {
    Page,
    Layout,
    Loading,
    Error,
    Template,
    NotFound,
    ApiRoute,
    Middleware,
}

/// Route-eligible file extensions.
const ROUTE_EXTENSIONS: &[&str] = &["ts", "tsx", "js", "jsx"];

/// Secondary extensions that disqualify a file from being a route.
/// E.g., `page.styles.ts` or `Login.test.tsx` are not routes.
const NON_ROUTE_SECONDARY_EXTS: &[&str] = &[
    "styles", "style", "css", "module",
    "test", "spec", "mock", "fixture", "snap",
    "stories", "story",
    "d",
];

/// Discover Next.js routes from filesystem conventions.
pub fn discover_next_routes(
    _project_root: &Path,
    known_files: &FileSet,
    _diag: &DiagnosticCollector,
) -> Option<NextRouteInfo> {
    let mut routes = Vec::new();
    let mut has_app_router = false;
    let mut has_pages_router = false;

    // Check for middleware at root
    for ext in ROUTE_EXTENSIONS {
        let middleware_path = format!("middleware.{}", ext);
        let cp = CanonicalPath::new(&middleware_path);
        if known_files.contains(&cp) {
            routes.push(NextRoute {
                path: "/".to_string(),
                file: cp,
                kind: NextRouteKind::Middleware,
            });
            break;
        }
    }

    for file in known_files.iter() {
        let file_str = file.as_str();

        // App Router: app/**/{page,layout,loading,error,template,not-found,route}.{ts,tsx,js,jsx}
        if let Some(rest) = strip_app_prefix(file_str) {
            if let Some((kind, route_path)) = classify_app_route(rest) {
                has_app_router = true;
                routes.push(NextRoute {
                    path: route_path,
                    file: file.clone(),
                    kind,
                });
            }
        }

        // Pages Router: pages/**/*.{ts,tsx,js,jsx}
        if let Some(rest) = strip_pages_prefix(file_str) {
            if let Some((kind, route_path)) = classify_pages_route(rest) {
                has_pages_router = true;
                routes.push(NextRoute {
                    path: route_path,
                    file: file.clone(),
                    kind,
                });
            }
        }
    }

    if routes.is_empty() {
        return None;
    }

    // Sort routes for determinism (D-006)
    routes.sort_by(|a, b| a.file.cmp(&b.file));

    let router_type = match (has_app_router, has_pages_router) {
        (true, true) => NextRouterType::Both,
        (true, false) => NextRouterType::AppRouter,
        (false, true) => NextRouterType::PagesRouter,
        (false, false) => return Some(NextRouteInfo {
            routes,
            router_type: NextRouterType::AppRouter, // middleware-only
        }),
    };

    Some(NextRouteInfo {
        routes,
        router_type,
    })
}

/// Strip `app/` or `*/app/` prefix, returning the rest of the path.
fn strip_app_prefix(path: &str) -> Option<&str> {
    if let Some(rest) = path.strip_prefix("app/") {
        return Some(rest);
    }
    // Monorepo: packages/web/app/...
    if let Some(pos) = path.find("/app/") {
        return Some(&path[pos + 5..]);
    }
    None
}

/// Strip `pages/` or `*/pages/` prefix, returning the rest of the path.
fn strip_pages_prefix(path: &str) -> Option<&str> {
    if let Some(rest) = path.strip_prefix("pages/") {
        return Some(rest);
    }
    if let Some(pos) = path.find("/pages/") {
        return Some(&path[pos + 7..]);
    }
    None
}

/// Check if a filename has a secondary extension that disqualifies it as a route.
/// E.g., `NotFound.styles.ts` has secondary ext "styles".
fn has_non_route_secondary_ext(file_name: &str) -> bool {
    // Split on dots: ["Login", "styles", "ts"]
    let parts: Vec<&str> = file_name.split('.').collect();
    if parts.len() >= 3 {
        // Check middle parts (everything except first and last)
        for part in &parts[1..parts.len() - 1] {
            if NON_ROUTE_SECONDARY_EXTS.contains(part) {
                return true;
            }
        }
    }
    false
}

/// Classify an App Router file and extract its route path.
fn classify_app_route(rest: &str) -> Option<(NextRouteKind, String)> {
    let (dir_part, file_name) = match rest.rsplit_once('/') {
        Some((d, f)) => (d, f),
        None => ("", rest),
    };

    if has_non_route_secondary_ext(file_name) {
        return None;
    }

    let stem = file_name.split('.').next()?;
    let ext = file_name.rsplit('.').next()?;

    if !ROUTE_EXTENSIONS.contains(&ext) {
        return None;
    }

    let kind = match stem {
        "page" => NextRouteKind::Page,
        "layout" => NextRouteKind::Layout,
        "loading" => NextRouteKind::Loading,
        "error" => NextRouteKind::Error,
        "template" => NextRouteKind::Template,
        "not-found" => NextRouteKind::NotFound,
        "route" => NextRouteKind::ApiRoute,
        _ => return None,
    };

    let route_path = dir_to_route_path(dir_part);

    // API routes get API: prefix
    let final_path = if kind == NextRouteKind::ApiRoute {
        format!("API:{}", route_path)
    } else {
        route_path
    };

    Some((kind, final_path))
}

/// Classify a Pages Router file and extract its route path.
fn classify_pages_route(rest: &str) -> Option<(NextRouteKind, String)> {
    let file_name = rest.rsplit('/').next().unwrap_or(rest);

    if has_non_route_secondary_ext(file_name) {
        return None;
    }

    let ext = rest.rsplit('.').next()?;
    if !ROUTE_EXTENSIONS.contains(&ext) {
        return None;
    }

    // Skip _app, _document, _error (Next.js internal pages)
    let stem_full = &rest[..rest.len() - ext.len() - 1]; // strip .ext
    let file_stem = stem_full.rsplit('/').next().unwrap_or(stem_full);
    if file_stem.starts_with('_') {
        return None;
    }

    // API routes: pages/api/**/*
    if rest.starts_with("api/") {
        let route = format!("API:/{}", stem_full);
        return Some((NextRouteKind::ApiRoute, route));
    }

    // Regular page: strip /index suffix
    let route = if stem_full == "index" {
        "/".to_string()
    } else if let Some(prefix) = stem_full.strip_suffix("/index") {
        format!("/{}", prefix)
    } else {
        format!("/{}", stem_full)
    };

    Some((NextRouteKind::Page, route))
}

/// Convert a directory path to a route path.
/// Strips dynamic route brackets for display: `[id]` stays as `[id]`.
fn dir_to_route_path(dir: &str) -> String {
    if dir.is_empty() {
        "/".to_string()
    } else {
        format!("/{}", dir)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_files(paths: &[&str]) -> FileSet {
        FileSet::from_iter(paths.iter().map(|p| CanonicalPath::new(p.to_string())))
    }

    fn discover(paths: &[&str]) -> Option<NextRouteInfo> {
        let files = make_files(paths);
        let diag = DiagnosticCollector::new();
        discover_next_routes(Path::new("."), &files, &diag)
    }

    // --- SC-19: App Router page ---

    #[test]
    fn app_router_page() {
        let info = discover(&["app/dashboard/page.tsx"]).unwrap();
        assert_eq!(info.router_type, NextRouterType::AppRouter);
        assert_eq!(info.routes.len(), 1);
        assert_eq!(info.routes[0].path, "/dashboard");
        assert_eq!(info.routes[0].kind, NextRouteKind::Page);
    }

    // --- SC-20: App Router API route ---

    #[test]
    fn app_router_api_route() {
        let info = discover(&["app/api/users/route.ts"]).unwrap();
        assert_eq!(info.routes.len(), 1);
        assert_eq!(info.routes[0].path, "API:/api/users");
        assert_eq!(info.routes[0].kind, NextRouteKind::ApiRoute);
    }

    // --- SC-21: App Router layout ---

    #[test]
    fn app_router_layout() {
        let info = discover(&["app/dashboard/layout.tsx"]).unwrap();
        assert_eq!(info.routes[0].path, "/dashboard");
        assert_eq!(info.routes[0].kind, NextRouteKind::Layout);
    }

    // --- SC-22: Middleware ---

    #[test]
    fn middleware_at_root() {
        let info = discover(&["middleware.ts"]).unwrap();
        assert_eq!(info.routes.len(), 1);
        assert_eq!(info.routes[0].kind, NextRouteKind::Middleware);
    }

    // --- SC-23: Pages Router ---

    #[test]
    fn pages_router_page() {
        let info = discover(&["pages/about.tsx"]).unwrap();
        assert_eq!(info.router_type, NextRouterType::PagesRouter);
        assert_eq!(info.routes.len(), 1);
        assert_eq!(info.routes[0].path, "/about");
        assert_eq!(info.routes[0].kind, NextRouteKind::Page);
    }

    #[test]
    fn pages_router_index() {
        let info = discover(&["pages/index.tsx"]).unwrap();
        assert_eq!(info.routes[0].path, "/");
    }

    #[test]
    fn pages_router_nested_index() {
        let info = discover(&["pages/blog/index.tsx"]).unwrap();
        assert_eq!(info.routes[0].path, "/blog");
    }

    #[test]
    fn pages_router_api() {
        let info = discover(&["pages/api/users.ts"]).unwrap();
        assert_eq!(info.routes[0].path, "API:/api/users");
        assert_eq!(info.routes[0].kind, NextRouteKind::ApiRoute);
    }

    #[test]
    fn pages_router_skips_internal() {
        let info = discover(&["pages/_app.tsx", "pages/_document.tsx", "pages/index.tsx"]);
        let info = info.unwrap();
        // Only index should be discovered, _app and _document are internal
        assert_eq!(info.routes.len(), 1);
        assert_eq!(info.routes[0].path, "/");
    }

    // --- SC-24: Both routers ---

    #[test]
    fn both_routers_detected() {
        let info = discover(&[
            "app/dashboard/page.tsx",
            "pages/about.tsx",
        ])
        .unwrap();
        assert_eq!(info.router_type, NextRouterType::Both);
        assert_eq!(info.routes.len(), 2);
    }

    // --- App Router convention files ---

    #[test]
    fn app_router_loading() {
        let info = discover(&["app/dashboard/loading.tsx"]).unwrap();
        assert_eq!(info.routes[0].kind, NextRouteKind::Loading);
    }

    #[test]
    fn app_router_error() {
        let info = discover(&["app/dashboard/error.tsx"]).unwrap();
        assert_eq!(info.routes[0].kind, NextRouteKind::Error);
    }

    #[test]
    fn app_router_template() {
        let info = discover(&["app/dashboard/template.tsx"]).unwrap();
        assert_eq!(info.routes[0].kind, NextRouteKind::Template);
    }

    #[test]
    fn app_router_not_found() {
        let info = discover(&["app/not-found.tsx"]).unwrap();
        assert_eq!(info.routes[0].kind, NextRouteKind::NotFound);
    }

    // --- Root page ---

    #[test]
    fn app_router_root_page() {
        let info = discover(&["app/page.tsx"]).unwrap();
        assert_eq!(info.routes[0].path, "/");
        assert_eq!(info.routes[0].kind, NextRouteKind::Page);
    }

    // --- Dynamic routes ---

    #[test]
    fn app_router_dynamic_route() {
        let info = discover(&["app/blog/[slug]/page.tsx"]).unwrap();
        assert_eq!(info.routes[0].path, "/blog/[slug]");
    }

    // --- No routes ---

    #[test]
    fn no_next_routes_returns_none() {
        assert!(discover(&["src/utils.ts"]).is_none());
    }

    // --- Non-route files in app/ ignored ---

    #[test]
    fn non_convention_file_in_app_ignored() {
        let info = discover(&["app/dashboard/utils.ts"]);
        assert!(info.is_none());
    }

    #[test]
    fn styles_file_in_pages_not_a_route() {
        let info = discover(&[
            "pages/Login/Login.tsx",
            "pages/Login/Login.styles.ts",
        ])
        .unwrap();
        assert_eq!(info.routes.len(), 1);
        assert_eq!(info.routes[0].file.as_str(), "pages/Login/Login.tsx");
    }

    #[test]
    fn test_file_in_app_not_a_route() {
        let info = discover(&[
            "app/dashboard/page.tsx",
            "app/dashboard/page.test.tsx",
        ])
        .unwrap();
        assert_eq!(info.routes.len(), 1);
        assert_eq!(info.routes[0].file.as_str(), "app/dashboard/page.tsx");
    }

    // --- Deterministic route order ---

    #[test]
    fn routes_sorted_by_file_path() {
        let info = discover(&[
            "app/z/page.tsx",
            "app/a/page.tsx",
            "app/m/page.tsx",
        ])
        .unwrap();
        let files: Vec<&str> = info.routes.iter().map(|r| r.file.as_str()).collect();
        let mut sorted = files.clone();
        sorted.sort();
        assert_eq!(files, sorted);
    }
}
