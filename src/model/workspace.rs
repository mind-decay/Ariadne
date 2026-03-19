use std::path::{Path, PathBuf};

/// Information about a detected workspace (npm/yarn/pnpm monorepo).
#[derive(Debug, Clone)]
pub struct WorkspaceInfo {
    pub kind: WorkspaceKind,
    pub members: Vec<WorkspaceMember>,
}

impl WorkspaceInfo {
    /// Create a copy with all member paths relativized to the given root.
    /// This converts absolute `path` and `entry_point` fields to paths
    /// relative to the project root, suitable for CanonicalPath comparison.
    pub fn relativize(&self, root: &Path) -> WorkspaceInfo {
        WorkspaceInfo {
            kind: self.kind.clone(),
            members: self
                .members
                .iter()
                .map(|m| WorkspaceMember {
                    name: m.name.clone(),
                    path: m.path.strip_prefix(root).unwrap_or(&m.path).to_path_buf(),
                    entry_point: m
                        .entry_point
                        .strip_prefix(root)
                        .unwrap_or(&m.entry_point)
                        .to_path_buf(),
                })
                .collect(),
        }
    }
}

/// A single workspace member (package).
#[derive(Debug, Clone)]
pub struct WorkspaceMember {
    /// Package name from package.json (e.g., "@myapp/auth")
    pub name: String,
    /// Absolute path to the member directory
    pub path: PathBuf,
    /// Path to the entry point file (resolved from main/module/default probe)
    pub entry_point: PathBuf,
}

/// The type of workspace/monorepo tool.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkspaceKind {
    Npm,
    Yarn,
    Pnpm,
}
