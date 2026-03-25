use serde::{Deserialize, Serialize};

/// Target of an annotation — a file, cluster, or edge in the graph.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AnnotationTarget {
    File { path: String },
    Cluster { name: String },
    Edge { from: String, to: String },
}

/// A user-created annotation attached to a graph element.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Annotation {
    pub id: String,
    pub target: AnnotationTarget,
    pub label: String,
    pub note: Option<String>,
    pub created_at: String,
}

/// Collection of annotations.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct AnnotationStore(pub Vec<Annotation>);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn annotation_target_file_roundtrip() {
        let target = AnnotationTarget::File {
            path: "src/main.rs".to_string(),
        };
        let json = serde_json::to_string(&target).unwrap();
        let parsed: AnnotationTarget = serde_json::from_str(&json).unwrap();
        assert_eq!(target, parsed);
        assert!(json.contains(r#""type":"file""#));
    }

    #[test]
    fn annotation_target_cluster_roundtrip() {
        let target = AnnotationTarget::Cluster {
            name: "auth".to_string(),
        };
        let json = serde_json::to_string(&target).unwrap();
        let parsed: AnnotationTarget = serde_json::from_str(&json).unwrap();
        assert_eq!(target, parsed);
        assert!(json.contains(r#""type":"cluster""#));
    }

    #[test]
    fn annotation_target_edge_roundtrip() {
        let target = AnnotationTarget::Edge {
            from: "a.ts".to_string(),
            to: "b.ts".to_string(),
        };
        let json = serde_json::to_string(&target).unwrap();
        let parsed: AnnotationTarget = serde_json::from_str(&json).unwrap();
        assert_eq!(target, parsed);
        assert!(json.contains(r#""type":"edge""#));
    }

    #[test]
    fn annotation_roundtrip() {
        let ann = Annotation {
            id: "ann-001".to_string(),
            target: AnnotationTarget::File {
                path: "src/lib.rs".to_string(),
            },
            label: "entry point".to_string(),
            note: Some("Main library entry".to_string()),
            created_at: "2026-03-25T00:00:00Z".to_string(),
        };
        let json = serde_json::to_string_pretty(&ann).unwrap();
        let parsed: Annotation = serde_json::from_str(&json).unwrap();
        assert_eq!(ann, parsed);
    }

    #[test]
    fn annotation_store_default_empty() {
        let store = AnnotationStore::default();
        assert_eq!(store.0.len(), 0);
    }

    #[test]
    fn annotation_store_roundtrip() {
        let store = AnnotationStore(vec![Annotation {
            id: "ann-001".to_string(),
            target: AnnotationTarget::Cluster {
                name: "core".to_string(),
            },
            label: "important".to_string(),
            note: None,
            created_at: "2026-03-25T00:00:00Z".to_string(),
        }]);
        let json = serde_json::to_string(&store).unwrap();
        let parsed: AnnotationStore = serde_json::from_str(&json).unwrap();
        assert_eq!(store, parsed);
    }
}
