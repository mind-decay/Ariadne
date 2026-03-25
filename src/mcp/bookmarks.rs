//! Bookmark CRUD handlers and path expansion for the MCP server.
//!
//! These functions operate on `UserStateManager` following the ArcSwap
//! load-clone-modify-persist-store pattern (D-084).
//! Path expansion is performed at query time per D-088.

use std::collections::BTreeSet;
use std::sync::Arc;

use serde_json::json;

use crate::mcp::user_state::UserStateManager;
use crate::model::{Bookmark, ProjectGraph};

/// Format current time as ISO 8601 UTC string (`YYYY-MM-DDTHH:MM:SSZ`).
fn now_iso8601() -> String {
    let dur = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = dur.as_secs();

    let days = secs / 86400;
    let time_of_day = secs % 86400;
    let hours = time_of_day / 3600;
    let minutes = (time_of_day % 3600) / 60;
    let seconds = time_of_day % 60;

    let z = days as i64 + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };

    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        y, m, d, hours, minutes, seconds
    )
}

/// Create or update a bookmark (upsert by name).
pub fn bookmark(
    manager: &UserStateManager,
    name: String,
    paths: Vec<String>,
    description: Option<String>,
) -> Result<serde_json::Value, String> {
    if name.trim().is_empty() {
        return Err("bookmark name must not be empty".to_string());
    }
    if paths.is_empty() {
        return Err("bookmark must contain at least one path".to_string());
    }

    let mut snapshot = (**manager.state.load()).clone();
    let now = now_iso8601();

    let existing = snapshot.bookmarks.0.iter().position(|b| b.name == name);

    let (status, bm) = if let Some(idx) = existing {
        snapshot.bookmarks.0[idx].paths = paths;
        snapshot.bookmarks.0[idx].description = description;
        snapshot.bookmarks.0[idx].created_at = now;
        ("updated", snapshot.bookmarks.0[idx].clone())
    } else {
        let bm = Bookmark {
            name,
            paths,
            description,
            created_at: now,
        };
        snapshot.bookmarks.0.push(bm.clone());
        ("created", bm)
    };

    manager
        .bookmark_store
        .save(&snapshot.bookmarks)
        .map_err(|e| format!("failed to persist bookmarks: {e}"))?;
    manager.state.store(Arc::new(snapshot));

    Ok(json!({
        "status": status,
        "bookmark": bm,
    }))
}

/// List all bookmarks.
pub fn list_bookmarks(manager: &UserStateManager) -> serde_json::Value {
    let state = manager.state.load();
    json!({
        "count": state.bookmarks.0.len(),
        "bookmarks": state.bookmarks.0,
    })
}

/// Remove a bookmark by name.
pub fn remove_bookmark(
    manager: &UserStateManager,
    name: String,
) -> Result<serde_json::Value, String> {
    if name.trim().is_empty() {
        return Err("bookmark name must not be empty".to_string());
    }

    let mut snapshot = (**manager.state.load()).clone();
    let original_len = snapshot.bookmarks.0.len();
    snapshot.bookmarks.0.retain(|b| b.name != name);

    if snapshot.bookmarks.0.len() == original_len {
        return Ok(json!({ "status": "not_found", "name": name }));
    }

    manager
        .bookmark_store
        .save(&snapshot.bookmarks)
        .map_err(|e| format!("failed to persist bookmarks: {e}"))?;
    manager.state.store(Arc::new(snapshot));

    Ok(json!({ "status": "removed", "name": name }))
}

/// Expand bookmark paths against the current project graph (D-088: query-time expansion).
///
/// For each raw path:
/// 1. If it matches a graph node key exactly, include it.
/// 2. If it ends with `/`, include all nodes whose path starts with this prefix.
/// 3. Otherwise, also try as a prefix match.
///
/// Results are deduplicated and sorted.
pub fn expand_paths(raw_paths: &[String], graph: &ProjectGraph) -> Vec<String> {
    let mut result = BTreeSet::new();

    for raw in raw_paths {
        // Check for exact match first
        let canonical = crate::model::CanonicalPath::new(raw.as_str());
        if graph.nodes.contains_key(&canonical) {
            result.insert(canonical.as_str().to_string());
            continue;
        }

        // Directory prefix expansion (explicit "/" or fallback prefix match)
        let prefix = if raw.ends_with('/') {
            raw.as_str()
        } else {
            // Try as prefix with trailing slash
            raw.as_str()
        };

        let prefix_with_slash = if prefix.ends_with('/') {
            prefix.to_string()
        } else {
            format!("{prefix}/")
        };

        let mut matched_any = false;
        for key in graph.nodes.keys() {
            if key.as_str().starts_with(&prefix_with_slash) {
                result.insert(key.as_str().to_string());
                matched_any = true;
            }
        }

        // If nothing matched as prefix either, include raw path as-is
        // (it may refer to something not yet in the graph)
        if !matched_any {
            result.insert(raw.clone());
        }
    }

    result.into_iter().collect()
}

/// Resolve a bookmark name to expanded paths against the current graph.
pub fn resolve_bookmark(
    manager: &UserStateManager,
    name: &str,
    graph: &ProjectGraph,
) -> Result<Vec<String>, String> {
    let state = manager.state.load();
    let bm = state
        .bookmarks
        .0
        .iter()
        .find(|b| b.name == name)
        .ok_or_else(|| format!("bookmark '{name}' not found"))?;

    Ok(expand_paths(&bm.paths, graph))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::user_state::UserStateManager;
    use crate::model::{CanonicalPath, ProjectGraph};
    use std::collections::BTreeMap;

    fn make_manager() -> (tempfile::TempDir, UserStateManager) {
        let dir = tempfile::tempdir().unwrap();
        let mgr = UserStateManager::load(dir.path()).unwrap();
        (dir, mgr)
    }

    fn make_graph(paths: &[&str]) -> ProjectGraph {
        let mut nodes = BTreeMap::new();
        for p in paths {
            let cp = CanonicalPath::new(*p);
            nodes.insert(
                cp,
                crate::model::Node {
                    file_type: crate::model::FileType::Source,
                    layer: crate::model::ArchLayer::Unknown,
                    fsd_layer: None,
                    arch_depth: 0,
                    lines: 10,
                    hash: crate::model::ContentHash::new("0000000000000000".to_string()),
                    exports: vec![],
                    cluster: crate::model::ClusterId::new("test"),
                    symbols: vec![],
                },
            );
        }
        ProjectGraph {
            nodes,
            edges: vec![],
        }
    }

    #[test]
    fn bookmark_creates_new() {
        let (_dir, mgr) = make_manager();
        let result = bookmark(
            &mgr,
            "auth".to_string(),
            vec!["src/auth/login.rs".to_string()],
            Some("Auth files".to_string()),
        )
        .unwrap();

        assert_eq!(result["status"], "created");
        assert_eq!(result["bookmark"]["name"], "auth");

        let state = mgr.state.load();
        assert_eq!(state.bookmarks.0.len(), 1);
    }

    #[test]
    fn bookmark_upserts_by_name() {
        let (_dir, mgr) = make_manager();
        bookmark(
            &mgr,
            "auth".to_string(),
            vec!["src/auth/login.rs".to_string()],
            None,
        )
        .unwrap();
        let result = bookmark(
            &mgr,
            "auth".to_string(),
            vec!["src/auth/login.rs".to_string(), "src/auth/logout.rs".to_string()],
            Some("Updated".to_string()),
        )
        .unwrap();

        assert_eq!(result["status"], "updated");

        let state = mgr.state.load();
        assert_eq!(state.bookmarks.0.len(), 1);
        assert_eq!(state.bookmarks.0[0].paths.len(), 2);
        assert_eq!(state.bookmarks.0[0].description, Some("Updated".to_string()));
    }

    #[test]
    fn bookmark_rejects_empty_name() {
        let (_dir, mgr) = make_manager();
        let result = bookmark(&mgr, "  ".to_string(), vec!["a.rs".to_string()], None);
        assert!(result.is_err());
    }

    #[test]
    fn bookmark_rejects_empty_paths() {
        let (_dir, mgr) = make_manager();
        let result = bookmark(&mgr, "test".to_string(), vec![], None);
        assert!(result.is_err());
    }

    #[test]
    fn list_bookmarks_returns_all() {
        let (_dir, mgr) = make_manager();
        bookmark(
            &mgr,
            "a".to_string(),
            vec!["a.rs".to_string()],
            None,
        )
        .unwrap();
        bookmark(
            &mgr,
            "b".to_string(),
            vec!["b.rs".to_string()],
            None,
        )
        .unwrap();

        let result = list_bookmarks(&mgr);
        assert_eq!(result["count"], 2);
    }

    #[test]
    fn remove_bookmark_by_name() {
        let (_dir, mgr) = make_manager();
        bookmark(
            &mgr,
            "auth".to_string(),
            vec!["a.rs".to_string()],
            None,
        )
        .unwrap();

        let result = remove_bookmark(&mgr, "auth".to_string()).unwrap();
        assert_eq!(result["status"], "removed");

        let state = mgr.state.load();
        assert_eq!(state.bookmarks.0.len(), 0);
    }

    #[test]
    fn remove_bookmark_not_found() {
        let (_dir, mgr) = make_manager();
        let result = remove_bookmark(&mgr, "nope".to_string()).unwrap();
        assert_eq!(result["status"], "not_found");
    }

    #[test]
    fn remove_bookmark_rejects_empty_name() {
        let (_dir, mgr) = make_manager();
        let result = remove_bookmark(&mgr, "  ".to_string());
        assert!(result.is_err());
    }

    #[test]
    fn expand_paths_exact_match() {
        let graph = make_graph(&["src/main.rs", "src/lib.rs"]);
        let result = expand_paths(&["src/main.rs".to_string()], &graph);
        assert_eq!(result, vec!["src/main.rs"]);
    }

    #[test]
    fn expand_paths_directory_prefix_with_slash() {
        let graph = make_graph(&[
            "src/auth/login.rs",
            "src/auth/logout.rs",
            "src/utils/helper.rs",
        ]);
        let result = expand_paths(&["src/auth/".to_string()], &graph);
        assert_eq!(result, vec!["src/auth/login.rs", "src/auth/logout.rs"]);
    }

    #[test]
    fn expand_paths_directory_prefix_without_slash() {
        let graph = make_graph(&[
            "src/auth/login.rs",
            "src/auth/logout.rs",
            "src/utils/helper.rs",
        ]);
        // "src/auth" is not an exact node, so try prefix expansion
        let result = expand_paths(&["src/auth".to_string()], &graph);
        assert_eq!(result, vec!["src/auth/login.rs", "src/auth/logout.rs"]);
    }

    #[test]
    fn expand_paths_deduplicates() {
        let graph = make_graph(&["src/main.rs"]);
        let result = expand_paths(
            &["src/main.rs".to_string(), "src/main.rs".to_string()],
            &graph,
        );
        assert_eq!(result, vec!["src/main.rs"]);
    }

    #[test]
    fn expand_paths_unknown_path_kept() {
        let graph = make_graph(&["src/main.rs"]);
        let result = expand_paths(&["unknown/file.rs".to_string()], &graph);
        assert_eq!(result, vec!["unknown/file.rs"]);
    }

    #[test]
    fn resolve_bookmark_expands_paths() {
        let (_dir, mgr) = make_manager();
        bookmark(
            &mgr,
            "auth".to_string(),
            vec!["src/auth/".to_string()],
            None,
        )
        .unwrap();

        let graph = make_graph(&[
            "src/auth/login.rs",
            "src/auth/logout.rs",
            "src/utils/helper.rs",
        ]);
        let result = resolve_bookmark(&mgr, "auth", &graph).unwrap();
        assert_eq!(result, vec!["src/auth/login.rs", "src/auth/logout.rs"]);
    }

    #[test]
    fn resolve_bookmark_not_found() {
        let (_dir, mgr) = make_manager();
        let graph = make_graph(&[]);
        let result = resolve_bookmark(&mgr, "nope", &graph);
        assert!(result.is_err());
    }

    #[test]
    fn persists_to_disk_and_survives_reload() {
        let dir = tempfile::tempdir().unwrap();
        let mgr = UserStateManager::load(dir.path()).unwrap();
        bookmark(
            &mgr,
            "auth".to_string(),
            vec!["src/auth/".to_string()],
            Some("Auth flow".to_string()),
        )
        .unwrap();

        let mgr2 = UserStateManager::load(dir.path()).unwrap();
        let state = mgr2.state.load();
        assert_eq!(state.bookmarks.0.len(), 1);
        assert_eq!(state.bookmarks.0[0].name, "auth");
        assert_eq!(state.bookmarks.0[0].description, Some("Auth flow".to_string()));
    }
}
