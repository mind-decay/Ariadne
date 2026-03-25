use std::collections::{BTreeMap, BTreeSet, HashMap};

use crate::model::temporal::OwnershipInfo;
use crate::model::CanonicalPath;

use super::git::CommitData;

/// Compute ownership information for all files referenced in the given commits.
///
/// For each file, determines the last author (most recent commit), distinct author count,
/// and top contributors by commit count (max 5).
///
/// File paths are resolved via `rename_map`; paths not in the map are treated as
/// already-canonical.
pub(crate) fn compute_ownership(
    commits: &[CommitData],
    rename_map: &BTreeMap<String, CanonicalPath>,
) -> BTreeMap<CanonicalPath, OwnershipInfo> {
    // Accumulator per canonical path
    let mut acc: BTreeMap<CanonicalPath, FileOwnership> = BTreeMap::new();

    for commit in commits {
        for file in &commit.files {
            let canonical = resolve_path(&file.path, rename_map);
            let entry = acc.entry(canonical).or_default();

            // Track last author: use the most recent commit date.
            // Git log outputs newest first, so the first commit we see for a file
            // is the most recent — but we process all commits, so track explicitly.
            let commit_date = &commit.date;
            if entry.last_date.is_none() || commit_date.as_str() > entry.last_date.as_deref().unwrap_or("")
            {
                entry.last_author = Some(commit.author.clone());
                entry.last_date = Some(commit_date.clone());
            }

            entry.authors.insert(commit.author.clone());
            *entry.author_commits.entry(commit.author.clone()).or_insert(0) += 1;
        }
    }

    acc.into_iter()
        .map(|(path, o)| {
            let mut top_contributors: Vec<(String, u32)> =
                o.author_commits.into_iter().collect();
            top_contributors.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
            top_contributors.truncate(5);

            let info = OwnershipInfo {
                last_author: o.last_author.unwrap_or_default(),
                top_contributors,
                author_count: o.authors.len() as u32,
            };
            (path, info)
        })
        .collect()
}

#[derive(Default)]
struct FileOwnership {
    last_author: Option<String>,
    last_date: Option<String>,
    authors: BTreeSet<String>,
    author_commits: HashMap<String, u32>,
}

/// Resolve a file path to its canonical path via the rename map.
fn resolve_path(
    path: &str,
    rename_map: &BTreeMap<String, CanonicalPath>,
) -> CanonicalPath {
    rename_map
        .get(path)
        .cloned()
        .unwrap_or_else(|| CanonicalPath::new(path))
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
        let result = compute_ownership(&[], &BTreeMap::new());
        assert!(result.is_empty());
    }

    #[test]
    fn single_author_single_file() {
        let commits = vec![make_commit(
            "c1",
            "Alice",
            "2026-03-20T10:00:00+00:00",
            vec![make_file("src/main.rs", 10, 5)],
        )];

        let result = compute_ownership(&commits, &BTreeMap::new());
        let path = CanonicalPath::new("src/main.rs");
        let info = result.get(&path).expect("should have ownership");

        assert_eq!(info.last_author, "Alice");
        assert_eq!(info.author_count, 1);
        assert_eq!(info.top_contributors, vec![("Alice".to_string(), 1)]);
    }

    #[test]
    fn multiple_authors_same_file() {
        let commits = vec![
            make_commit("c1", "Alice", "2026-03-20", vec![make_file("f.rs", 1, 0)]),
            make_commit("c2", "Bob", "2026-03-21", vec![make_file("f.rs", 1, 0)]),
            make_commit("c3", "Alice", "2026-03-22", vec![make_file("f.rs", 1, 0)]),
        ];

        let result = compute_ownership(&commits, &BTreeMap::new());
        let info = result.get(&CanonicalPath::new("f.rs")).unwrap();

        // Alice has the latest date (2026-03-22)
        assert_eq!(info.last_author, "Alice");
        assert_eq!(info.author_count, 2);
        // Alice: 2 commits, Bob: 1 commit
        assert_eq!(info.top_contributors[0], ("Alice".to_string(), 2));
        assert_eq!(info.top_contributors[1], ("Bob".to_string(), 1));
    }

    #[test]
    fn last_author_is_most_recent_by_date() {
        // Even though Bob's commit is listed first, Alice has the later date
        let commits = vec![
            make_commit("c1", "Bob", "2026-03-15", vec![make_file("f.rs", 1, 0)]),
            make_commit("c2", "Alice", "2026-03-20", vec![make_file("f.rs", 1, 0)]),
        ];

        let result = compute_ownership(&commits, &BTreeMap::new());
        let info = result.get(&CanonicalPath::new("f.rs")).unwrap();

        assert_eq!(info.last_author, "Alice");
    }

    #[test]
    fn rename_map_resolves_paths() {
        let mut rename_map = BTreeMap::new();
        rename_map.insert("old.rs".to_string(), CanonicalPath::new("new.rs"));

        let commits = vec![make_commit(
            "c1",
            "Alice",
            "2026-03-20",
            vec![make_file("old.rs", 5, 3)],
        )];

        let result = compute_ownership(&commits, &rename_map);

        assert!(result.contains_key(&CanonicalPath::new("new.rs")));
        assert!(!result.contains_key(&CanonicalPath::new("old.rs")));
    }

    #[test]
    fn top_contributors_max_5() {
        let authors = vec!["A", "B", "C", "D", "E", "F"];
        let mut commits = Vec::new();
        for (i, author) in authors.iter().enumerate() {
            for j in 0..(authors.len() - i) {
                commits.push(make_commit(
                    &format!("c{}{}", i, j),
                    author,
                    "2026-03-20",
                    vec![make_file("f.rs", 1, 0)],
                ));
            }
        }

        let result = compute_ownership(&commits, &BTreeMap::new());
        let info = result.get(&CanonicalPath::new("f.rs")).unwrap();

        assert_eq!(info.top_contributors.len(), 5);
        assert_eq!(info.author_count, 6); // 6 distinct authors
        assert_eq!(info.top_contributors[0].0, "A");
        assert_eq!(info.top_contributors[0].1, 6);
    }

    #[test]
    fn multiple_files_tracked_separately() {
        let commits = vec![
            make_commit(
                "c1",
                "Alice",
                "2026-03-20",
                vec![make_file("a.rs", 1, 0), make_file("b.rs", 2, 0)],
            ),
            make_commit("c2", "Bob", "2026-03-21", vec![make_file("a.rs", 3, 0)]),
        ];

        let result = compute_ownership(&commits, &BTreeMap::new());

        let a_info = result.get(&CanonicalPath::new("a.rs")).unwrap();
        assert_eq!(a_info.author_count, 2);
        assert_eq!(a_info.last_author, "Bob");

        let b_info = result.get(&CanonicalPath::new("b.rs")).unwrap();
        assert_eq!(b_info.author_count, 1);
        assert_eq!(b_info.last_author, "Alice");
    }
}
