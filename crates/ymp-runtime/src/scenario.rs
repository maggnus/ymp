//! Assembly of the admitted scenario over any [`Journal`] adapter.
//!
//! [`ExecutionScenario`] wires the kernel's execution ports around one
//! journal: discovery, admission, invocation, termination observation and
//! accounting all run through the kernel's orchestration functions, the
//! [`Dispatcher`] remains the session entry point, and the journal remains
//! the only durable record. Each step holds the shared ports for its whole
//! duration, which serializes concurrent callers; the journal's revision
//! check still decides which competing commit wins.
//!
//! The default backend is the [`ScriptedBackend`]: deterministic,
//! configuration-driven, with no real provider execution. Real provider
//! adapters are later tasks and must satisfy the same port.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use ymp_domain::{SessionId, Task};
use ymp_kernel::execution::{
    self, AdmissionContext, AdmissionFailure, Assignment, AssignmentRequest, CancellationContext,
    CancellationOutcome, EvidenceContext, EvidenceOutcome, ExecutionBackend, ExecutionError,
    InvocationId, LedgerTreasury, ObservationContext, ObservationOutcome, PolicyGatekeeper, Pool,
    Registry, RegistryFailure, ResourceAmount, SessionAccounting, SessionExecutionView,
    SettlementContext, StartContext, StartOutcome, TrackedWorkspaceGuard, Treasury, WorkspaceGuard,
    WorkspaceScope, observe_invocation, read_execution, record_effect_evidence,
    request_invocation_cancellation, scan_registry, settle_invocation, start_invocation,
};
use ymp_kernel::{DispatchError, Dispatcher, Journal, Revision, SessionView};

use crate::clock::Clock;
use crate::scripted::{ScriptedBackend, ScriptedProvider, ScriptedRegistry};

/// The admitted scenario assembled over one journal.
///
/// All port state (reservations, workspace holds, start times) is in-memory
/// projection of committed journal events; [`ExecutionScenario::reattach`]
/// rebuilds the reservations and holds for one session after reopening a
/// durable journal.
pub struct ExecutionScenario<J, B = ScriptedBackend>
where
    J: Journal + Clone,
    B: ExecutionBackend,
{
    journal: J,
    dispatcher: Dispatcher<J>,
    registry: Mutex<ScriptedRegistry>,
    treasury: Mutex<LedgerTreasury>,
    workspace: Mutex<TrackedWorkspaceGuard>,
    backend: Mutex<B>,
    gatekeeper: PolicyGatekeeper,
    clock: Arc<dyn Clock + Send + Sync>,
    started_at: Mutex<BTreeMap<InvocationId, std::time::Duration>>,
}

impl<J, B> ExecutionScenario<J, B>
where
    J: Journal + Clone,
    B: ExecutionBackend,
{
    /// Full wiring over caller-chosen adapters.
    pub fn over(
        journal: J,
        backend: B,
        enforceable_scopes: impl IntoIterator<Item = WorkspaceScope>,
        registry: ScriptedRegistry,
        capacity: ResourceAmount,
        clock: Arc<dyn Clock + Send + Sync>,
    ) -> Self {
        Self {
            dispatcher: Dispatcher::new(journal.clone()),
            journal,
            registry: Mutex::new(registry),
            treasury: Mutex::new(LedgerTreasury::new(capacity)),
            workspace: Mutex::new(TrackedWorkspaceGuard::new(enforceable_scopes)),
            backend: Mutex::new(backend),
            gatekeeper: PolicyGatekeeper,
            clock,
            started_at: Mutex::new(BTreeMap::new()),
        }
    }

    /// The dispatcher remains the session entry point.
    pub fn dispatcher(&self) -> &Dispatcher<J> {
        &self.dispatcher
    }

    /// Runs one explicit discovery scan.
    pub fn scan(&self) -> Result<Pool, RegistryFailure> {
        let mut registry = self.registry.lock().expect("registry lock is available");
        scan_registry(&mut *registry)
    }

    /// The current pool snapshot; never scans.
    pub fn pool(&self) -> Pool {
        let registry = self.registry.lock().expect("registry lock is available");
        registry.pool()
    }

    /// Opens a session through the dispatcher.
    pub fn open_session(
        &self,
        session_id: SessionId,
        task: Task,
    ) -> Result<SessionView, DispatchError> {
        self.dispatcher.open(session_id, task)
    }

    /// Reads a session through the dispatcher.
    pub fn read_session(&self, session_id: &SessionId) -> Result<SessionView, DispatchError> {
        self.dispatcher.read(session_id)
    }

    /// Admits one assignment at the expected revision.
    pub fn admit(
        &self,
        session_id: &SessionId,
        request: AssignmentRequest,
        expected_revision: Revision,
    ) -> Result<Assignment, AdmissionFailure> {
        let pool = self.pool();
        let mut treasury = self.treasury.lock().expect("treasury lock is available");
        let mut workspace = self.workspace.lock().expect("workspace lock is available");
        let context = AdmissionContext {
            journal: &self.journal,
            session_id,
            expected_revision,
            pool: &pool,
            gatekeeper: &self.gatekeeper,
            treasury: &mut *treasury,
            workspace: &mut *workspace,
        };
        execution::admit_assignment(context, request)
    }

    /// Starts one admitted invocation with the settings recorded in the
    /// admission batch. Only the invocation ID is the input; every executed
    /// parameter comes from the journaled assignment.
    pub fn invoke(
        &self,
        session_id: &SessionId,
        invocation: &InvocationId,
        expected_revision: Revision,
    ) -> Result<StartOutcome, ExecutionError> {
        let mut backend = self.backend.lock().expect("backend lock is available");
        let mut treasury = self.treasury.lock().expect("treasury lock is available");
        let mut workspace = self.workspace.lock().expect("workspace lock is available");
        let context = StartContext {
            journal: &self.journal,
            backend: &mut *backend,
            session_id,
            expected_revision,
            invocation,
            treasury: &mut *treasury,
            workspace: &mut *workspace,
        };
        let outcome = start_invocation(context)?;
        if matches!(outcome, StartOutcome::Started) {
            self.started_at
                .lock()
                .expect("start-time lock is available")
                .insert(invocation.clone(), self.clock.elapsed());
        }
        Ok(outcome)
    }

    /// Requests cancellation of one started invocation.
    pub fn cancel(
        &self,
        session_id: &SessionId,
        invocation: &InvocationId,
        expected_revision: Revision,
    ) -> Result<CancellationOutcome, ExecutionError> {
        let mut backend = self.backend.lock().expect("backend lock is available");
        let context = CancellationContext {
            journal: &self.journal,
            backend: &mut *backend,
            session_id,
            expected_revision,
            invocation,
        };
        request_invocation_cancellation(context)
    }

    /// Observes one in-flight invocation, applying host-enforced wall-clock
    /// timeouts against the scenario clock.
    pub fn observe(
        &self,
        session_id: &SessionId,
        invocation: &InvocationId,
        expected_revision: Revision,
    ) -> Result<ObservationOutcome, ExecutionError> {
        let started_at = *self
            .started_at
            .lock()
            .expect("start-time lock is available")
            .get(invocation)
            .ok_or_else(|| ExecutionError::InvocationNotStarted {
                invocation: invocation.clone(),
            })?;
        let now = self.clock.elapsed();

        let mut backend = self.backend.lock().expect("backend lock is available");
        let mut treasury = self.treasury.lock().expect("treasury lock is available");
        let mut workspace = self.workspace.lock().expect("workspace lock is available");
        let context = ObservationContext {
            journal: &self.journal,
            backend: &mut *backend,
            session_id,
            expected_revision,
            invocation,
            started_at,
            now,
            treasury: &mut *treasury,
            workspace: &mut *workspace,
        };
        let outcome = observe_invocation(context)?;
        if matches!(outcome, ObservationOutcome::Terminated { .. }) {
            self.started_at
                .lock()
                .expect("start-time lock is available")
                .remove(invocation);
        }
        Ok(outcome)
    }

    /// Settles one terminated invocation: the reservation is released
    /// against the observed usage. A failed settlement append returns a
    /// typed error, keeps the reservation held, and is retried by calling
    /// again.
    pub fn settle(
        &self,
        session_id: &SessionId,
        invocation: &InvocationId,
        expected_revision: Revision,
    ) -> Result<ResourceAmount, ExecutionError> {
        let mut treasury = self.treasury.lock().expect("treasury lock is available");
        let context = SettlementContext {
            journal: &self.journal,
            session_id,
            expected_revision,
            invocation,
            treasury: &mut *treasury,
        };
        settle_invocation(context)
    }

    /// Records effect evidence for one invocation: a writes-ended
    /// observation on its own stream releases the workspace hold and lifts
    /// the successor bar in one append, while the invocation itself remains
    /// `uncertain` with its reservation held.
    pub fn evidence(
        &self,
        session_id: &SessionId,
        invocation: &InvocationId,
        expected_revision: Revision,
    ) -> Result<EvidenceOutcome, ExecutionError> {
        let mut backend = self.backend.lock().expect("backend lock is available");
        let mut workspace = self.workspace.lock().expect("workspace lock is available");
        let context = EvidenceContext {
            journal: &self.journal,
            backend: &mut *backend,
            session_id,
            expected_revision,
            invocation,
            workspace: &mut *workspace,
        };
        record_effect_evidence(context)
    }

    /// The replayed execution view of one session.
    pub fn execution_view(
        &self,
        session_id: &SessionId,
    ) -> Result<SessionExecutionView, ExecutionError> {
        read_execution(&self.journal, session_id)
    }

    /// The session accounting replayed from the journal.
    pub fn accounting(&self, session_id: &SessionId) -> Result<SessionAccounting, ExecutionError> {
        Ok(self.execution_view(session_id)?.accounting().clone())
    }

    /// Runs one read-only inspection over the backend adapter.
    pub fn with_backend<R>(&self, inspect: impl FnOnce(&B) -> R) -> R {
        let backend = self.backend.lock().expect("backend lock is available");
        inspect(&backend)
    }

    /// Runs one mutation over the backend adapter: scripted-provider
    /// control for delivering deferred observations in tests.
    pub fn with_backend_mut<R>(&self, mutate: impl FnOnce(&mut B) -> R) -> R {
        let mut backend = self.backend.lock().expect("backend lock is available");
        mutate(&mut backend)
    }

    /// The reservations currently held by the treasury.
    pub fn treasury_held(&self) -> ResourceAmount {
        let treasury = self.treasury.lock().expect("treasury lock is available");
        treasury.held()
    }

    /// The workspace scope one invocation currently holds, if any.
    pub fn workspace_hold_of(&self, invocation: &InvocationId) -> Option<WorkspaceScope> {
        let workspace = self.workspace.lock().expect("workspace lock is available");
        workspace.hold_of(invocation)
    }

    /// The elapsed time the scenario clock reports.
    pub fn clock_elapsed(&self) -> std::time::Duration {
        self.clock.elapsed()
    }

    /// Rebuilds the in-memory reservations and workspace holds for one
    /// session from its journaled history. Start times are not durable, so
    /// wall-clock adjudication of invocations that were in flight across a
    /// restart begins from the reattachment, not from their original start.
    pub fn reattach(&self, session_id: &SessionId) -> Result<(), ExecutionError> {
        let view = read_execution(&self.journal, session_id)?;
        let mut treasury_holds = Vec::new();
        let mut workspace_holds = Vec::new();
        for (invocation, invocation_view) in view.invocations() {
            if invocation_view.holds_reservation() {
                treasury_holds.push((
                    invocation.clone(),
                    invocation_view.assignment().grant().reservation(),
                ));
                workspace_holds.push((
                    invocation.clone(),
                    invocation_view.assignment().workspace().clone(),
                ));
            }
        }
        self.treasury
            .lock()
            .expect("treasury lock is available")
            .rebuild(treasury_holds);
        self.workspace
            .lock()
            .expect("workspace lock is available")
            .rebuild_holds(workspace_holds);
        Ok(())
    }
}

impl<J> ExecutionScenario<J, ScriptedBackend>
where
    J: Journal + Clone,
{
    /// The admitted scenario over the scripted provider and the real clock.
    pub fn new(journal: J, provider: ScriptedProvider, capacity: ResourceAmount) -> Self {
        Self::with_clock(
            journal,
            provider,
            capacity,
            Arc::new(crate::clock::SystemClock::new()),
        )
    }

    /// The admitted scenario over the scripted provider and a chosen clock.
    pub fn with_clock(
        journal: J,
        provider: ScriptedProvider,
        capacity: ResourceAmount,
        clock: Arc<dyn Clock + Send + Sync>,
    ) -> Self {
        let backend = ScriptedBackend::for_provider(&provider);
        let scopes = provider.effective_workspaces().to_vec();
        Self::over(
            journal,
            backend,
            scopes,
            ScriptedRegistry::new([provider]),
            capacity,
            clock,
        )
    }

    /// Queues one scripted outcome for the next started invocation.
    pub fn script_outcome(&self, outcome: crate::scripted::ScriptedOutcome) {
        self.backend
            .lock()
            .expect("backend lock is available")
            .push_outcome(outcome);
    }
}
