//! Workspace I/O port. Provider observations cannot grant execution authority.
use crate::journal::ContentStore;
use ymp_domain::{
    Result,
    journal::PolicySelection,
    workspace::{SnapshotTree, WorkspaceLocation, WorkspacePath},
};

pub trait WorkspaceProvider: Send + Sync {
    fn selection(&self) -> &PolicySelection;
    fn location(&self) -> Result<WorkspaceLocation>;
    /// Validate current path topology. Missing leaf paths may be intended writes;
    /// symlinks, hard links, aliases and unsupported file kinds are refused.
    fn validate_paths(&self, paths: &[WorkspacePath]) -> Result<()>;
    /// Store immutable file bytes before returning a complete manifest.
    /// The caller validates that every digest resolves before committing SnapshotTaken.
    fn capture(&self, store: &dyn ContentStore) -> Result<SnapshotTree>;
}
