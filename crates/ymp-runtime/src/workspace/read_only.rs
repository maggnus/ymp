//! A constrained WorkspaceProvider: live capture and file reads, no file writes.
use super::direct::Direct;
use std::path::Path;
use ymp_domain::{Result, journal::PolicySelection, workspace::*};
use ymp_kernel::{
    journal::{ContentStore, Journal},
    ports::execution::WorkspaceProvider,
    workspace_guard::FileAccess,
};
pub struct ReadOnly {
    direct: Direct,
    selection: PolicySelection,
}
impl ReadOnly {
    pub fn open(root: &Path, limits: CaptureLimits) -> Result<Self> {
        let direct = Direct::open(root, limits)?;
        let selection = PolicySelection::new(
            "WorkspaceProvider",
            "ReadOnly",
            "1",
            direct.selection().parameters.clone(),
        )?;
        Ok(Self { direct, selection })
    }
}
impl WorkspaceProvider for ReadOnly {
    fn coordinate(
        &self,
        binding: &WorkspaceBinding,
    ) -> Result<Box<dyn ymp_kernel::ports::execution::WorkspaceCoordination + '_>> {
        self.direct.coordinate(binding)
    }
    fn validate_file_request(&self, access: &FileAccess) -> Result<()> {
        self.direct.validate_file_request(access)
    }
    fn prepare_file(
        &self,
        access: &FileAccess,
    ) -> Result<Box<dyn ymp_kernel::ports::execution::WorkspaceFile>> {
        if access.mode() != LockMode::Read {
            return Err(ymp_domain::Denial::new(
                "file_operation",
                "ReadOnly does not prepare writes",
            ));
        }
        self.direct.prepare_file(access)
    }
    fn selection(&self) -> &PolicySelection {
        &self.selection
    }
    fn location(&self) -> Result<WorkspaceLocation> {
        self.direct.location()
    }
    fn file_modes(&self) -> Vec<LockMode> {
        vec![LockMode::Read]
    }
    fn bind(&self, journal: &dyn Journal) -> Result<WorkspaceBinding> {
        self.direct.bind(journal)
    }
    fn verify_binding(&self, expected: &WorkspaceBinding, journal: &dyn Journal) -> Result<()> {
        self.direct.verify_binding(expected, journal)
    }
    fn validate_paths(&self, paths: &[WorkspacePath]) -> Result<()> {
        self.direct.validate_paths(paths)
    }
    fn observe_paths(&self, paths: &[WorkspacePath]) -> Result<Vec<PathObservation>> {
        self.direct.observe_paths(paths)
    }
    fn capture(
        &self,
        journal: &Result<JournalIdentity>,
        store: &dyn ContentStore,
    ) -> Result<SnapshotTree> {
        self.direct.capture(journal, store)
    }
}
