//! Annotation CRUD handlers for the MCP server.
//!
//! These functions operate on `UserStateManager` following the ArcSwap
//! load-clone-modify-persist-store pattern (D-084).

use std::sync::Arc;

use serde_json::json;

use crate::mcp::user_state::UserStateManager;
use crate::model::{Annotation, AnnotationStore, AnnotationTarget};

/// Format current time as ISO 8601 UTC string (`YYYY-MM-DDTHH:MM:SSZ`).
fn now_iso8601() -> String {
    let dur = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = dur.as_secs();

    // Compute date/time components from unix timestamp
    let days = secs / 86400;
    let time_of_day = secs % 86400;
    let hours = time_of_day / 3600;
    let minutes = (time_of_day % 3600) / 60;
    let seconds = time_of_day % 60;

    // Days since 1970-01-01 to Y-M-D (civil calendar)
    // Algorithm from Howard Hinnant's date library (public domain)
    let z = days as i64 + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u64; // day of era [0, 146096]
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

/// Extract the numeric suffix from an annotation id like "ann-42".
fn parse_ann_id(id: &str) -> Option<u64> {
    id.strip_prefix("ann-").and_then(|n| n.parse::<u64>().ok())
}

/// Generate the next annotation id based on existing annotations.
fn next_ann_id(store: &AnnotationStore) -> String {
    let max = store
        .0
        .iter()
        .filter_map(|a| parse_ann_id(&a.id))
        .max()
        .unwrap_or(0);
    format!("ann-{}", max + 1)
}

/// Add or update an annotation.
///
/// Upsert semantics: if an annotation with the same `target` and `label` exists,
/// update its `note` and `created_at` fields; otherwise create a new one.
pub fn annotate(
    manager: &UserStateManager,
    target: AnnotationTarget,
    label: String,
    note: Option<String>,
) -> Result<serde_json::Value, String> {
    if label.trim().is_empty() {
        return Err("label must not be empty".to_string());
    }

    let mut snapshot = (**manager.state.load()).clone();
    let now = now_iso8601();

    // Check for existing annotation with same target+label
    let existing = snapshot
        .annotations
        .0
        .iter()
        .position(|a| a.target == target && a.label == label);

    let (status, annotation) = if let Some(idx) = existing {
        // Update existing
        snapshot.annotations.0[idx].note = note;
        snapshot.annotations.0[idx].created_at = now;
        ("updated", snapshot.annotations.0[idx].clone())
    } else {
        // Create new
        let id = next_ann_id(&snapshot.annotations);
        let ann = Annotation {
            id,
            target,
            label,
            note,
            created_at: now,
        };
        snapshot.annotations.0.push(ann.clone());
        ("created", ann)
    };

    manager
        .annotation_store
        .save(&snapshot.annotations)
        .map_err(|e| format!("failed to persist annotations: {e}"))?;
    manager.state.store(Arc::new(snapshot));

    Ok(json!({
        "status": status,
        "annotation": annotation,
    }))
}

/// List annotations with optional filters.
///
/// Expired annotations (where `expires_at < now`) are excluded. Since the current
/// `Annotation` model does not have an `expires_at` field, this is a no-op for now
/// but the filtering structure is in place.
pub fn list_annotations(
    manager: &UserStateManager,
    tag: Option<String>,
    target_type: Option<String>,
    target_path: Option<String>,
) -> serde_json::Value {
    let state = manager.state.load();
    let filtered: Vec<&Annotation> = state
        .annotations
        .0
        .iter()
        .filter(|a| {
            // Filter by label
            if let Some(ref t) = tag {
                if a.label != *t {
                    return false;
                }
            }
            // Filter by target type
            if let Some(ref tt) = target_type {
                let actual_type = match &a.target {
                    AnnotationTarget::File { .. } => "file",
                    AnnotationTarget::Cluster { .. } => "cluster",
                    AnnotationTarget::Edge { .. } => "edge",
                };
                if actual_type != tt.as_str() {
                    return false;
                }
            }
            // Filter by target path/name
            if let Some(ref tp) = target_path {
                let matches = match &a.target {
                    AnnotationTarget::File { path } => path.contains(tp.as_str()),
                    AnnotationTarget::Cluster { name } => name.contains(tp.as_str()),
                    AnnotationTarget::Edge { from, to } => {
                        from.contains(tp.as_str()) || to.contains(tp.as_str())
                    }
                };
                if !matches {
                    return false;
                }
            }
            true
        })
        .collect();

    json!({
        "count": filtered.len(),
        "annotations": filtered,
    })
}

/// Remove an annotation by id.
pub fn remove_annotation(
    manager: &UserStateManager,
    id: String,
) -> Result<serde_json::Value, String> {
    if id.trim().is_empty() {
        return Err("id must not be empty".to_string());
    }

    let mut snapshot = (**manager.state.load()).clone();
    let original_len = snapshot.annotations.0.len();
    snapshot.annotations.0.retain(|a| a.id != id);

    if snapshot.annotations.0.len() == original_len {
        return Ok(json!({ "status": "not_found", "id": id }));
    }

    manager
        .annotation_store
        .save(&snapshot.annotations)
        .map_err(|e| format!("failed to persist annotations: {e}"))?;
    manager.state.store(Arc::new(snapshot));

    Ok(json!({ "status": "removed", "id": id }))
}

/// Get all annotations targeting a specific file path.
pub fn annotations_for_file(manager: &UserStateManager, path: &str) -> Vec<Annotation> {
    let state = manager.state.load();
    state
        .annotations
        .0
        .iter()
        .filter(|a| matches!(&a.target, AnnotationTarget::File { path: p } if p == path))
        .cloned()
        .collect()
}

/// Get all annotations targeting a specific cluster name.
pub fn annotations_for_cluster(manager: &UserStateManager, name: &str) -> Vec<Annotation> {
    let state = manager.state.load();
    state
        .annotations
        .0
        .iter()
        .filter(|a| matches!(&a.target, AnnotationTarget::Cluster { name: n } if n == name))
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::user_state::UserStateManager;
    use crate::model::AnnotationTarget;

    fn make_manager() -> (tempfile::TempDir, UserStateManager) {
        let dir = tempfile::tempdir().unwrap();
        let mgr = UserStateManager::load(dir.path()).unwrap();
        (dir, mgr)
    }

    #[test]
    fn annotate_creates_new() {
        let (_dir, mgr) = make_manager();
        let result = annotate(
            &mgr,
            AnnotationTarget::File {
                path: "src/main.rs".to_string(),
            },
            "entry".to_string(),
            Some("Main entry point".to_string()),
        )
        .unwrap();

        assert_eq!(result["status"], "created");
        assert_eq!(result["annotation"]["id"], "ann-1");
        assert_eq!(result["annotation"]["label"], "entry");

        // Verify in-memory state
        let state = mgr.state.load();
        assert_eq!(state.annotations.0.len(), 1);
    }

    #[test]
    fn annotate_upserts_on_same_target_and_label() {
        let (_dir, mgr) = make_manager();
        let target = AnnotationTarget::File {
            path: "src/main.rs".to_string(),
        };

        annotate(&mgr, target.clone(), "entry".to_string(), Some("v1".to_string())).unwrap();
        let result = annotate(&mgr, target, "entry".to_string(), Some("v2".to_string())).unwrap();

        assert_eq!(result["status"], "updated");
        assert_eq!(result["annotation"]["note"], "v2");

        // Still only one annotation
        let state = mgr.state.load();
        assert_eq!(state.annotations.0.len(), 1);
        assert_eq!(state.annotations.0[0].id, "ann-1");
    }

    #[test]
    fn annotate_different_label_creates_new() {
        let (_dir, mgr) = make_manager();
        let target = AnnotationTarget::File {
            path: "src/main.rs".to_string(),
        };

        annotate(&mgr, target.clone(), "entry".to_string(), None).unwrap();
        annotate(&mgr, target, "important".to_string(), None).unwrap();

        let state = mgr.state.load();
        assert_eq!(state.annotations.0.len(), 2);
    }

    #[test]
    fn annotate_rejects_empty_label() {
        let (_dir, mgr) = make_manager();
        let result = annotate(
            &mgr,
            AnnotationTarget::File {
                path: "src/main.rs".to_string(),
            },
            "  ".to_string(),
            None,
        );
        assert!(result.is_err());
    }

    #[test]
    fn annotate_id_increments() {
        let (_dir, mgr) = make_manager();
        annotate(
            &mgr,
            AnnotationTarget::File {
                path: "a.rs".to_string(),
            },
            "first".to_string(),
            None,
        )
        .unwrap();
        let result = annotate(
            &mgr,
            AnnotationTarget::File {
                path: "b.rs".to_string(),
            },
            "second".to_string(),
            None,
        )
        .unwrap();

        assert_eq!(result["annotation"]["id"], "ann-2");
    }

    #[test]
    fn list_annotations_empty_store() {
        let (_dir, mgr) = make_manager();
        let result = list_annotations(&mgr, None, None, None);
        assert_eq!(result["count"], 0);
        assert_eq!(result["annotations"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn list_annotations_returns_all() {
        let (_dir, mgr) = make_manager();
        annotate(
            &mgr,
            AnnotationTarget::File {
                path: "a.rs".to_string(),
            },
            "tag1".to_string(),
            None,
        )
        .unwrap();
        annotate(
            &mgr,
            AnnotationTarget::Cluster {
                name: "core".to_string(),
            },
            "tag2".to_string(),
            None,
        )
        .unwrap();

        let result = list_annotations(&mgr, None, None, None);
        assert_eq!(result["count"], 2);
    }

    #[test]
    fn list_annotations_filters_by_tag() {
        let (_dir, mgr) = make_manager();
        annotate(
            &mgr,
            AnnotationTarget::File {
                path: "a.rs".to_string(),
            },
            "important".to_string(),
            None,
        )
        .unwrap();
        annotate(
            &mgr,
            AnnotationTarget::File {
                path: "b.rs".to_string(),
            },
            "trivial".to_string(),
            None,
        )
        .unwrap();

        let result = list_annotations(&mgr, Some("important".to_string()), None, None);
        assert_eq!(result["count"], 1);
        assert_eq!(result["annotations"][0]["label"], "important");
    }

    #[test]
    fn list_annotations_filters_by_target_type() {
        let (_dir, mgr) = make_manager();
        annotate(
            &mgr,
            AnnotationTarget::File {
                path: "a.rs".to_string(),
            },
            "tag".to_string(),
            None,
        )
        .unwrap();
        annotate(
            &mgr,
            AnnotationTarget::Cluster {
                name: "core".to_string(),
            },
            "tag".to_string(),
            None,
        )
        .unwrap();

        let result = list_annotations(&mgr, None, Some("cluster".to_string()), None);
        assert_eq!(result["count"], 1);
    }

    #[test]
    fn list_annotations_filters_by_path() {
        let (_dir, mgr) = make_manager();
        annotate(
            &mgr,
            AnnotationTarget::File {
                path: "src/auth/login.rs".to_string(),
            },
            "tag".to_string(),
            None,
        )
        .unwrap();
        annotate(
            &mgr,
            AnnotationTarget::File {
                path: "src/utils/helper.rs".to_string(),
            },
            "tag".to_string(),
            None,
        )
        .unwrap();

        let result = list_annotations(&mgr, None, None, Some("auth".to_string()));
        assert_eq!(result["count"], 1);
    }

    #[test]
    fn remove_annotation_by_id() {
        let (_dir, mgr) = make_manager();
        annotate(
            &mgr,
            AnnotationTarget::File {
                path: "a.rs".to_string(),
            },
            "tag".to_string(),
            None,
        )
        .unwrap();

        let result = remove_annotation(&mgr, "ann-1".to_string()).unwrap();
        assert_eq!(result["status"], "removed");

        let state = mgr.state.load();
        assert_eq!(state.annotations.0.len(), 0);
    }

    #[test]
    fn remove_annotation_not_found() {
        let (_dir, mgr) = make_manager();
        let result = remove_annotation(&mgr, "ann-999".to_string()).unwrap();
        assert_eq!(result["status"], "not_found");
    }

    #[test]
    fn remove_annotation_rejects_empty_id() {
        let (_dir, mgr) = make_manager();
        let result = remove_annotation(&mgr, "  ".to_string());
        assert!(result.is_err());
    }

    #[test]
    fn annotations_for_file_filters_correctly() {
        let (_dir, mgr) = make_manager();
        annotate(
            &mgr,
            AnnotationTarget::File {
                path: "src/main.rs".to_string(),
            },
            "entry".to_string(),
            None,
        )
        .unwrap();
        annotate(
            &mgr,
            AnnotationTarget::File {
                path: "src/lib.rs".to_string(),
            },
            "lib".to_string(),
            None,
        )
        .unwrap();
        annotate(
            &mgr,
            AnnotationTarget::Cluster {
                name: "core".to_string(),
            },
            "core".to_string(),
            None,
        )
        .unwrap();

        let result = annotations_for_file(&mgr, "src/main.rs");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].label, "entry");
    }

    #[test]
    fn annotations_for_cluster_filters_correctly() {
        let (_dir, mgr) = make_manager();
        annotate(
            &mgr,
            AnnotationTarget::File {
                path: "src/main.rs".to_string(),
            },
            "entry".to_string(),
            None,
        )
        .unwrap();
        annotate(
            &mgr,
            AnnotationTarget::Cluster {
                name: "auth".to_string(),
            },
            "security".to_string(),
            None,
        )
        .unwrap();

        let result = annotations_for_cluster(&mgr, "auth");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].label, "security");
    }

    #[test]
    fn persists_to_disk_and_survives_reload() {
        let dir = tempfile::tempdir().unwrap();
        let mgr = UserStateManager::load(dir.path()).unwrap();
        annotate(
            &mgr,
            AnnotationTarget::File {
                path: "a.rs".to_string(),
            },
            "tag".to_string(),
            Some("note".to_string()),
        )
        .unwrap();

        // Reload from disk
        let mgr2 = UserStateManager::load(dir.path()).unwrap();
        let state = mgr2.state.load();
        assert_eq!(state.annotations.0.len(), 1);
        assert_eq!(state.annotations.0[0].label, "tag");
        assert_eq!(state.annotations.0[0].note, Some("note".to_string()));
    }

    #[test]
    fn now_iso8601_format() {
        let ts = now_iso8601();
        // Should match YYYY-MM-DDTHH:MM:SSZ
        assert_eq!(ts.len(), 20);
        assert!(ts.ends_with('Z'));
        assert_eq!(&ts[4..5], "-");
        assert_eq!(&ts[7..8], "-");
        assert_eq!(&ts[10..11], "T");
        assert_eq!(&ts[13..14], ":");
        assert_eq!(&ts[16..17], ":");
    }
}
