//! UserState: aggregates annotations and bookmarks with persistence stores.
//!
//! Kept independent of GraphState per D-084. Uses ArcSwap for lock-free reads.

use std::path::Path;
use std::sync::Arc;

use arc_swap::ArcSwap;

use crate::mcp::persist::{JsonStore, JsonStoreError};
use crate::model::{AnnotationStore, BookmarkStore};

/// Snapshot of user-created annotations and bookmarks.
#[derive(Clone, Debug)]
pub struct UserState {
    pub annotations: AnnotationStore,
    pub bookmarks: BookmarkStore,
}

/// Manages user state with lock-free reads and atomic persistence.
///
/// The `state` field is an `ArcSwap` for concurrent reads from MCP handlers.
/// Mutations go through the manager, which updates both memory and disk atomically.
pub struct UserStateManager {
    /// Current user state, readable from any thread without locking.
    pub state: Arc<ArcSwap<UserState>>,
    /// Persistence store for annotations.
    pub annotation_store: JsonStore<AnnotationStore>,
    /// Persistence store for bookmarks.
    pub bookmark_store: JsonStore<BookmarkStore>,
}

impl std::fmt::Debug for UserStateManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UserStateManager")
            .field("state", &"<ArcSwap<UserState>>")
            .finish()
    }
}

impl UserStateManager {
    /// Load user state from disk, or create defaults if files do not exist.
    ///
    /// `output_dir` is the `.ariadne/` directory (NOT `.ariadne/graph/`).
    /// Annotations are stored at `{output_dir}/annotations.json`.
    /// Bookmarks are stored at `{output_dir}/bookmarks.json`.
    pub fn load(output_dir: &Path) -> Result<Self, JsonStoreError> {
        let annotation_store = JsonStore::new(output_dir.join("annotations.json"));
        let bookmark_store = JsonStore::new(output_dir.join("bookmarks.json"));
        let annotations = annotation_store.load()?;
        let bookmarks = bookmark_store.load()?;
        let state = Arc::new(ArcSwap::from_pointee(UserState {
            annotations,
            bookmarks,
        }));
        Ok(Self {
            state,
            annotation_store,
            bookmark_store,
        })
    }

    /// Persist current annotations to disk.
    pub fn save_annotations(&self) -> Result<(), JsonStoreError> {
        let guard = self.state.load();
        self.annotation_store.save(&guard.annotations)
    }

    /// Persist current bookmarks to disk.
    pub fn save_bookmarks(&self) -> Result<(), JsonStoreError> {
        let guard = self.state.load();
        self.bookmark_store.save(&guard.bookmarks)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_creates_defaults_when_no_files() {
        let dir = tempfile::tempdir().unwrap();
        let mgr = UserStateManager::load(dir.path()).unwrap();
        let state = mgr.state.load();
        assert_eq!(state.annotations.0.len(), 0);
        assert_eq!(state.bookmarks.0.len(), 0);
    }

    #[test]
    fn save_and_reload_annotations() {
        let dir = tempfile::tempdir().unwrap();
        let mgr = UserStateManager::load(dir.path()).unwrap();

        // Mutate in-memory state
        let mut snapshot = (**mgr.state.load()).clone();
        snapshot.annotations.0.push(crate::model::Annotation {
            id: "ann-001".to_string(),
            target: crate::model::AnnotationTarget::File {
                path: "src/main.rs".to_string(),
            },
            label: "entry".to_string(),
            note: None,
            created_at: "2026-03-25T00:00:00Z".to_string(),
        });
        mgr.state.store(Arc::new(snapshot));
        mgr.save_annotations().unwrap();

        // Reload from disk
        let mgr2 = UserStateManager::load(dir.path()).unwrap();
        let state2 = mgr2.state.load();
        assert_eq!(state2.annotations.0.len(), 1);
        assert_eq!(state2.annotations.0[0].id, "ann-001");
    }

    #[test]
    fn save_and_reload_bookmarks() {
        let dir = tempfile::tempdir().unwrap();
        let mgr = UserStateManager::load(dir.path()).unwrap();

        let mut snapshot = (**mgr.state.load()).clone();
        snapshot.bookmarks.0.push(crate::model::Bookmark {
            name: "auth-flow".to_string(),
            paths: vec!["src/auth.rs".to_string()],
            description: None,
            created_at: "2026-03-25T00:00:00Z".to_string(),
        });
        mgr.state.store(Arc::new(snapshot));
        mgr.save_bookmarks().unwrap();

        let mgr2 = UserStateManager::load(dir.path()).unwrap();
        let state2 = mgr2.state.load();
        assert_eq!(state2.bookmarks.0.len(), 1);
        assert_eq!(state2.bookmarks.0[0].name, "auth-flow");
    }
}
