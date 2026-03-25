//! Integration tests for Phase 7 — Git Temporal Analysis.
//!
//! Tests the full `temporal::analyze()` pipeline against real git repositories
//! created in temporary directories.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::process::Command;

use ariadne_graph::diagnostic::DiagnosticCollector;
use ariadne_graph::model::{
    CanonicalPath, ContentHash, Edge, EdgeType, Node, ProjectGraph,
    ArchLayer, ClusterId, FileType,
};
use ariadne_graph::temporal;

use tempfile::TempDir;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Create a test git repository with scripted commits.
/// Returns the TempDir (must be kept alive for the repo to exist).
fn create_test_repo() -> TempDir {
    let dir = TempDir::new().unwrap();
    let p = dir.path();

    git(p, &["init"]);
    git(p, &["config", "user.email", "test@test.com"]);
    git(p, &["config", "user.name", "Test"]);

    // Create source files
    fs::create_dir_all(p.join("src")).unwrap();
    fs::write(p.join("src/main.rs"), "fn main() {}\n").unwrap();
    fs::write(p.join("src/lib.rs"), "pub mod util;\n").unwrap();
    fs::write(p.join("src/util.rs"), "pub fn helper() {}\n").unwrap();

    // Commit 1: initial
    git(p, &["add", "."]);
    commit_with_date(p, "Alice", "Initial commit", "2026-03-01T10:00:00+00:00");

    // Commit 2: modify main.rs and lib.rs together
    fs::write(p.join("src/main.rs"), "fn main() { println!(\"hello\"); }\n").unwrap();
    fs::write(p.join("src/lib.rs"), "pub mod util;\npub mod extra;\n").unwrap();
    git(p, &["add", "."]);
    commit_with_date(p, "Bob", "Add extra module", "2026-03-05T10:00:00+00:00");

    // Commit 3: modify main.rs and lib.rs again
    fs::write(
        p.join("src/main.rs"),
        "fn main() { println!(\"hello world\"); }\n",
    )
    .unwrap();
    fs::write(
        p.join("src/lib.rs"),
        "pub mod util;\npub mod extra;\npub mod more;\n",
    )
    .unwrap();
    git(p, &["add", "."]);
    commit_with_date(p, "Alice", "Extend modules", "2026-03-10T10:00:00+00:00");

    // Commit 4: modify main.rs and lib.rs again
    fs::write(
        p.join("src/main.rs"),
        "fn main() {\n    println!(\"hello world v2\");\n}\n",
    )
    .unwrap();
    fs::write(
        p.join("src/lib.rs"),
        "pub mod util;\npub mod extra;\npub mod more;\npub mod v2;\n",
    )
    .unwrap();
    git(p, &["add", "."]);
    commit_with_date(p, "Alice", "v2 modules", "2026-03-15T10:00:00+00:00");

    // Commit 5: modify main.rs only
    fs::write(
        p.join("src/main.rs"),
        "fn main() {\n    println!(\"final\");\n}\n",
    )
    .unwrap();
    git(p, &["add", "."]);
    commit_with_date(p, "Charlie", "Final main", "2026-03-20T10:00:00+00:00");

    // Commit 6: modify util.rs
    fs::write(
        p.join("src/util.rs"),
        "pub fn helper() { /* updated */ }\n",
    )
    .unwrap();
    git(p, &["add", "."]);
    commit_with_date(p, "Alice", "Update util", "2026-03-22T10:00:00+00:00");

    dir
}

/// Build a minimal ProjectGraph matching the test repo files.
fn build_test_graph(root: &Path) -> ProjectGraph {
    let mut nodes = BTreeMap::new();
    for file in &["src/main.rs", "src/lib.rs", "src/util.rs"] {
        let content = fs::read_to_string(root.join(file)).unwrap_or_default();
        let lines = content.lines().count() as u32;
        nodes.insert(
            CanonicalPath::new(*file),
            Node {
                file_type: FileType::Source,
                layer: ArchLayer::Unknown,
                fsd_layer: None,
                arch_depth: 0,
                lines,
                hash: ContentHash::new("0000000000000000".to_string()),
                exports: Vec::new(),
                cluster: ClusterId::new("src"),
                symbols: Vec::new(),
            },
        );
    }

    // Add an edge: main.rs -> lib.rs
    let edges = vec![Edge {
        from: CanonicalPath::new("src/main.rs"),
        to: CanonicalPath::new("src/lib.rs"),
        edge_type: EdgeType::Imports,
        symbols: Vec::new(),
    }];

    ProjectGraph { nodes, edges }
}

fn git(dir: &Path, args: &[&str]) {
    let output = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .expect("git command failed to execute");
    assert!(
        output.status.success(),
        "git {:?} failed: {}",
        args,
        String::from_utf8_lossy(&output.stderr)
    );
}

fn commit_with_date(dir: &Path, author: &str, message: &str, date: &str) {
    let output = Command::new("git")
        .args(["commit", "-m", message])
        .env("GIT_AUTHOR_NAME", author)
        .env("GIT_AUTHOR_EMAIL", &format!("{}@test.com", author.to_lowercase()))
        .env("GIT_AUTHOR_DATE", date)
        .env("GIT_COMMITTER_NAME", author)
        .env("GIT_COMMITTER_EMAIL", &format!("{}@test.com", author.to_lowercase()))
        .env("GIT_COMMITTER_DATE", date)
        .current_dir(dir)
        .output()
        .expect("git commit failed to execute");
    assert!(
        output.status.success(),
        "git commit failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

// ===========================================================================
// 1. Temporal analysis on a fixture git repo
// ===========================================================================

#[test]
fn temporal_analyze_on_fixture_repo() {
    let repo = create_test_repo();
    let graph = build_test_graph(repo.path());
    let collector = DiagnosticCollector::new();

    let result = temporal::analyze(repo.path(), &graph, &collector);
    assert!(result.is_some(), "analyze() should return Some for a valid git repo");

    let state = result.unwrap();

    // Should have analyzed commits
    assert!(
        state.commits_analyzed > 0,
        "commits_analyzed should be > 0, got {}",
        state.commits_analyzed
    );

    // Should have churn data for files that were committed
    assert!(
        !state.churn.is_empty(),
        "churn should not be empty"
    );

    // src/main.rs was modified in 5 of the 6 commits (initial + 4 modifications)
    let main_churn = state.churn.get(&CanonicalPath::new("src/main.rs"));
    assert!(
        main_churn.is_some(),
        "should have churn data for src/main.rs"
    );
    let main_metrics = main_churn.unwrap();
    assert!(
        main_metrics.commits_1y >= 4,
        "src/main.rs should have >= 4 commits in 1y, got {}",
        main_metrics.commits_1y
    );

    // Ownership should exist
    assert!(
        !state.ownership.is_empty(),
        "ownership should not be empty"
    );

    // Hotspots should exist
    assert!(
        !state.hotspots.is_empty(),
        "hotspots should not be empty"
    );

    // Window dates should be populated
    assert!(
        !state.window_start.is_empty(),
        "window_start should not be empty"
    );
    assert!(
        !state.window_end.is_empty(),
        "window_end should not be empty"
    );

    // Should not be a shallow clone
    assert!(!state.shallow, "test repo should not be shallow");
}

#[test]
fn temporal_analyze_churn_ownership_details() {
    let repo = create_test_repo();
    let graph = build_test_graph(repo.path());
    let collector = DiagnosticCollector::new();

    let state = temporal::analyze(repo.path(), &graph, &collector).unwrap();

    // Check ownership for src/main.rs — last author should be Charlie (commit 5)
    let main_ownership = state.ownership.get(&CanonicalPath::new("src/main.rs"));
    assert!(main_ownership.is_some(), "should have ownership for src/main.rs");
    let main_own = main_ownership.unwrap();
    assert_eq!(
        main_own.last_author, "Charlie",
        "last author of src/main.rs should be Charlie"
    );
    // Should have 3 distinct authors (Alice, Bob, Charlie)
    assert!(
        main_own.author_count >= 3,
        "src/main.rs should have >= 3 authors, got {}",
        main_own.author_count
    );

    // Check ownership for src/util.rs — last author should be Alice
    let util_ownership = state.ownership.get(&CanonicalPath::new("src/util.rs"));
    assert!(util_ownership.is_some(), "should have ownership for src/util.rs");
    assert_eq!(util_ownership.unwrap().last_author, "Alice");
}

// ===========================================================================
// 2. Graceful degradation tests
// ===========================================================================

#[test]
fn temporal_analyze_non_git_directory_returns_none() {
    let dir = TempDir::new().unwrap();
    // Create a file so the directory isn't empty, but no .git
    fs::write(dir.path().join("src.rs"), "fn main() {}\n").unwrap();

    let graph = ProjectGraph {
        nodes: BTreeMap::new(),
        edges: Vec::new(),
    };
    let collector = DiagnosticCollector::new();

    let result = temporal::analyze(dir.path(), &graph, &collector);
    assert!(
        result.is_none(),
        "analyze() should return None for a non-git directory"
    );

    // Verify a warning was emitted
    let report = collector.drain();
    assert!(
        !report.warnings.is_empty(),
        "should have at least one warning for non-git directory"
    );
}

#[test]
fn temporal_analyze_empty_git_repo() {
    let dir = TempDir::new().unwrap();
    git(dir.path(), &["init"]);
    git(dir.path(), &["config", "user.email", "test@test.com"]);
    git(dir.path(), &["config", "user.name", "Test"]);

    // git repo with no commits — git log will fail or return empty
    let graph = ProjectGraph {
        nodes: BTreeMap::new(),
        edges: Vec::new(),
    };
    let collector = DiagnosticCollector::new();

    let result = temporal::analyze(dir.path(), &graph, &collector);

    // Should either return None (git log fails on empty repo) or Some with empty data
    if let Some(state) = result {
        assert_eq!(state.commits_analyzed, 0);
        assert!(state.churn.is_empty());
        assert!(state.co_changes.is_empty());
    }
    // Both None and Some(empty) are acceptable graceful degradation
}

// ===========================================================================
// 3. Determinism test
// ===========================================================================

#[test]
fn temporal_analyze_deterministic() {
    let repo = create_test_repo();
    let graph = build_test_graph(repo.path());

    let collector1 = DiagnosticCollector::new();
    let result1 = temporal::analyze(repo.path(), &graph, &collector1).unwrap();
    let json1 = serde_json::to_string(&result1).unwrap();

    let collector2 = DiagnosticCollector::new();
    let result2 = temporal::analyze(repo.path(), &graph, &collector2).unwrap();
    let json2 = serde_json::to_string(&result2).unwrap();

    assert_eq!(
        json1, json2,
        "Two runs of analyze() on the same repo must produce byte-identical JSON output"
    );
}

// ===========================================================================
// 4. Invariant tests
// ===========================================================================

/// INV-T1: All Jaccard confidence values in [0.0, 1.0]
#[test]
fn invariant_confidence_in_range() {
    let repo = create_test_repo();
    let graph = build_test_graph(repo.path());
    let collector = DiagnosticCollector::new();

    let state = temporal::analyze(repo.path(), &graph, &collector).unwrap();

    for co_change in &state.co_changes {
        assert!(
            co_change.confidence >= 0.0 && co_change.confidence <= 1.0,
            "INV-T1 violated: confidence {} for ({}, {}) not in [0.0, 1.0]",
            co_change.confidence,
            co_change.file_a.as_str(),
            co_change.file_b.as_str(),
        );
    }
}

/// INV-T2: All co_change_count >= 3 (MIN_CO_CHANGE_COUNT)
#[test]
fn invariant_co_change_count_minimum() {
    let repo = create_test_repo();
    let graph = build_test_graph(repo.path());
    let collector = DiagnosticCollector::new();

    let state = temporal::analyze(repo.path(), &graph, &collector).unwrap();

    for co_change in &state.co_changes {
        assert!(
            co_change.co_change_count >= 3,
            "INV-T2 violated: co_change_count {} for ({}, {}) is below minimum 3",
            co_change.co_change_count,
            co_change.file_a.as_str(),
            co_change.file_b.as_str(),
        );
    }
}

/// INV-T3: All hotspot scores in [0.0, 1.0]
#[test]
fn invariant_hotspot_scores_in_range() {
    let repo = create_test_repo();
    let graph = build_test_graph(repo.path());
    let collector = DiagnosticCollector::new();

    let state = temporal::analyze(repo.path(), &graph, &collector).unwrap();

    for hotspot in &state.hotspots {
        assert!(
            hotspot.score >= 0.0 && hotspot.score <= 1.0,
            "INV-T3 violated: hotspot score {} for {} not in [0.0, 1.0]",
            hotspot.score,
            hotspot.path.as_str(),
        );
    }
}

/// INV-T4: Churn monotonic: commits_30d <= commits_90d <= commits_1y
#[test]
fn invariant_churn_monotonic() {
    let repo = create_test_repo();
    let graph = build_test_graph(repo.path());
    let collector = DiagnosticCollector::new();

    let state = temporal::analyze(repo.path(), &graph, &collector).unwrap();

    for (path, metrics) in &state.churn {
        assert!(
            metrics.commits_30d <= metrics.commits_90d,
            "INV-T4 violated: commits_30d ({}) > commits_90d ({}) for {}",
            metrics.commits_30d,
            metrics.commits_90d,
            path.as_str(),
        );
        assert!(
            metrics.commits_90d <= metrics.commits_1y,
            "INV-T4 violated: commits_90d ({}) > commits_1y ({}) for {}",
            metrics.commits_90d,
            metrics.commits_1y,
            path.as_str(),
        );
    }
}

/// INV-T4 extended: lines_changed_30d <= lines_changed_90d
#[test]
fn invariant_lines_changed_monotonic() {
    let repo = create_test_repo();
    let graph = build_test_graph(repo.path());
    let collector = DiagnosticCollector::new();

    let state = temporal::analyze(repo.path(), &graph, &collector).unwrap();

    for (path, metrics) in &state.churn {
        assert!(
            metrics.lines_changed_30d <= metrics.lines_changed_90d,
            "lines_changed_30d ({}) > lines_changed_90d ({}) for {}",
            metrics.lines_changed_30d,
            metrics.lines_changed_90d,
            path.as_str(),
        );
    }
}

/// Additional invariant: hotspots sorted by score descending
#[test]
fn invariant_hotspots_sorted_descending() {
    let repo = create_test_repo();
    let graph = build_test_graph(repo.path());
    let collector = DiagnosticCollector::new();

    let state = temporal::analyze(repo.path(), &graph, &collector).unwrap();

    for window in state.hotspots.windows(2) {
        assert!(
            window[0].score >= window[1].score,
            "Hotspots not sorted descending: {} ({}) < {} ({})",
            window[0].path.as_str(),
            window[0].score,
            window[1].path.as_str(),
            window[1].score,
        );
    }
}

/// Additional invariant: co_changes sorted by confidence descending
#[test]
fn invariant_co_changes_sorted_descending() {
    let repo = create_test_repo();
    let graph = build_test_graph(repo.path());
    let collector = DiagnosticCollector::new();

    let state = temporal::analyze(repo.path(), &graph, &collector).unwrap();

    for window in state.co_changes.windows(2) {
        assert!(
            window[0].confidence >= window[1].confidence,
            "Co-changes not sorted descending: ({}, {}) conf {} < ({}, {}) conf {}",
            window[0].file_a.as_str(),
            window[0].file_b.as_str(),
            window[0].confidence,
            window[1].file_a.as_str(),
            window[1].file_b.as_str(),
            window[1].confidence,
        );
    }
}

/// Additional invariant: ownership author_count > 0 for every file
#[test]
fn invariant_ownership_has_authors() {
    let repo = create_test_repo();
    let graph = build_test_graph(repo.path());
    let collector = DiagnosticCollector::new();

    let state = temporal::analyze(repo.path(), &graph, &collector).unwrap();

    for (path, info) in &state.ownership {
        assert!(
            info.author_count > 0,
            "ownership for {} has author_count 0",
            path.as_str(),
        );
        assert!(
            !info.last_author.is_empty(),
            "ownership for {} has empty last_author",
            path.as_str(),
        );
        assert!(
            !info.top_contributors.is_empty(),
            "ownership for {} has empty top_contributors",
            path.as_str(),
        );
    }
}
