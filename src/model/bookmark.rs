use serde::{Deserialize, Serialize};

/// A named collection of file paths for quick navigation.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Bookmark {
    pub name: String,
    pub paths: Vec<String>,
    pub description: Option<String>,
    pub created_at: String,
}

/// Collection of bookmarks.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct BookmarkStore(pub Vec<Bookmark>);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bookmark_roundtrip() {
        let bm = Bookmark {
            name: "auth-flow".to_string(),
            paths: vec!["src/auth/login.ts".to_string(), "src/auth/logout.ts".to_string()],
            description: Some("Authentication flow files".to_string()),
            created_at: "2026-03-25T00:00:00Z".to_string(),
        };
        let json = serde_json::to_string_pretty(&bm).unwrap();
        let parsed: Bookmark = serde_json::from_str(&json).unwrap();
        assert_eq!(bm, parsed);
    }

    #[test]
    fn bookmark_without_description() {
        let bm = Bookmark {
            name: "quick".to_string(),
            paths: vec!["a.ts".to_string()],
            description: None,
            created_at: "2026-03-25T00:00:00Z".to_string(),
        };
        let json = serde_json::to_string(&bm).unwrap();
        assert!(!json.contains("null") || json.contains(r#""description":null"#));
        let parsed: Bookmark = serde_json::from_str(&json).unwrap();
        assert_eq!(bm, parsed);
    }

    #[test]
    fn bookmark_store_default_empty() {
        let store = BookmarkStore::default();
        assert_eq!(store.0.len(), 0);
    }

    #[test]
    fn bookmark_store_roundtrip() {
        let store = BookmarkStore(vec![Bookmark {
            name: "core".to_string(),
            paths: vec!["src/lib.rs".to_string()],
            description: None,
            created_at: "2026-03-25T00:00:00Z".to_string(),
        }]);
        let json = serde_json::to_string(&store).unwrap();
        let parsed: BookmarkStore = serde_json::from_str(&json).unwrap();
        assert_eq!(store, parsed);
    }
}
