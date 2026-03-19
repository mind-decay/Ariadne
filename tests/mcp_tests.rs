#[cfg(feature = "serve")]
mod lock_tests {
    use ariadne_graph::mcp::lock::{acquire_lock, check_lock, release_lock};
    use tempfile::tempdir;

    #[test]
    fn test_acquire_and_release_lock() {
        let dir = tempdir().unwrap();
        let lock_path = dir.path().join(".lock");

        acquire_lock(&lock_path).unwrap();
        assert!(lock_path.exists());

        let status = check_lock(&lock_path).unwrap();
        assert!(status.is_held_by_us());

        release_lock(&lock_path).unwrap();
        assert!(!lock_path.exists());
    }

    #[test]
    fn test_check_lock_no_file() {
        let dir = tempdir().unwrap();
        let lock_path = dir.path().join(".lock");

        let status = check_lock(&lock_path).unwrap();
        assert!(status.is_free());
    }

    #[test]
    fn test_stale_lock_detection() {
        let dir = tempdir().unwrap();
        let lock_path = dir.path().join(".lock");

        // Write a lock with a fake PID that doesn't exist
        let content = serde_json::json!({
            "pid": 999999999u32,
            "started_at": "2026-01-01T00:00:00Z"
        });
        std::fs::write(&lock_path, serde_json::to_string(&content).unwrap()).unwrap();

        let status = check_lock(&lock_path).unwrap();
        assert!(status.is_stale());
    }

    #[test]
    fn test_acquire_removes_stale_lock() {
        let dir = tempdir().unwrap();
        let lock_path = dir.path().join(".lock");

        // Write a stale lock
        let content = serde_json::json!({
            "pid": 999999999u32,
            "started_at": "2026-01-01T00:00:00Z"
        });
        std::fs::write(&lock_path, serde_json::to_string(&content).unwrap()).unwrap();

        // Acquire should succeed (stale lock gets replaced)
        acquire_lock(&lock_path).unwrap();
        let status = check_lock(&lock_path).unwrap();
        assert!(status.is_held_by_us());

        release_lock(&lock_path).unwrap();
    }

    #[test]
    fn test_double_acquire_is_idempotent() {
        let dir = tempdir().unwrap();
        let lock_path = dir.path().join(".lock");

        acquire_lock(&lock_path).unwrap();
        // Second acquire by same process should be fine
        acquire_lock(&lock_path).unwrap();

        let status = check_lock(&lock_path).unwrap();
        assert!(status.is_held_by_us());

        release_lock(&lock_path).unwrap();
    }

    #[test]
    fn test_release_nonexistent_is_ok() {
        let dir = tempdir().unwrap();
        let lock_path = dir.path().join(".lock");

        // Should not error
        release_lock(&lock_path).unwrap();
    }
}
