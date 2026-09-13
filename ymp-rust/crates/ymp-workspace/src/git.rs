//! Shared data contract for the embedded Git backend. Git operations never invoke a tool.

use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Comparison {
    Uncommitted,
    Committed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Change {
    pub path: PathBuf,
    pub old_path: Option<PathBuf>,
    pub status: String,
    pub staged: bool,
    pub unstaged: bool,
    pub untracked: bool,
    pub added: Option<u64>,
    pub removed: Option<u64>,
    pub binary: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Snapshot {
    pub root: PathBuf,
    pub branch: Option<String>,
    pub head: Option<String>,
    pub base: Option<String>,
    pub base_oid: Option<String>,
    pub dirty: bool,
    pub comparison: Comparison,
    pub files: Vec<Change>,
    pub notice: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Worktree {
    pub path: PathBuf,
    pub branch: Option<String>,
    pub head: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Branch {
    pub name: String,
    pub current: bool,
    pub worktree: Option<PathBuf>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Choices {
    pub worktrees: Vec<Worktree>,
    pub branches: Vec<Branch>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diff {
    pub text: String,
    pub notice: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GitError {
    NotRepository,
    Unavailable(String),
    Failed(String),
    Timeout,
    TooLarge,
    Stale,
    Dirty,
}

impl std::fmt::Display for GitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotRepository => write!(f, "Not a Git working tree"),
            Self::Unavailable(message) | Self::Failed(message) => f.write_str(message),
            Self::Timeout => write!(f, "Git inspection exceeded its work budget"),
            Self::TooLarge => write!(f, "Git data is too large to display"),
            Self::Stale => write!(f, "The repository changed; refresh before continuing"),
            Self::Dirty => write!(
                f,
                "The worktree has uncommitted changes; switching was refused"
            ),
        }
    }
}

impl std::error::Error for GitError {}

// These temporary entry points let the independently authored UI compile against one contract.
// They are replaced by the embedded implementation before integration acceptance or release.
pub async fn inspect(
    _cwd: &Path,
    _comparison: Option<Comparison>,
    _base: Option<&str>,
) -> Result<Snapshot, GitError> {
    Err(GitError::Unavailable(
        "Git backend integration is in progress".into(),
    ))
}

pub async fn choices(_cwd: &Path) -> Result<Choices, GitError> {
    Err(GitError::Unavailable(
        "Git backend integration is in progress".into(),
    ))
}

pub async fn diff(_snapshot: &Snapshot, _change: &Change) -> Result<Diff, GitError> {
    Err(GitError::Unavailable(
        "Git backend integration is in progress".into(),
    ))
}

/// Caller holds the selected project's existing Store lock and excludes active/new agent runs.
pub async fn switch_branch(
    _cwd: &Path,
    _branch: &str,
    _expected_head: Option<&str>,
) -> Result<(), GitError> {
    Err(GitError::Unavailable(
        "Git backend integration is in progress".into(),
    ))
}
