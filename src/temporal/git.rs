use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;

use crate::diagnostic::{DiagnosticCollector, Warning, WarningCode};
use crate::model::CanonicalPath;

/// Parsed data from a single git commit.
#[derive(Debug, Clone)]
pub(crate) struct CommitData {
    #[allow(dead_code)] // used in tests; retained for commit identification
    pub hash: String,
    pub author: String,
    pub date: String, // ISO 8601
    pub files: Vec<FileChange>,
}

/// A single file change within a commit.
#[derive(Debug, Clone)]
pub(crate) struct FileChange {
    pub additions: u32,
    pub deletions: u32,
    pub path: String,
    pub old_path: Option<String>, // Set when -M detects rename
}

/// Parse git log for temporal analysis.
///
/// Returns `None` if git is unavailable or not a git repo.
/// Returns `(commits, rename_map)` on success where `rename_map` maps
/// old paths to their current canonical paths (chaining renames).
pub(crate) fn parse_git_log(
    project_root: &Path,
    collector: &DiagnosticCollector,
) -> Option<(Vec<CommitData>, BTreeMap<String, CanonicalPath>)> {
    // 1. Check git binary exists
    if Command::new("git").arg("--version").output().is_err() {
        collector.warn(Warning {
            code: WarningCode::W024GitNotFound,
            path: CanonicalPath::new(""),
            message: "git binary not found in PATH".to_string(),
            detail: Some("temporal analysis requires git".to_string()),
        });
        return None;
    }

    // 2. Check git repo
    let rev_parse = Command::new("git")
        .args(["-C", &project_root.to_string_lossy(), "rev-parse", "--git-dir"])
        .output();

    match rev_parse {
        Ok(output) if output.status.success() => {}
        _ => {
            collector.warn(Warning {
                code: WarningCode::W025NotGitRepository,
                path: CanonicalPath::new(""),
                message: "project root is not inside a git repository".to_string(),
                detail: None,
            });
            return None;
        }
    }

    // 3. Check shallow
    let shallow_check = Command::new("git")
        .args([
            "-C",
            &project_root.to_string_lossy(),
            "rev-parse",
            "--is-shallow-repository",
        ])
        .output();

    if let Ok(output) = shallow_check {
        let stdout = String::from_utf8_lossy(&output.stdout);
        if stdout.trim() == "true" {
            collector.warn(Warning {
                code: WarningCode::W026ShallowRepository,
                path: CanonicalPath::new(""),
                message: "repository is a shallow clone; temporal analysis may be incomplete"
                    .to_string(),
                detail: None,
            });
        }
    }

    // 4. Execute git log
    let git_log = Command::new("git")
        .args([
            "-C",
            &project_root.to_string_lossy(),
            "log",
            "--numstat",
            "-M",
            "--format=commit %H%nauthor %aN%ndate %aI",
            "--since=1 year ago",
        ])
        .output();

    let output = match git_log {
        Ok(output) if output.status.success() => {
            String::from_utf8_lossy(&output.stdout).into_owned()
        }
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            collector.warn(Warning {
                code: WarningCode::W027GitCommandFailed,
                path: CanonicalPath::new(""),
                message: "git log command failed".to_string(),
                detail: Some(stderr.trim().to_string()),
            });
            return None;
        }
        Err(e) => {
            collector.warn(Warning {
                code: WarningCode::W027GitCommandFailed,
                path: CanonicalPath::new(""),
                message: "failed to execute git log".to_string(),
                detail: Some(e.to_string()),
            });
            return None;
        }
    };

    // 5. Parse output
    let commits = parse_git_output(&output);

    // 6. Build rename map
    let rename_map = build_rename_map(&commits, project_root);

    Some((commits, rename_map))
}

/// Parse raw git log output into structured commit data.
///
/// This is separated from `parse_git_log` to enable testing without
/// actually running git.
pub(crate) fn parse_git_output(output: &str) -> Vec<CommitData> {
    let mut commits = Vec::new();
    let mut current: Option<CommitData> = None;

    for line in output.lines() {
        if let Some(hash) = line.strip_prefix("commit ") {
            // Flush previous commit
            if let Some(commit) = current.take() {
                commits.push(commit);
            }
            current = Some(CommitData {
                hash: hash.trim().to_string(),
                author: String::new(),
                date: String::new(),
                files: Vec::new(),
            });
        } else if let Some(author) = line.strip_prefix("author ") {
            if let Some(ref mut commit) = current {
                commit.author = author.trim().to_string();
            }
        } else if let Some(date) = line.strip_prefix("date ") {
            if let Some(ref mut commit) = current {
                commit.date = date.trim().to_string();
            }
        } else if line.is_empty() {
            // Blank line between header and numstat, or between commits — skip
        } else {
            // Try to parse as numstat line: "{adds}\t{dels}\t{path}"
            if let Some(change) = parse_numstat_line(line) {
                if let Some(ref mut commit) = current {
                    commit.files.push(change);
                }
            }
            // Malformed lines are silently skipped
        }
    }

    // Flush last commit
    if let Some(commit) = current.take() {
        commits.push(commit);
    }

    commits
}

/// Parse a single numstat line.
///
/// Format: `{additions}\t{deletions}\t{path}`
/// Binary files show as: `-\t-\t{path}`
/// Renames show as: `{adds}\t{dels}\t{old => new}` or `{adds}\t{dels}\t{prefix}/{old => new}/suffix`
fn parse_numstat_line(line: &str) -> Option<FileChange> {
    let parts: Vec<&str> = line.splitn(3, '\t').collect();
    if parts.len() != 3 {
        return None;
    }

    let adds_str = parts[0].trim();
    let dels_str = parts[1].trim();
    let path_str = parts[2].trim();

    // Binary files have "-" for both additions and deletions — skip them
    if adds_str == "-" && dels_str == "-" {
        return None;
    }

    let additions = adds_str.parse::<u32>().ok()?;
    let deletions = dels_str.parse::<u32>().ok()?;

    // Check for rename pattern: `{old => new}` or `old => new`
    let (path, old_path) = parse_rename_path(path_str);

    Some(FileChange {
        additions,
        deletions,
        path,
        old_path,
    })
}

/// Parse a potentially renamed path from numstat output.
///
/// Git rename formats:
/// - `{old => new}` (simple rename in same directory)
/// - `prefix/{old => new}/suffix` (rename with shared prefix/suffix)
/// - `old => new` (complete path change, no braces)
///
/// Returns `(current_path, Some(old_path))` for renames, or `(path, None)` for normal paths.
fn parse_rename_path(path: &str) -> (String, Option<String>) {
    // Try brace-style rename: `prefix/{old => new}/suffix`
    if let Some(brace_start) = path.find('{') {
        if let Some(brace_end) = path.find('}') {
            if brace_start < brace_end {
                let prefix = &path[..brace_start];
                let suffix = &path[brace_end + 1..];
                let inner = &path[brace_start + 1..brace_end];

                if let Some((old_part, new_part)) = inner.split_once(" => ") {
                    let old_path = format!("{}{}{}", prefix, old_part, suffix);
                    let new_path = format!("{}{}{}", prefix, new_part, suffix);
                    return (new_path, Some(old_path));
                }
            }
        }
    }

    // Try simple arrow rename: `old => new`
    if let Some((old, new)) = path.split_once(" => ") {
        return (new.to_string(), Some(old.to_string()));
    }

    // Normal path, no rename
    (path.to_string(), None)
}

/// Build a rename map that chains renames: if A->B and B->C, then A maps to C.
/// Only includes mappings where the final path exists on disk.
fn build_rename_map(commits: &[CommitData], project_root: &Path) -> BTreeMap<String, CanonicalPath> {
    // First pass: collect all direct renames (old -> new).
    // Process commits in chronological order (git log outputs newest first).
    let mut direct_map: BTreeMap<String, String> = BTreeMap::new();

    for commit in commits.iter().rev() {
        for file in &commit.files {
            if let Some(ref old_path) = file.old_path {
                direct_map.insert(old_path.clone(), file.path.clone());
            }
        }
    }

    // Second pass: resolve chains. For each old path, follow the chain to its final name.
    let mut rename_map = BTreeMap::new();

    for old_path in direct_map.keys().cloned().collect::<Vec<_>>() {
        let mut current = old_path.clone();
        let mut visited = std::collections::HashSet::new();
        visited.insert(current.clone());

        while let Some(next) = direct_map.get(&current) {
            if !visited.insert(next.clone()) {
                // Cycle detected — break
                break;
            }
            current = next.clone();
        }

        // Only include if final path exists on disk
        let final_abs = project_root.join(&current);
        if final_abs.exists() {
            rename_map.insert(old_path, CanonicalPath::new(current));
        }
    }

    rename_map
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_standard_git_log_output() {
        let output = "\
commit abc123def456
author Alice Smith
date 2026-03-20T14:30:00+00:00

10\t5\tsrc/main.rs
3\t1\tsrc/lib.rs

commit def789abc012
author Bob Jones
date 2026-03-19T10:00:00+00:00

20\t10\tsrc/parser/mod.rs
";

        let commits = parse_git_output(output);
        assert_eq!(commits.len(), 2);

        assert_eq!(commits[0].hash, "abc123def456");
        assert_eq!(commits[0].author, "Alice Smith");
        assert_eq!(commits[0].date, "2026-03-20T14:30:00+00:00");
        assert_eq!(commits[0].files.len(), 2);
        assert_eq!(commits[0].files[0].additions, 10);
        assert_eq!(commits[0].files[0].deletions, 5);
        assert_eq!(commits[0].files[0].path, "src/main.rs");
        assert!(commits[0].files[0].old_path.is_none());

        assert_eq!(commits[1].hash, "def789abc012");
        assert_eq!(commits[1].author, "Bob Jones");
        assert_eq!(commits[1].files.len(), 1);
        assert_eq!(commits[1].files[0].path, "src/parser/mod.rs");
    }

    #[test]
    fn parse_rename_brace_format() {
        let output = "\
commit aaa111
author Dev
date 2026-01-01T00:00:00+00:00

5\t2\tsrc/{old_name.rs => new_name.rs}
";

        let commits = parse_git_output(output);
        assert_eq!(commits.len(), 1);
        assert_eq!(commits[0].files.len(), 1);

        let f = &commits[0].files[0];
        assert_eq!(f.path, "src/new_name.rs");
        assert_eq!(f.old_path.as_deref(), Some("src/old_name.rs"));
        assert_eq!(f.additions, 5);
        assert_eq!(f.deletions, 2);
    }

    #[test]
    fn parse_rename_arrow_format() {
        let output = "\
commit bbb222
author Dev
date 2026-01-01T00:00:00+00:00

0\t0\told/path.rs => new/path.rs
";

        let commits = parse_git_output(output);
        assert_eq!(commits.len(), 1);
        assert_eq!(commits[0].files.len(), 1);

        let f = &commits[0].files[0];
        assert_eq!(f.path, "new/path.rs");
        assert_eq!(f.old_path.as_deref(), Some("old/path.rs"));
    }

    #[test]
    fn parse_rename_nested_brace_format() {
        // Rename with shared prefix and suffix: src/{utils => helpers}/format.rs
        let output = "\
commit ccc333
author Dev
date 2026-01-01T00:00:00+00:00

3\t1\tsrc/{utils => helpers}/format.rs
";

        let commits = parse_git_output(output);
        assert_eq!(commits.len(), 1);
        let f = &commits[0].files[0];
        assert_eq!(f.path, "src/helpers/format.rs");
        assert_eq!(f.old_path.as_deref(), Some("src/utils/format.rs"));
    }

    #[test]
    fn parse_empty_output() {
        let commits = parse_git_output("");
        assert!(commits.is_empty());
    }

    #[test]
    fn parse_empty_output_with_whitespace() {
        let commits = parse_git_output("   \n\n   \n");
        assert!(commits.is_empty());
    }

    #[test]
    fn malformed_lines_are_skipped() {
        let output = "\
commit aaa111
author Dev
date 2026-01-01T00:00:00+00:00

this is not a valid numstat line
10\t5\tsrc/valid.rs
also invalid
nope\tnope\tnope
";

        let commits = parse_git_output(output);
        assert_eq!(commits.len(), 1);
        // Only the valid numstat line is parsed
        assert_eq!(commits[0].files.len(), 1);
        assert_eq!(commits[0].files[0].path, "src/valid.rs");
    }

    #[test]
    fn binary_files_are_skipped() {
        let output = "\
commit ddd444
author Dev
date 2026-01-01T00:00:00+00:00

-\t-\timage.png
10\t5\tsrc/code.rs
-\t-\tfont.woff2
";

        let commits = parse_git_output(output);
        assert_eq!(commits.len(), 1);
        // Binary files (- - path) should be skipped
        assert_eq!(commits[0].files.len(), 1);
        assert_eq!(commits[0].files[0].path, "src/code.rs");
    }

    #[test]
    fn multiple_commits_with_no_files() {
        let output = "\
commit eee555
author Dev
date 2026-01-01T00:00:00+00:00

commit fff666
author Dev2
date 2026-01-02T00:00:00+00:00

1\t0\tREADME.md
";

        let commits = parse_git_output(output);
        assert_eq!(commits.len(), 2);
        assert!(commits[0].files.is_empty());
        assert_eq!(commits[1].files.len(), 1);
    }

    #[test]
    fn parse_rename_path_no_rename() {
        let (path, old) = parse_rename_path("src/main.rs");
        assert_eq!(path, "src/main.rs");
        assert!(old.is_none());
    }

    #[test]
    fn parse_rename_path_brace_style() {
        let (path, old) = parse_rename_path("src/{old.rs => new.rs}");
        assert_eq!(path, "src/new.rs");
        assert_eq!(old.as_deref(), Some("src/old.rs"));
    }

    #[test]
    fn parse_rename_path_arrow_style() {
        let (path, old) = parse_rename_path("a/b.rs => c/d.rs");
        assert_eq!(path, "c/d.rs");
        assert_eq!(old.as_deref(), Some("a/b.rs"));
    }

    #[test]
    fn build_rename_map_chains_renames() {
        // Simulate: A->B in commit 2 (older), B->C in commit 1 (newer)
        // Git log outputs newest first, so commit 1 comes first
        let commits = vec![
            CommitData {
                hash: "newer".to_string(),
                author: "Dev".to_string(),
                date: "2026-03-02".to_string(),
                files: vec![FileChange {
                    additions: 0,
                    deletions: 0,
                    path: "src/c.rs".to_string(),
                    old_path: Some("src/b.rs".to_string()),
                }],
            },
            CommitData {
                hash: "older".to_string(),
                author: "Dev".to_string(),
                date: "2026-03-01".to_string(),
                files: vec![FileChange {
                    additions: 0,
                    deletions: 0,
                    path: "src/b.rs".to_string(),
                    old_path: Some("src/a.rs".to_string()),
                }],
            },
        ];

        // Use a temp dir with the final file present
        let tmp = std::env::temp_dir().join("ariadne_test_rename_chain");
        let _ = std::fs::create_dir_all(tmp.join("src"));
        let _ = std::fs::write(tmp.join("src/c.rs"), "");

        let map = build_rename_map(&commits, &tmp);

        // a.rs should chain through b.rs to c.rs
        assert_eq!(map.get("src/a.rs").map(|p| p.as_str()), Some("src/c.rs"));
        // b.rs should map directly to c.rs
        assert_eq!(map.get("src/b.rs").map(|p| p.as_str()), Some("src/c.rs"));

        // Cleanup
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn build_rename_map_excludes_deleted_files() {
        let commits = vec![CommitData {
            hash: "abc".to_string(),
            author: "Dev".to_string(),
            date: "2026-03-01".to_string(),
            files: vec![FileChange {
                additions: 0,
                deletions: 0,
                path: "nonexistent_file.rs".to_string(),
                old_path: Some("old_name.rs".to_string()),
            }],
        }];

        let tmp = std::env::temp_dir().join("ariadne_test_rename_deleted");
        let _ = std::fs::create_dir_all(&tmp);

        let map = build_rename_map(&commits, &tmp);

        // Should not include mapping since final path doesn't exist
        assert!(map.is_empty());

        let _ = std::fs::remove_dir_all(&tmp);
    }
}
