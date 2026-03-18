use std::path::PathBuf;

/// Information about a detected workspace (npm/yarn/pnpm monorepo).
#[derive(Debug, Clone)]
pub struct WorkspaceInfo {
    pub kind: WorkspaceKind,
    pub members: Vec<WorkspaceMember>,
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
