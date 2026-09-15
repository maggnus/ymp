#![forbid(unsafe_code)]

mod check;
pub mod clock;
pub mod codex;
mod scenario;
pub mod scripted;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

pub use ymp_domain::{
    AcceptanceContract, Check, CheckMethod, Constraints, Criterion, CriterionEvaluation,
    CriterionId, CriterionStatus, DomainError, Evidence, EvidenceFile, Goal, SessionId, Task,
    TaskId, VerifierDigest,
};
use ymp_kernel::Dispatcher;
pub use ymp_kernel::{
    AcceptanceAuthority, DispatchError, HistoryError, Journal, JournalEntry, JournalError,
    LifecycleEventKind, Revision, SessionEvent, SessionStatus, SessionView,
};

pub use check::{BuiltinCheckExecutor, CheckExecutionError, DEFAULT_CHECK_TIMEOUT};
#[cfg(target_os = "linux")]
pub use check::{run_linux_check_sandbox, run_linux_check_sandbox_stage2};
pub use clock::{Clock, ManualClock, SystemClock};
pub use codex::{CodexBackend, CodexProbe, CodexRegistry, CodexStreamStats, CodexTokenUsage};
pub use scenario::ExecutionScenario;
pub use scripted::{
    ScriptedBackend, ScriptedOutcome, ScriptedProvider, ScriptedReceiptSpec, ScriptedRegistry,
};
pub use ymp_kernel::execution::{
    AdmissionDenial, AdmissionFailure, AgentId, AgentIneligibility, Allowance, Assignment,
    AssignmentRequest, BackendCancelRefused, BackendInvocation, BackendStartFailure,
    CancellationOutcome, EmptyPoolReason, ErrorClass, EvidenceOutcome, ExclusionReason,
    ExecutionBackend, ExecutionError, ExecutionObservation, ExecutionProfile, Grant, GrantId,
    HostEnforcement, IndependenceConflict, InvocationId, InvocationLimits, InvocationStatus,
    InvocationView, LedgerTreasury, LiveAssignment, ModelOffering, ObservationAccumulation,
    ObservationOutcome, ObservedUsage, OfferingId, PolicyGatekeeper, Pool, PoolEligibility,
    PoolEntry, Receipt, Registry, RegistryFailure, ReservationPurpose, ReserveRefused,
    ResourceAmount, Role, SessionAccounting, SessionExecutionView, SettingKey, SettingValue,
    Settings, SettleRefused, StartOutcome, SupportedControl, Termination, TrackedWorkspaceGuard,
    Treasury, UncertaintyCause, UsageAggregate, WorkspaceAccess, WorkspaceAccessRefusal,
    WorkspaceGuard, WorkspaceHoldConflict, WorkspaceOperation, WorkspaceScope,
};

pub const APPLICATION_NAME: &str = "ymp";
pub const APPLICATION_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const CAPABILITY_LIMIT: &str = "A completed invocation is only an observed provider outcome.\nIt does not establish acceptance or confirmation of the requested work.";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ApplicationMetadata {
    name: &'static str,
    version: &'static str,
    capability_limit: &'static str,
}

impl ApplicationMetadata {
    pub const fn name(self) -> &'static str {
        self.name
    }

    pub const fn version(self) -> &'static str {
        self.version
    }

    pub const fn capability_limit(self) -> &'static str {
        self.capability_limit
    }
}

pub const fn application_metadata() -> ApplicationMetadata {
    ApplicationMetadata {
        name: APPLICATION_NAME,
        version: APPLICATION_VERSION,
        capability_limit: CAPABILITY_LIMIT,
    }
}

#[derive(Clone, Debug, Default)]
pub struct MemoryJournal {
    store: Arc<Mutex<HashMap<SessionId, Vec<JournalEntry>>>>,
}

impl MemoryJournal {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Journal for MemoryJournal {
    fn read(&self, session_id: &SessionId) -> Result<Vec<JournalEntry>, JournalError> {
        let store = self
            .store
            .lock()
            .map_err(|_| JournalError::AdapterFailure {
                message: "memory journal lock is poisoned".to_owned(),
            })?;
        Ok(store.get(session_id).cloned().unwrap_or_default())
    }

    fn append(
        &self,
        session_id: &SessionId,
        expected_revision: Revision,
        events: Vec<SessionEvent>,
    ) -> Result<Revision, JournalError> {
        if events.is_empty() {
            return Err(JournalError::EmptyBatch);
        }

        let mut store = self
            .store
            .lock()
            .map_err(|_| JournalError::AdapterFailure {
                message: "memory journal lock is poisoned".to_owned(),
            })?;
        let actual_revision = store
            .get(session_id)
            .and_then(|stream| stream.last())
            .map_or(Revision::INITIAL, JournalEntry::revision);
        if actual_revision != expected_revision {
            return Err(JournalError::StaleRevision {
                expected: expected_revision,
                actual: actual_revision,
            });
        }

        let mut next_revision = actual_revision;
        let mut entries = Vec::with_capacity(events.len());
        for event in events {
            next_revision = next_revision
                .checked_next()
                .ok_or(JournalError::RevisionOverflow)?;
            entries.push(JournalEntry::new(next_revision, event));
        }
        store.entry(session_id.clone()).or_default().extend(entries);
        Ok(next_revision)
    }
}

/// Application assembly over any [`Journal`] adapter, defaulting to the
/// in-memory assembly so existing type-position uses of `Application` keep
/// meaning `Application<MemoryJournal>`.
#[derive(Clone, Debug)]
pub struct Application<J = MemoryJournal> {
    dispatcher: Dispatcher<J>,
}

impl Application<MemoryJournal> {
    pub fn in_memory() -> Self {
        Self::new(MemoryJournal::new())
    }
}

impl<J> Application<J>
where
    J: Journal,
{
    pub fn new(journal: J) -> Self {
        Self {
            dispatcher: Dispatcher::new(journal),
        }
    }

    pub fn metadata(&self) -> ApplicationMetadata {
        application_metadata()
    }

    pub fn open_session(
        &self,
        session_id: SessionId,
        task: Task,
    ) -> Result<SessionView, DispatchError> {
        self.dispatcher.open(session_id, task)
    }

    pub fn read_session(&self, session_id: &SessionId) -> Result<SessionView, DispatchError> {
        self.dispatcher.read(session_id)
    }

    pub fn cancel_session(
        &self,
        session_id: &SessionId,
        expected_revision: Revision,
    ) -> Result<SessionView, DispatchError> {
        self.dispatcher.cancel(session_id, expected_revision)
    }
}

/// Reads the execution projection through the kernel without assembling
/// provider adapters. This is the read-only runtime facade used after
/// reopening a durable journal.
pub fn read_execution<J: Journal>(
    journal: &J,
    session_id: &SessionId,
) -> Result<SessionExecutionView, ExecutionError> {
    ymp_kernel::execution::read_execution(journal, session_id)
}

impl Default for Application<MemoryJournal> {
    fn default() -> Self {
        Self::in_memory()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn revision_overflow_leaves_the_memory_stream_unchanged() {
        let journal = MemoryJournal::new();
        let session_id = SessionId::new("overflow").expect("valid session ID");
        let existing = JournalEntry::new(
            Revision::new(u64::MAX),
            SessionEvent::SessionCancelled {
                session_id: session_id.clone(),
            },
        );
        journal
            .store
            .lock()
            .expect("memory journal lock is available")
            .insert(session_id.clone(), vec![existing.clone()]);

        let result = journal.append(
            &session_id,
            Revision::new(u64::MAX),
            vec![SessionEvent::SessionCancelled {
                session_id: session_id.clone(),
            }],
        );

        assert_eq!(result, Err(JournalError::RevisionOverflow));
        assert_eq!(
            journal.read(&session_id).expect("history reads"),
            vec![existing]
        );
    }
}
