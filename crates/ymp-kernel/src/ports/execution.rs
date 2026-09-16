//! Workspace I/O port. Provider observations cannot grant execution authority.
use crate::journal::{ContentStore, Journal};
use ymp_domain::{
    Result,
    journal::PolicySelection,
    workspace::{
        PathObservation, SnapshotTree, WorkspaceBinding, WorkspaceLocation, WorkspacePath,
    },
};

pub trait WorkspaceProvider: Send + Sync {
    fn selection(&self) -> &PolicySelection;
    fn location(&self) -> Result<WorkspaceLocation>;
    /// Prepare or attach immutable root metadata; this does not grant access.
    fn bind(&self, _journal: &dyn Journal) -> Result<WorkspaceBinding> {
        Err(ymp_domain::Denial::new(
            "workspace_binding",
            "Provider does not implement persistent root binding",
        ))
    }
    fn verify_binding(&self, _expected: &WorkspaceBinding, _journal: &dyn Journal) -> Result<()> {
        Err(ymp_domain::Denial::new(
            "workspace_binding",
            "Provider does not verify persistent root binding",
        ))
    }
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
    /// `journal` is the caller's identity observation, not access to its writer.
    /// A bound provider must reject an unavailable or different caller identity.
    /// The caller validates that every digest resolves before committing SnapshotTaken.
    fn capture(
        &self,
        journal: &Result<ymp_domain::workspace::JournalIdentity>,
        store: &dyn ContentStore,
    ) -> Result<SnapshotTree>;
}
