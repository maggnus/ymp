//! Workspace I/O port. Provider observations cannot grant execution authority.
use crate::journal::ContentStore;
use ymp_domain::{
    Result,
    journal::PolicySelection,
    workspace::{PathObservation, SnapshotTree, WorkspaceLocation, WorkspacePath},
};

pub trait WorkspaceProvider: Send + Sync {
    fn selection(&self) -> &PolicySelection;
    fn location(&self) -> Result<WorkspaceLocation>;
    /// Validate current path topology. Missing leaf paths may be intended writes;
    /// symlinks, hard links, aliases and unsupported file kinds are refused.
    fn validate_paths(&self, paths: &[WorkspacePath]) -> Result<()>;
    fn observe_paths(&self, _paths: &[WorkspacePath]) -> Result<Vec<PathObservation>> {
        Err(ymp_domain::Denial::new(
            "path_observation",
            "This provider does not supply physical path observations",
        ))
    }
    /// Store immutable file bytes before returning a complete manifest.
    /// All capture I/O must finish before return, including on failure; no detached
    /// operations may survive this synchronous call.
    /// The caller validates that every digest resolves before committing SnapshotTaken.
    fn capture(&self, store: &dyn ContentStore) -> Result<SnapshotTree>;
}
