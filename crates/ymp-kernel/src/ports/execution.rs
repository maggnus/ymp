//! Workspace I/O port. Provider observations cannot grant execution authority.
use crate::journal::{ContentStore, Journal};
use ymp_domain::{
    Result,
    journal::PolicySelection,
    workspace::{
        PathObservation, SnapshotTree, WorkspaceBinding, WorkspaceLocation, WorkspacePath,
    },
};

/// A short physical-root coordination section for resolving and preparing I/O.
/// Every mediated operation participates in the same physical-root coordinator,
/// including across processes. Data I/O runs after this guard drops.
pub trait WorkspaceCoordination {
    fn binding(&self) -> &WorkspaceBinding;
}
/// An already opened, validated object. Preparation never truncates or writes data.
/// Implementations keep its descriptor pinned through synchronous data I/O. Every
/// method ends all its I/O before returning, including on error; no descriptor or
/// detached operation escapes this object. Dropping it ends its descriptor ownership.
pub trait WorkspaceFile {
    fn identity(&self) -> ymp_domain::workspace::FileIdentity;
    fn read(&mut self) -> Result<Vec<u8>>;
    fn write(&mut self, bytes: &[u8]) -> Result<()>;
}

pub trait WorkspaceProvider: Send + Sync {
    fn selection(&self) -> &PolicySelection;
    fn location(&self) -> Result<WorkspaceLocation>;
    fn coordinate(
        &self,
        _binding: &WorkspaceBinding,
    ) -> Result<Box<dyn WorkspaceCoordination + '_>> {
        Err(ymp_domain::Denial::new(
            "file_coordination",
            "Provider has no physical I/O coordinator",
        ))
    }
    fn validate_file_request(&self, _access: &crate::workspace_guard::FileAccess) -> Result<()> {
        Ok(())
    }
    fn prepare_file(
        &self,
        _access: &crate::workspace_guard::FileAccess,
    ) -> Result<Box<dyn WorkspaceFile>> {
        Err(ymp_domain::Denial::new(
            "file_operation",
            "Provider has no prepared file operations",
        ))
    }

    /// Available operations may only narrow the kernel mediator's file API.
    fn file_modes(&self) -> Vec<ymp_domain::workspace::LockMode> {
        vec![]
    }
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
