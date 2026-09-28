//! Workspace I/O port. Provider observations cannot grant execution authority.
use crate::journal::{ContentStore, Journal};
use ymp_domain::{
    Result,
    journal::PolicySelection,
    workspace::{
        PathObservation, SnapshotTree, WorkspaceBinding, WorkspaceLocation, WorkspacePath,
    },
};

/// Files supplied by the host from its existing mediated capability. Implementations
/// receive no filesystem root, arbitrary process API, or authority to expand scope.
pub trait InvocationFiles: Send + Sync {
    fn read(&self, path: &WorkspacePath, limit: usize) -> Result<Vec<u8>>;
    fn write(&self, path: &WorkspacePath, bytes: &[u8]) -> Result<()>;
}
pub struct ExecutionRequest<'a> {
    pub invocation: ymp_domain::Id<ymp_domain::assignment::Invocation>,
    pub receipt: ymp_domain::Id<ymp_domain::resources::Receipt>,
    pub assignment: ymp_domain::assignment::Assignment,
    pub prompt: ymp_domain::assignment::Prompt,
    pub settings: ymp_domain::identity::ProfileSettings,
    pub grant: &'a crate::gatekeeper::GrantToken,
    pub allowance: ymp_domain::resources::Allowance,
    pub files: Option<std::sync::Arc<dyn InvocationFiles>>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExecutionHandle {
    pub invocation: ymp_domain::Id<ymp_domain::assignment::Invocation>,
    pub handle: ymp_domain::Id,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BackendStart {
    pub handle: ExecutionHandle,
    pub sent: ymp_domain::identity::ProfileSettings,
    pub reported: ymp_domain::identity::ProfileSettings,
    pub native_session: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BackendEvent {
    pub invocation: ymp_domain::Id<ymp_domain::assignment::Invocation>,
    pub sequence: u64,
    pub observation: BackendObservation,
}
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum BackendObservation {
    Output(String),
    Progress {
        signal: ymp_domain::coordination::ProgressSignal,
        basis: Option<ymp_domain::Ref>,
    },
    ToolDenied(ymp_domain::journal::Capability),
    OperationRequest {
        operation: ymp_domain::assignment::TeamOperation,
        args: String,
        correlation: String,
    },
    Usage {
        usage: ymp_domain::resources::Usage,
        turns: u32,
    },
    Terminal(ymp_domain::assignment::InvocationTerminal),
}
/// Calls may be supervised independently by the host. Cancellation requests never
/// certify cessation. Events use stable invocation-local sequences and cumulative
/// usage/turn counters; the cursor is observation, not permission to repeat work.
pub trait ExecutionBackend: Send + Sync {
    fn selection(&self) -> &PolicySelection;
    fn start(&self, request: &ExecutionRequest<'_>) -> Result<BackendStart>;
    fn cancel(&self, handle: &ExecutionHandle) -> Result<()>;
    fn events(
        &self,
        handle: &ExecutionHandle,
        after: u64,
        limit: usize,
    ) -> Result<Vec<BackendEvent>>;
    fn receipt(&self, handle: &ExecutionHandle) -> Result<ymp_domain::resources::Receipt>;
    fn reply(&self, _handle: &ExecutionHandle, _correlation: &str, _result: &str) -> Result<()> {
        Err(ymp_domain::Denial::new(
            "operation_reply_unsupported",
            "Backend does not expose an operation reply channel",
        ))
    }
}

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
