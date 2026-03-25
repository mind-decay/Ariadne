use std::collections::{BTreeMap, BTreeSet, HashMap};

use crate::model::temporal::ChurnMetrics;
use crate::model::CanonicalPath;

use super::git::CommitData;

/// Compute churn metrics for all files referenced in the given commits.
///
/// Metrics are computed in three time windows (30d, 90d, 1y) relative to `window_end`.
/// File paths are resolved via `rename_map`; paths not in the map are treated as
/// already-canonical.
pub(crate) fn compute_churn(
    commits: &[CommitData],
    rename_map: &BTreeMap<String, CanonicalPath>,
    window_end: &str, // ISO 8601 date or datetime
) -> BTreeMap<CanonicalPath, ChurnMetrics> {
    let end_days = match parse_date_to_days(window_end) {
        Some(d) => d,
        None => return BTreeMap::new(),
    };

    let boundary_30d = end_days - 30;
    let boundary_90d = end_days - 90;
    let boundary_1y = end_days - 365;

    // Accumulator per canonical path
    let mut acc: BTreeMap<CanonicalPath, FileAccum> = BTreeMap::new();

    for commit in commits {
        let commit_days = match parse_date_to_days(&commit.date) {
            Some(d) => d,
            None => continue,
        };

        // Skip commits outside the 1-year window
        if commit_days > end_days || commit_days < boundary_1y {
            continue;
        }

        for file in &commit.files {
            let canonical = resolve_path(&file.path, rename_map);
            let entry = acc.entry(canonical).or_default();

            let lines = file.additions + file.deletions;

            // 1-year window (always true if we reached here)
            entry.commits_1y += 1;

            // 90-day window
            if commit_days >= boundary_90d {
                entry.commits_90d += 1;
            }

            // 30-day window
            if commit_days >= boundary_30d {
                entry.commits_30d += 1;
                entry.lines_changed_30d += lines;
                entry.authors_30d.insert(commit.author.clone());
            }

            // 90-day lines
            if commit_days >= boundary_90d {
                entry.lines_changed_90d += lines;
            }

            // Track last_changed
            if entry.last_changed_days.is_none()
                || commit_days > entry.last_changed_days.unwrap()
            {
                entry.last_changed_days = Some(commit_days);
                entry.last_changed_date = Some(extract_date_prefix(&commit.date));
            }

            // Track author commit counts for top_authors
            *entry.author_commits.entry(commit.author.clone()).or_insert(0) += 1;
        }
    }

    // Convert accumulators to ChurnMetrics
    acc.into_iter()
        .map(|(path, a)| {
            let mut top_authors: Vec<(String, u32)> =
                a.author_commits.into_iter().collect();
            top_authors.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
            top_authors.truncate(5);

            let metrics = ChurnMetrics {
                commits_30d: a.commits_30d,
                commits_90d: a.commits_90d,
                commits_1y: a.commits_1y,
                lines_changed_30d: a.lines_changed_30d,
                lines_changed_90d: a.lines_changed_90d,
                authors_30d: a.authors_30d.len() as u32,
                last_changed: a.last_changed_date,
                top_authors,
            };
            (path, metrics)
        })
        .collect()
}

#[derive(Default)]
struct FileAccum {
    commits_30d: u32,
    commits_90d: u32,
    commits_1y: u32,
    lines_changed_30d: u32,
    lines_changed_90d: u32,
    authors_30d: BTreeSet<String>,
    last_changed_days: Option<i64>,
    last_changed_date: Option<String>,
    author_commits: HashMap<String, u32>,
}

/// Resolve a file path to its canonical path via the rename map.
/// If not found in the map, create a CanonicalPath directly.
fn resolve_path(
    path: &str,
    rename_map: &BTreeMap<String, CanonicalPath>,
) -> CanonicalPath {
    rename_map
        .get(path)
        .cloned()
        .unwrap_or_else(|| CanonicalPath::new(path))
}

/// Extract the date prefix (YYYY-MM-DD) from an ISO 8601 datetime string.
fn extract_date_prefix(date: &str) -> String {
    // Handles both "2026-03-20" and "2026-03-20T14:30:00+00:00"
    date.get(..10).unwrap_or(date).to_string()
}

/// Parse an ISO 8601 date/datetime string to an approximate day count since epoch.
///
/// Supports:
/// - `YYYY-MM-DD`
/// - `YYYY-MM-DDT...` (time portion is ignored)
///
/// Returns `None` if the format is unrecognizable.
fn parse_date_to_days(date: &str) -> Option<i64> {
    // Extract YYYY-MM-DD portion
    let date_part = if date.len() >= 10 && date.as_bytes().get(4) == Some(&b'-') {
        &date[..10]
    } else {
        return None;
    };

    let parts: Vec<&str> = date_part.split('-').collect();
    if parts.len() != 3 {
        return None;
    }

    let year: i64 = parts[0].parse().ok()?;
    let month: i64 = parts[1].parse().ok()?;
    let day: i64 = parts[2].parse().ok()?;

    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }

    // Approximate days since epoch using a simple formula.
    // Exact precision is not critical — we only need ~day accuracy for windowing.
    Some(year * 365 + year / 4 - year / 100 + year / 400 + month * 30 + day)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::temporal::git::{CommitData, FileChange};

    fn make_commit(hash: &str, author: &str, date: &str, files: Vec<FileChange>) -> CommitData {
        CommitData {
            hash: hash.to_string(),
            author: author.to_string(),
            date: date.to_string(),
            files,
        }
    }

    fn make_file(path: &str, adds: u32, dels: u32) -> FileChange {
        FileChange {
            additions: adds,
            deletions: dels,
            path: path.to_string(),
            old_path: None,
        }
    }

    #[test]
    fn empty_commits_produces_empty_result() {
        let result = compute_churn(&[], &BTreeMap::new(), "2026-03-25");
        assert!(result.is_empty());
    }

    #[test]
    fn single_commit_within_30d() {
        let commits = vec![make_commit(
            "abc",
            "Alice",
            "2026-03-20T10:00:00+00:00",
            vec![make_file("src/main.rs", 10, 5)],
        )];

        let result = compute_churn(&commits, &BTreeMap::new(), "2026-03-25");
        let path = CanonicalPath::new("src/main.rs");
        let m = result.get(&path).expect("should have metrics");

        assert_eq!(m.commits_30d, 1);
        assert_eq!(m.commits_90d, 1);
        assert_eq!(m.commits_1y, 1);
        assert_eq!(m.lines_changed_30d, 15);
        assert_eq!(m.lines_changed_90d, 15);
        assert_eq!(m.authors_30d, 1);
        assert_eq!(m.last_changed.as_deref(), Some("2026-03-20"));
        assert_eq!(m.top_authors, vec![("Alice".to_string(), 1)]);
    }

    #[test]
    fn windowing_30d_90d_1y() {
        // window_end = 2026-03-25
        // 30d boundary = ~2026-02-23
        // 90d boundary = ~2025-12-25
        // 1y boundary  = ~2025-03-25
        let commits = vec![
            // Within 30d
            make_commit("c1", "A", "2026-03-20", vec![make_file("f.rs", 1, 0)]),
            // Within 90d but outside 30d
            make_commit("c2", "B", "2026-01-15", vec![make_file("f.rs", 2, 0)]),
            // Within 1y but outside 90d
            make_commit("c3", "C", "2025-06-01", vec![make_file("f.rs", 3, 0)]),
        ];

        let result = compute_churn(&commits, &BTreeMap::new(), "2026-03-25");
        let path = CanonicalPath::new("f.rs");
        let m = result.get(&path).expect("should have metrics");

        assert_eq!(m.commits_30d, 1);
        assert_eq!(m.commits_90d, 2);
        assert_eq!(m.commits_1y, 3);
        assert_eq!(m.lines_changed_30d, 1);
        assert_eq!(m.lines_changed_90d, 3); // 1 + 2
        assert_eq!(m.authors_30d, 1); // Only A is within 30d
    }

    #[test]
    fn rename_map_resolves_paths() {
        let mut rename_map = BTreeMap::new();
        rename_map.insert("old/path.rs".to_string(), CanonicalPath::new("new/path.rs"));

        let commits = vec![make_commit(
            "c1",
            "Alice",
            "2026-03-20",
            vec![make_file("old/path.rs", 5, 3)],
        )];

        let result = compute_churn(&commits, &rename_map, "2026-03-25");

        // Should be stored under the canonical (new) path
        assert!(result.contains_key(&CanonicalPath::new("new/path.rs")));
        assert!(!result.contains_key(&CanonicalPath::new("old/path.rs")));
    }

    #[test]
    fn top_authors_max_5_sorted_desc() {
        let authors: Vec<&str> = vec!["A", "B", "C", "D", "E", "F"];
        let mut commits = Vec::new();
        for (i, author) in authors.iter().enumerate() {
            // Give each author a different number of commits: A=6, B=5, ..., F=1
            for j in 0..(authors.len() - i) {
                commits.push(make_commit(
                    &format!("c{}{}", i, j),
                    author,
                    "2026-03-20",
                    vec![make_file("f.rs", 1, 0)],
                ));
            }
        }

        let result = compute_churn(&commits, &BTreeMap::new(), "2026-03-25");
        let m = result.get(&CanonicalPath::new("f.rs")).unwrap();

        assert_eq!(m.top_authors.len(), 5);
        assert_eq!(m.top_authors[0].0, "A");
        assert_eq!(m.top_authors[0].1, 6);
        assert_eq!(m.top_authors[4].0, "E");
        assert_eq!(m.top_authors[4].1, 2);
    }

    #[test]
    fn multiple_authors_in_30d_window() {
        let commits = vec![
            make_commit("c1", "Alice", "2026-03-20", vec![make_file("f.rs", 1, 0)]),
            make_commit("c2", "Bob", "2026-03-21", vec![make_file("f.rs", 1, 0)]),
            make_commit("c3", "Alice", "2026-03-22", vec![make_file("f.rs", 1, 0)]),
        ];

        let result = compute_churn(&commits, &BTreeMap::new(), "2026-03-25");
        let m = result.get(&CanonicalPath::new("f.rs")).unwrap();

        assert_eq!(m.authors_30d, 2); // Alice and Bob
        assert_eq!(m.commits_30d, 3);
    }

    #[test]
    fn last_changed_tracks_most_recent() {
        let commits = vec![
            make_commit("c1", "A", "2026-03-10", vec![make_file("f.rs", 1, 0)]),
            make_commit("c2", "A", "2026-03-20", vec![make_file("f.rs", 1, 0)]),
            make_commit("c3", "A", "2026-03-15", vec![make_file("f.rs", 1, 0)]),
        ];

        let result = compute_churn(&commits, &BTreeMap::new(), "2026-03-25");
        let m = result.get(&CanonicalPath::new("f.rs")).unwrap();

        assert_eq!(m.last_changed.as_deref(), Some("2026-03-20"));
    }

    #[test]
    fn invalid_window_end_returns_empty() {
        let commits = vec![make_commit(
            "c1",
            "A",
            "2026-03-20",
            vec![make_file("f.rs", 1, 0)],
        )];

        let result = compute_churn(&commits, &BTreeMap::new(), "invalid");
        assert!(result.is_empty());
    }

    #[test]
    fn parse_date_to_days_valid() {
        assert!(parse_date_to_days("2026-03-25").is_some());
        assert!(parse_date_to_days("2026-03-25T14:30:00+00:00").is_some());
    }

    #[test]
    fn parse_date_to_days_invalid() {
        assert!(parse_date_to_days("invalid").is_none());
        assert!(parse_date_to_days("").is_none());
        assert!(parse_date_to_days("2026").is_none());
    }
}
