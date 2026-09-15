//! Acceptance tests of the bounded native execution slice (stage 3).
//!
//! Each test below carries the verbatim title of one acceptance criterion
//! from `ymp-docs/bounded-execution-contract.md`; the snake_case test name
//! maps to it one-to-one. Every check runs against scripted in-process
//! adapters: discovery is simulated by the scripted scan, and no check
//! accesses the real native environment, credentials, network or user data.
//! Nothing in this slice spawns a process; the scripted adapters import no
//! process, network or credential API, which this header documents as the
//! structural guarantee behind criterion 8.

#![forbid(unsafe_code)]

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Barrier, Mutex};
use std::thread;
use std::time::Duration;

use ymp_runtime::{
    AcceptanceContract, AdmissionDenial, AdmissionFailure, AgentId, AgentIneligibility, Allowance,
    AssignmentRequest, Constraints, Criterion, CriterionId, EmptyPoolReason, ExecutionError,
    ExecutionObservation, ExecutionScenario, Goal, IndependenceConflict, InvocationLimits,
    InvocationStatus, Journal, JournalError, ManualClock, MemoryJournal, ModelOffering,
    ObservationOutcome, ObservedUsage, OfferingId, ResourceAmount, Revision, Role, ScriptedBackend,
    ScriptedOutcome, ScriptedProvider, ScriptedRegistry, SessionEvent, SessionId, SessionStatus,
    SettingKey, SettingValue, Settings, StartOutcome, SupportedControl, Task, TaskId, Termination,
    UncertaintyCause, WorkspaceAccessRefusal, WorkspaceScope,
};

const AGENT: &str = "claude-opus-5";
const OTHER_AGENT: &str = "codex-opus-5";
const SCOPE: &str = "session-primary";

fn session(value: &str) -> SessionId {
    SessionId::new(value).expect("valid session ID")
}

fn task() -> Task {
    Task::new(
        TaskId::new("task-execution-slice").expect("valid task ID"),
        Goal::new("Produce one admitted invocation").expect("valid goal"),
        AcceptanceContract::new(vec![
            Criterion::new(
                CriterionId::new("scripted").expect("valid criterion ID"),
                "The scripted lifecycle is observed.",
            )
            .expect("valid criterion"),
        ])
        .expect("valid acceptance contract"),
        Constraints::new(vec!["scripted adapters only".to_owned()]).expect("valid constraints"),
    )
}

fn setting_key(name: &str) -> SettingKey {
    SettingKey::new(name).expect("valid setting key")
}

fn setting_value(name: &str) -> SettingValue {
    SettingValue::new(name).expect("valid setting value")
}

fn settings(pairs: &[(&str, &str)]) -> Settings {
    Settings::from_pairs(
        pairs
            .iter()
            .map(|(key, value)| (setting_key(key), setting_value(value))),
    )
    .expect("valid settings")
}

fn offering() -> ModelOffering {
    ModelOffering::new(
        OfferingId::new("scripted-offering").expect("valid offering ID"),
        vec![
            SupportedControl::new(
                setting_key("effort"),
                vec![setting_value("low"), setting_value("high")],
            ),
            SupportedControl::new(
                setting_key("thinking"),
                vec![setting_value("on"), setting_value("off")],
            ),
        ],
    )
    .expect("valid offering")
}

fn scope() -> WorkspaceScope {
    WorkspaceScope::new(SCOPE).expect("valid workspace scope")
}

fn agent() -> AgentId {
    AgentId::new(AGENT).expect("valid agent ID")
}

fn other_agent() -> AgentId {
    AgentId::new(OTHER_AGENT).expect("valid agent ID")
}

fn review_scope() -> WorkspaceScope {
    WorkspaceScope::new("session-review").expect("valid workspace scope")
}

fn provider() -> ScriptedProvider {
    ScriptedProvider::new(agent(), offering()).with_effective_workspaces([scope(), review_scope()])
}

fn other_provider() -> ScriptedProvider {
    let offering = ModelOffering::new(
        OfferingId::new("scripted-offering-codex").expect("valid offering ID"),
        vec![
            SupportedControl::new(
                setting_key("effort"),
                vec![setting_value("low"), setting_value("high")],
            ),
            SupportedControl::new(
                setting_key("thinking"),
                vec![setting_value("on"), setting_value("off")],
            ),
        ],
    )
    .expect("valid offering");
    ScriptedProvider::new(other_agent(), offering)
        .with_effective_workspaces([scope(), review_scope()])
}

fn providers() -> Vec<ScriptedProvider> {
    vec![provider(), other_provider()]
}

fn limits() -> InvocationLimits {
    InvocationLimits::new(6, 2048, Duration::from_millis(60_000)).expect("valid limits")
}

fn tight_limits() -> InvocationLimits {
    InvocationLimits::new(3, 100, Duration::from_millis(60_000)).expect("valid limits")
}

fn allowance(reservation: u64) -> Allowance {
    Allowance::new(ResourceAmount::new(reservation), limits()).expect("valid allowance")
}

fn tight_allowance() -> Allowance {
    Allowance::new(ResourceAmount::new(4), tight_limits()).expect("valid allowance")
}

fn request_for(
    agent: AgentId,
    role: &str,
    workspace: WorkspaceScope,
    reservation: u64,
    requested: Settings,
) -> AssignmentRequest {
    AssignmentRequest::new(
        agent,
        Role::new(role).expect("valid role"),
        requested,
        allowance(reservation),
        workspace,
    )
}

fn default_request() -> AssignmentRequest {
    request_for(agent(), "implementer", scope(), 4, default_settings())
}

fn default_settings() -> Settings {
    settings(&[("effort", "high")])
}

/// A scenario over its own memory journal, so raw journal events stay
/// assertable next to the scenario's own views.
fn scenario_multi(journal: MemoryJournal, clock: ManualClock) -> ExecutionScenario<MemoryJournal> {
    let providers = providers();
    let backend = ScriptedBackend::for_provider(&providers[0]);
    ExecutionScenario::over(
        journal,
        backend,
        [scope(), review_scope()],
        ScriptedRegistry::new(providers),
        ResourceAmount::new(10),
        Arc::new(clock),
    )
}

fn scenario_with(clock: ManualClock) -> (MemoryJournal, ExecutionScenario<MemoryJournal>) {
    let journal = MemoryJournal::new();
    let scenario = scenario_multi(journal.clone(), clock);
    (journal, scenario)
}

/// A scenario over a caller-provided journal with tight limits and the
/// scripted provider.
fn scenario_over<J>(journal: J, clock: ManualClock) -> ExecutionScenario<J>
where
    J: ymp_runtime::Journal + Clone,
{
    let providers = providers();
    let backend = ScriptedBackend::for_provider(&providers[0]);
    ExecutionScenario::over(
        journal,
        backend,
        [scope(), review_scope()],
        ScriptedRegistry::new(providers),
        ResourceAmount::new(10),
        Arc::new(clock),
    )
}

/// Opens a session and runs one discovery scan.
fn prepared(clock: ManualClock) -> (MemoryJournal, ExecutionScenario<MemoryJournal>, SessionId) {
    let (journal, scenario) = scenario_with(clock);
    let id = session("admitted-scenario");
    let opened = scenario
        .open_session(id.clone(), task())
        .expect("session opens");
    assert_eq!(opened.status(), SessionStatus::Open);
    assert_eq!(opened.revision(), Revision::new(1));
    scenario.scan().expect("scripted scan succeeds");
    (journal, scenario, id)
}

/// "Scripted backend lifecycle: one invocation passes start, observation and
/// each typed termination — completed, failed with error class, cancelled
/// and timed out — while the `ExecutionProfile` retains requested, sent and
/// reported settings separately."
#[test]
fn scripted_backend_lifecycle_end_to_end() {
    // Completed: reported settings differ from requested and sent; all
    // three meanings survive beside each other.
    let (journal, scenario, id) = prepared(ManualClock::new());
    scenario.script_outcome(ScriptedOutcome::completes(
        Some(settings(&[("effort", "low")])),
        ObservedUsage::unknown()
            .with_turns(4)
            .with_output_chars(1200)
            .with_wall_clock(Duration::from_millis(900)),
    ));
    let assignment = scenario
        .admit(&id, default_request(), Revision::new(1))
        .expect("admission commits");
    assert_eq!(assignment.invocation().as_str(), "invocation-1");
    assert_eq!(assignment.grant().id().as_str(), "grant-1");
    assert_eq!(assignment.grant().reservation().value(), 4);
    assert_eq!(assignment.role().as_str(), "implementer");
    assert_eq!(scenario.treasury_held().value(), 4);
    assert_eq!(
        scenario.workspace_hold_of(assignment.invocation()),
        Some(scope())
    );

    assert_eq!(
        scenario
            .invoke(&id, assignment.invocation(), Revision::new(2))
            .expect("invocation starts"),
        StartOutcome::Started
    );
    match scenario
        .observe(&id, assignment.invocation(), Revision::new(3))
        .expect("observation succeeds")
    {
        ObservationOutcome::Terminated {
            termination,
            reported_settings,
            ..
        } => {
            assert_eq!(termination, Termination::Completed);
            assert_eq!(reported_settings, Some(settings(&[("effort", "low")])));
        }
        other => panic!("expected a completed termination, got {other:?}"),
    }
    scenario
        .settle(&id, assignment.invocation(), Revision::new(4))
        .expect("settlement commits");

    let view = scenario.execution_view(&id).expect("view replays");
    let invocation = view
        .invocation(assignment.invocation())
        .expect("invocation");
    assert_eq!(invocation.status(), InvocationStatus::Terminated);
    assert_eq!(invocation.termination(), Some(&Termination::Completed));
    let profile = invocation.profile();
    assert_eq!(
        profile.requested().get(&setting_key("effort")),
        Some(&setting_value("high"))
    );
    assert_eq!(
        profile.sent().get(&setting_key("effort")),
        Some(&setting_value("high"))
    );
    // The backend says it used `low`; the report never overwrites the
    // requested or sent settings.
    assert_eq!(
        profile
            .reported()
            .expect("reported settings exist")
            .get(&setting_key("effort")),
        Some(&setting_value("low"))
    );
    assert_eq!(scenario.accounting(&id).expect("accounting").completed(), 1);
    assert_eq!(scenario.treasury_held().value(), 0);
    assert!(
        scenario
            .workspace_hold_of(assignment.invocation())
            .is_none()
    );
    assert_eq!(journal.read(&id).expect("history reads").len(), 5);

    // Failed with an error class, reported through the receipt.
    let (_, scenario, id) = prepared(ManualClock::new());
    scenario.script_outcome(ScriptedOutcome::fails(
        "provider_overloaded",
        ObservedUsage::unknown(),
    ));
    let assignment = scenario
        .admit(&id, default_request(), Revision::new(1))
        .expect("admission commits");
    assert_eq!(
        scenario
            .invoke(&id, assignment.invocation(), Revision::new(2))
            .expect("invocation starts"),
        StartOutcome::Started
    );
    match scenario
        .observe(&id, assignment.invocation(), Revision::new(3))
        .expect("observation succeeds")
    {
        ObservationOutcome::Terminated {
            termination: Termination::Failed { class },
            ..
        } => assert_eq!(class.as_str(), "provider_overloaded"),
        other => panic!("expected a failed termination, got {other:?}"),
    }
    scenario
        .settle(&id, assignment.invocation(), Revision::new(4))
        .expect("settlement commits");
    assert_eq!(scenario.accounting(&id).expect("accounting").failed(), 1);

    // Cancelled with confirmation.
    let (journal, scenario, id) = prepared(ManualClock::new());
    scenario.script_outcome(ScriptedOutcome::cancelled_with_confirmation(
        ObservedUsage::unknown().with_turns(1),
    ));
    let assignment = scenario
        .admit(&id, default_request(), Revision::new(1))
        .expect("admission commits");
    scenario
        .invoke(&id, assignment.invocation(), Revision::new(2))
        .expect("invocation starts");
    let cancellation = scenario
        .cancel(&id, assignment.invocation(), Revision::new(3))
        .expect("cancellation request records");
    assert!(cancellation.backend_acknowledged());
    assert_eq!(
        cancellation.invocation_phase(),
        InvocationStatus::Cancelling
    );
    match scenario
        .observe(&id, assignment.invocation(), Revision::new(4))
        .expect("observation succeeds")
    {
        ObservationOutcome::Terminated {
            termination: Termination::Cancelled,
            ..
        } => {}
        other => panic!("expected a cancelled termination, got {other:?}"),
    }
    scenario
        .settle(&id, assignment.invocation(), Revision::new(5))
        .expect("settlement commits");
    assert_eq!(scenario.accounting(&id).expect("accounting").cancelled(), 1);
    assert_eq!(journal.read(&id).expect("history reads").len(), 6);

    // Timed out: only a termination observation confirms the expiry.
    let (_, scenario, id) = prepared(ManualClock::new());
    scenario.script_outcome(ScriptedOutcome::times_out(
        ObservedUsage::unknown().with_turns(2),
    ));
    let assignment = scenario
        .admit(&id, default_request(), Revision::new(1))
        .expect("admission commits");
    scenario
        .invoke(&id, assignment.invocation(), Revision::new(2))
        .expect("invocation starts");
    match scenario
        .observe(&id, assignment.invocation(), Revision::new(3))
        .expect("observation succeeds")
    {
        ObservationOutcome::Terminated {
            termination: Termination::TimedOut,
            usage,
            ..
        } => assert_eq!(usage.turns(), Some(2)),
        other => panic!("expected a timed-out termination, got {other:?}"),
    }
    scenario
        .settle(&id, assignment.invocation(), Revision::new(4))
        .expect("settlement commits");
    assert_eq!(scenario.accounting(&id).expect("accounting").timed_out(), 1);
}

/// "Typed admission denials: ineligible agent, unsupported settings,
/// assignment not independent, resources unavailable, workspace access not
/// enforceable and stale revision each return their own failure, append
/// nothing and leave journal, revision, grants and reservations unchanged."
#[test]
fn typed_admission_denials_each_observable() {
    // Ineligible: the agent was never discovered (no scan has run).
    let (journal, scenario) = scenario_with(ManualClock::new());
    let id = session("ineligible-not-discovered");
    scenario
        .open_session(id.clone(), task())
        .expect("session opens");
    assert_eq!(
        scenario.pool().empty_reason(),
        Some(EmptyPoolReason::NotScanned)
    );
    assert!(matches!(
        scenario
            .admit(&id, default_request(), Revision::new(1))
            .unwrap_err(),
        AdmissionFailure::Denied(AdmissionDenial::IneligibleAgent {
            ineligibility: AgentIneligibility::NotDiscovered,
            ..
        })
    ));
    assert_eq!(journal.read(&id).expect("history reads").len(), 1);

    // Ineligible: the scan excluded the agent (not ready).
    let journal = MemoryJournal::new();
    let unready = ScriptedProvider::new(agent(), offering())
        .with_readiness(false, "the scripted probe reports not ready")
        .with_effective_workspaces([scope()]);
    let scenario = ExecutionScenario::with_clock(
        journal.clone(),
        unready,
        ResourceAmount::new(10),
        Arc::new(ManualClock::new()),
    );
    let id = session("ineligible-excluded");
    scenario
        .open_session(id.clone(), task())
        .expect("session opens");
    let pool = scenario.scan().expect("scan succeeds");
    assert_eq!(pool.entries().len(), 1);
    assert!(pool.entries()[0].exclusion().is_some());
    assert!(matches!(
        scenario
            .admit(&id, default_request(), Revision::new(1))
            .unwrap_err(),
        AdmissionFailure::Denied(AdmissionDenial::IneligibleAgent {
            ineligibility: AgentIneligibility::Excluded(_),
            ..
        })
    ));
    assert_eq!(journal.read(&id).expect("history reads").len(), 1);

    // The remaining denials share one prepared session; each must append
    // nothing and leave revision, grants and reservations unchanged.
    let (journal, scenario, id) = prepared(ManualClock::new());
    let journal_len = || journal.read(&id).expect("history reads").len();
    let held = || scenario.treasury_held().value();

    // Unsupported settings: the value is outside the reported controls.
    let request = request_for(
        agent(),
        "implementer",
        scope(),
        4,
        settings(&[("effort", "ultra"), ("thinking", "on")]),
    );
    assert!(matches!(
        scenario.admit(&id, request, Revision::new(1)).unwrap_err(),
        AdmissionFailure::Denied(AdmissionDenial::UnsupportedSettings {
            unsupported,
            ..
        }) if unsupported == vec![setting_key("effort")]
    ));
    assert_eq!(journal_len(), 1);
    assert_eq!(held(), 0);

    // Assignment not independent: the agent already holds a live
    // assignment.
    scenario.script_outcome(ScriptedOutcome::never_reports());
    let live = scenario
        .admit(&id, default_request(), Revision::new(1))
        .expect("first admission commits");
    assert_eq!(held(), 4);
    let same_agent = default_request();
    assert!(matches!(
        scenario.admit(&id, same_agent, Revision::new(2)).unwrap_err(),
        AdmissionFailure::Denied(AdmissionDenial::AssignmentNotIndependent {
            conflict: IndependenceConflict::AgentHasLiveAssignment { invocation },
            ..
        }) if invocation == *live.invocation()
    ));
    // The role is already active, even for a different agent.
    let same_role = request_for(other_agent(), "implementer", scope(), 4, default_settings());
    assert!(matches!(
        scenario.admit(&id, same_role, Revision::new(2)).unwrap_err(),
        AdmissionFailure::Denied(AdmissionDenial::AssignmentNotIndependent {
            conflict: IndependenceConflict::RoleAlreadyActive { invocation },
            ..
        }) if invocation == *live.invocation()
    ));
    // Resources unavailable: the reservation exceeds what the treasury can
    // hold beside the live assignment.
    let request = request_for(
        other_agent(),
        "researcher",
        review_scope(),
        7, // 4 already held of 10 capacity
        default_settings(),
    );
    assert!(matches!(
        scenario.admit(&id, request, Revision::new(2)).unwrap_err(),
        AdmissionFailure::Denied(AdmissionDenial::ResourcesUnavailable {
            requested,
            available,
        }) if requested.value() == 7 && available.value() == 6
    ));
    assert_eq!(journal_len(), 2);
    assert_eq!(held(), 4);

    // Workspace access not enforceable: the backend does not state
    // effective access to the requested scope.
    let request = request_for(
        other_agent(),
        "researcher",
        WorkspaceScope::new("undeclared-scope").expect("valid scope"),
        1,
        default_settings(),
    );
    assert!(matches!(
        scenario.admit(&id, request, Revision::new(2)).unwrap_err(),
        AdmissionFailure::Denied(AdmissionDenial::WorkspaceNotEnforceable {
            refusal: WorkspaceAccessRefusal::NotEnforceable { .. },
            ..
        })
    ));

    // Stale revision: the expected revision is not the current one.
    assert!(matches!(
        scenario
            .admit(&id, default_request(), Revision::new(1))
            .unwrap_err(),
        AdmissionFailure::Denied(AdmissionDenial::StaleRevision {
            expected,
            actual,
        }) if expected == Revision::new(1) && actual == Revision::new(2)
    ));

    // A different agent with a different role on a free scope stays
    // independent and commits; every denial above appended nothing.
    let independent = request_for(
        other_agent(),
        "reviewer",
        review_scope(),
        4,
        default_settings(),
    );
    let second = scenario
        .admit(&id, independent, Revision::new(2))
        .expect("an independent assignment commits");
    assert_eq!(second.invocation().as_str(), "invocation-2");
    assert_eq!(held(), 8);
    assert_eq!(journal_len(), 3);
    assert_eq!(
        scenario
            .execution_view(&id)
            .expect("view")
            .invocation_count(),
        2
    );
}

/// A journal wrapper that resolves one admission append indeterminately, in
/// one of three modes: the batch is committed but reported unproven; the
/// batch is absent and reported unproven; or a competing event advances the
/// history before the resolution read.
#[derive(Clone)]
struct IndeterminateJournal {
    inner: MemoryJournal,
    mode: IndeterminateMode,
    fired: Arc<AtomicBool>,
    competing_done: Arc<AtomicBool>,
    pending_expected: Arc<Mutex<Option<Revision>>>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum IndeterminateMode {
    Committed,
    Absent,
    CompetingAdvances,
}

impl IndeterminateJournal {
    fn new(mode: IndeterminateMode) -> Self {
        Self {
            inner: MemoryJournal::new(),
            mode,
            fired: Arc::new(AtomicBool::new(false)),
            competing_done: Arc::new(AtomicBool::new(false)),
            pending_expected: Arc::new(Mutex::new(None)),
        }
    }
}

impl Journal for IndeterminateJournal {
    fn read(&self, session_id: &SessionId) -> Result<Vec<ymp_runtime::JournalEntry>, JournalError> {
        if self.mode == IndeterminateMode::CompetingAdvances
            && self.fired.load(Ordering::SeqCst)
            && !self.competing_done.swap(true, Ordering::SeqCst)
        {
            let expected = self
                .pending_expected
                .lock()
                .expect("pending revision lock")
                .take()
                .expect("expected revision recorded");
            self.inner
                .append(
                    session_id,
                    expected,
                    vec![SessionEvent::SessionCancelled {
                        session_id: session_id.clone(),
                    }],
                )
                .expect("competing append lands");
        }
        self.inner.read(session_id)
    }

    fn append(
        &self,
        session_id: &SessionId,
        expected_revision: Revision,
        events: Vec<SessionEvent>,
    ) -> Result<Revision, JournalError> {
        let admission = events
            .iter()
            .any(|event| matches!(event, SessionEvent::AssignmentAdmitted { .. }));
        if !admission || self.fired.swap(true, Ordering::SeqCst) {
            return self.inner.append(session_id, expected_revision, events);
        }
        match self.mode {
            IndeterminateMode::Committed => {
                let attempted = self
                    .inner
                    .append(session_id, expected_revision, events)
                    .expect("batch commits before the fault");
                Err(JournalError::IndeterminateCommit {
                    expected: expected_revision,
                    attempted,
                    message: "injected unproven commit (committed)".to_owned(),
                })
            }
            IndeterminateMode::Absent | IndeterminateMode::CompetingAdvances => {
                if self.mode == IndeterminateMode::CompetingAdvances {
                    *self.pending_expected.lock().expect("pending revision lock") =
                        Some(expected_revision);
                }
                Err(JournalError::IndeterminateCommit {
                    expected: expected_revision,
                    attempted: expected_revision
                        .checked_next()
                        .expect("attempted revision fits"),
                    message: "injected unproven commit (absent)".to_owned(),
                })
            }
        }
    }
}

/// A journal wrapper that fails the first settlement append with a typed
/// adapter failure, then passes through.
#[derive(Clone)]
struct FlakySettlementJournal {
    inner: MemoryJournal,
    fired: Arc<AtomicBool>,
}

impl FlakySettlementJournal {
    fn new() -> Self {
        Self {
            inner: MemoryJournal::new(),
            fired: Arc::new(AtomicBool::new(false)),
        }
    }
}

impl Journal for FlakySettlementJournal {
    fn read(&self, session_id: &SessionId) -> Result<Vec<ymp_runtime::JournalEntry>, JournalError> {
        self.inner.read(session_id)
    }

    fn append(
        &self,
        session_id: &SessionId,
        expected_revision: Revision,
        events: Vec<SessionEvent>,
    ) -> Result<Revision, JournalError> {
        let settlement = events
            .iter()
            .any(|event| matches!(event, SessionEvent::InvocationAccounted { .. }));
        if settlement && !self.fired.swap(true, Ordering::SeqCst) {
            return Err(JournalError::AdapterFailure {
                message: "injected settlement append failure".to_owned(),
            });
        }
        self.inner.append(session_id, expected_revision, events)
    }
}

/// "Atomic admission: assignment, grant, allowance, reservation and sent
/// settings land as one batch at one revision, `start` passes exactly the
/// recorded sent settings, of two admissions at one expected revision
/// exactly one commits with the other stale and no grant or reservation,
/// and an indeterminate commit is resolved by the durable-journal rule
/// before any `start`."
#[test]
fn atomic_admission_one_batch_recorded_sent_and_one_winner() {
    // The admission batch is exactly one event carrying assignment, grant,
    // allowance, reservation and the resolved sent settings.
    let (journal, scenario, id) = prepared(ManualClock::new());
    scenario.script_outcome(ScriptedOutcome::never_reports());
    let assignment = scenario
        .admit(&id, default_request(), Revision::new(1))
        .expect("admission commits");
    let history = journal.read(&id).expect("history reads");
    assert_eq!(history.len(), 2);
    match history[1].event() {
        SessionEvent::AssignmentAdmitted { invocation, .. } => {
            assert_eq!(*invocation, *assignment.invocation());
        }
        other => panic!("expected one admission event, got {other:?}"),
    }
    assert_eq!(assignment.requested_settings(), &default_settings());
    assert_eq!(assignment.sent_settings(), &default_settings());
    assert_eq!(assignment.grant().reservation().value(), 4);
    assert_eq!(assignment.allowance().limits().max_turns(), 6);

    // `start` passes exactly the recorded sent settings.
    scenario
        .invoke(&id, assignment.invocation(), Revision::new(2))
        .expect("invocation starts");
    let started = scenario
        .with_backend(BackendAccess::last_started)
        .expect("a start was accepted");
    assert_eq!(started.sent_settings(), assignment.sent_settings());
    assert_eq!(started.invocation(), assignment.invocation());

    // Two admissions at one expected revision: exactly one commits.
    let (journal, scenario, id) = prepared(ManualClock::new());
    let scenario = Arc::new(scenario);
    let barrier = Arc::new(Barrier::new(2));
    let mut handles = Vec::new();
    for _ in 0..2 {
        let scenario = Arc::clone(&scenario);
        let barrier = Arc::clone(&barrier);
        let id = id.clone();
        handles.push(thread::spawn(move || {
            barrier.wait();
            scenario.admit(&id, default_request(), Revision::new(1))
        }));
    }
    let results: Vec<_> = handles
        .into_iter()
        .map(|handle| handle.join().expect("admission thread completes"))
        .collect();
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    let stale: Vec<_> = results
        .iter()
        .filter(|result| {
            matches!(
                result,
                Err(AdmissionFailure::Denied(AdmissionDenial::StaleRevision {
                    expected,
                    actual,
                })) if *expected == Revision::new(1) && *actual == Revision::new(2)
            )
        })
        .collect();
    assert_eq!(stale.len(), 1);
    let view = scenario.execution_view(&id).expect("view replays");
    assert_eq!(view.invocation_count(), 1);
    assert_eq!(scenario.treasury_held().value(), 4);
    assert_eq!(journal.read(&id).expect("history reads").len(), 2);

    // Indeterminate, committed: the stored exact match resolves as
    // committed before any start.
    let journal = IndeterminateJournal::new(IndeterminateMode::Committed);
    let scenario = scenario_over(journal.clone(), ManualClock::new());
    let id = session("indeterminate-committed");
    scenario
        .open_session(id.clone(), task())
        .expect("session opens");
    scenario.scan().expect("scan succeeds");
    let assignment = scenario
        .admit(&id, default_request(), Revision::new(1))
        .expect("the stored exact match resolves as committed");
    let view = scenario.execution_view(&id).expect("view replays");
    assert_eq!(view.invocation_count(), 1);
    scenario.script_outcome(ScriptedOutcome::never_reports());
    assert_eq!(
        scenario
            .invoke(&id, assignment.invocation(), Revision::new(2))
            .expect("start proceeds after the resolution"),
        StartOutcome::Started
    );

    // Indeterminate, absent: the retry commits while the history still ends
    // at the original expected revision.
    let journal = IndeterminateJournal::new(IndeterminateMode::Absent);
    let scenario = scenario_over(journal.clone(), ManualClock::new());
    let id = session("indeterminate-absent");
    scenario
        .open_session(id.clone(), task())
        .expect("session opens");
    scenario.scan().expect("scan succeeds");
    scenario
        .admit(&id, default_request(), Revision::new(1))
        .expect("the retry commits");
    assert_eq!(
        scenario
            .execution_view(&id)
            .expect("view")
            .invocation_count(),
        1
    );
    assert_eq!(
        journal
            .read(&id)
            .expect("history reads")
            .iter()
            .filter(|entry| matches!(entry.event(), SessionEvent::AssignmentAdmitted { .. }))
            .count(),
        1
    );

    // Indeterminate with any other advancement: stale.
    let journal = IndeterminateJournal::new(IndeterminateMode::CompetingAdvances);
    let scenario = scenario_over(journal.clone(), ManualClock::new());
    let id = session("indeterminate-competing");
    scenario
        .open_session(id.clone(), task())
        .expect("session opens");
    scenario.scan().expect("scan succeeds");
    assert!(matches!(
        scenario
            .admit(&id, default_request(), Revision::new(1))
            .unwrap_err(),
        AdmissionFailure::Denied(AdmissionDenial::StaleRevision { .. })
    ));
    assert_eq!(
        scenario
            .execution_view(&id)
            .expect("view")
            .invocation_count(),
        0
    );
}

/// Helper namespace for backend inspections in tests.
struct BackendAccess;

impl BackendAccess {
    fn last_started(backend: &ScriptedBackend) -> Option<ymp_runtime::BackendInvocation> {
        backend.last_started().cloned()
    }
}

/// "Start failure branches: a confirmed never-started failure is accounted
/// as `failed`, while a start error with unknown outcome becomes
/// `uncertain` with reservation and workspace hold intact and the
/// conflicting successor refused."
#[test]
fn start_failure_branches_confirmed_and_unknown() {
    // Confirmed never-started: accounted as failed.
    let (_, scenario, id) = prepared(ManualClock::new());
    scenario.script_outcome(ScriptedOutcome::fails_at_start("adapter_unavailable"));
    let assignment = scenario
        .admit(&id, default_request(), Revision::new(1))
        .expect("admission commits");
    match scenario
        .invoke(&id, assignment.invocation(), Revision::new(2))
        .expect("start resolves")
    {
        StartOutcome::FailedAtStart { class } => assert_eq!(class.as_str(), "adapter_unavailable"),
        other => panic!("expected a confirmed never-started failure, got {other:?}"),
    }
    let view = scenario.execution_view(&id).expect("view replays");
    let invocation = view
        .invocation(assignment.invocation())
        .expect("invocation");
    // Terminal `failed` without a termination observation.
    assert_eq!(invocation.status(), InvocationStatus::Failed);
    assert_eq!(
        invocation
            .failure_class()
            .expect("failure class recorded")
            .as_str(),
        "adapter_unavailable"
    );
    assert_eq!(invocation.termination(), None);
    assert!(invocation.settled_usage().is_some());
    assert!(!invocation.is_live());
    assert_eq!(scenario.treasury_held().value(), 0);
    assert!(
        scenario
            .workspace_hold_of(assignment.invocation())
            .is_none()
    );
    assert_eq!(scenario.accounting(&id).expect("accounting").failed(), 1);

    // Unknown start outcome: uncertain, holds intact, successor refused.
    let (_, scenario, id) = prepared(ManualClock::new());
    scenario.script_outcome(ScriptedOutcome::start_outcome_unknown(
        "start_outcome_unproven",
    ));
    let assignment = scenario
        .admit(&id, default_request(), Revision::new(1))
        .expect("admission commits");
    match scenario
        .invoke(&id, assignment.invocation(), Revision::new(2))
        .expect("start resolves")
    {
        StartOutcome::UncertainAtStart { class } => {
            assert_eq!(class.as_str(), "start_outcome_unproven");
        }
        other => panic!("expected an unknown start outcome, got {other:?}"),
    }
    let view = scenario.execution_view(&id).expect("view replays");
    let invocation = view
        .invocation(assignment.invocation())
        .expect("invocation");
    assert_eq!(invocation.status(), InvocationStatus::Uncertain);
    assert_eq!(
        invocation.uncertainty_cause(),
        Some(UncertaintyCause::StartOutcomeUnknown)
    );
    assert!(invocation.holds_reservation());
    assert_eq!(scenario.treasury_held().value(), 4);
    assert_eq!(
        scenario.workspace_hold_of(assignment.invocation()),
        Some(scope())
    );
    assert_eq!(scenario.accounting(&id).expect("accounting").uncertain(), 1);
    let successor = request_for(other_agent(), "reviewer", scope(), 1, default_settings());
    assert!(matches!(
        scenario
            .admit(&id, successor, Revision::new(3))
            .unwrap_err(),
        AdmissionFailure::Denied(AdmissionDenial::WorkspaceNotEnforceable {
            refusal: WorkspaceAccessRefusal::HeldByPredecessor { held_by },
            ..
        }) if held_by == *assignment.invocation()
    ));
}

/// "Host-enforced limits: no turns are issued past the turn bound, output
/// past the size bound is refused, and expired wall-clock, a lost
/// observation stream or a lost receipt without a termination observation
/// records `uncertain` after the bounded deadline."
#[test]
fn host_enforced_limits_bound_output_turns_and_deadline() {
    // Output past the size bound is refused; a lost receipt records
    // uncertain after the deadline, never timed out.
    let clock = ManualClock::new();
    let (journal, scenario) = scenario_with(clock.clone());
    let id = session("host-limits-output");
    scenario
        .open_session(id.clone(), task())
        .expect("session opens");
    scenario.scan().expect("scan succeeds");
    scenario.script_outcome(ScriptedOutcome::never_reports_with(vec![
        ExecutionObservation::OutputObserved { chars: 60 },
        ExecutionObservation::OutputObserved { chars: 60 },
    ]));
    let request = AssignmentRequest::new(
        agent(),
        Role::new("implementer").expect("valid role"),
        default_settings(),
        tight_allowance(),
        scope(),
    );
    let assignment = scenario
        .admit(&id, request, Revision::new(1))
        .expect("admission commits");
    scenario
        .invoke(&id, assignment.invocation(), Revision::new(2))
        .expect("invocation starts");
    match scenario
        .observe(&id, assignment.invocation(), Revision::new(3))
        .expect("observation resolves")
    {
        ObservationOutcome::WaitingForTermination {
            partial_usage,
            enforcement,
        } => {
            assert!(enforcement.output_refused);
            assert!(!enforcement.turns_capped);
            // 120 observed against a 100 bound: 100 accepted, the rest
            // refused at the host boundary.
            assert_eq!(partial_usage.output_chars(), Some(100));
        }
        other => panic!("expected a waiting observation, got {other:?}"),
    }
    clock.advance(Duration::from_millis(60_000));
    match scenario
        .observe(&id, assignment.invocation(), Revision::new(3))
        .expect("deadline observation resolves")
    {
        ObservationOutcome::UncertainAfterDeadline { .. } => {}
        other => panic!("expected uncertainty after the deadline, got {other:?}"),
    }
    let view = scenario.execution_view(&id).expect("view replays");
    let invocation = view
        .invocation(assignment.invocation())
        .expect("invocation");
    assert_eq!(invocation.status(), InvocationStatus::Uncertain);
    assert_eq!(
        invocation.uncertainty_cause(),
        Some(UncertaintyCause::BoundedWaitExpired)
    );
    assert_eq!(invocation.termination(), None);
    // Reservation and hold stay intact.
    assert_eq!(scenario.treasury_held().value(), 4);
    assert_eq!(
        scenario.workspace_hold_of(assignment.invocation()),
        Some(scope())
    );
    // No termination was recorded: only the waiting and uncertain facts.
    assert_eq!(journal.read(&id).expect("history reads").len(), 4);

    // No turns are issued past the turn bound: later observations are not
    // processed once the reported turns reach the bound.
    let (_, scenario, id) = prepared(ManualClock::new());
    scenario.script_outcome(ScriptedOutcome::never_reports_with(vec![
        ExecutionObservation::UsageObserved {
            usage: ObservedUsage::unknown().with_turns(3),
        },
        ExecutionObservation::OutputObserved { chars: 50 },
        ExecutionObservation::OutputObserved { chars: 50 },
    ]));
    let request = AssignmentRequest::new(
        agent(),
        Role::new("implementer").expect("valid role"),
        default_settings(),
        tight_allowance(),
        scope(),
    );
    let assignment = scenario
        .admit(&id, request, Revision::new(1))
        .expect("admission commits");
    scenario
        .invoke(&id, assignment.invocation(), Revision::new(2))
        .expect("invocation starts");
    match scenario
        .observe(&id, assignment.invocation(), Revision::new(3))
        .expect("observation resolves")
    {
        ObservationOutcome::WaitingForTermination {
            partial_usage,
            enforcement,
        } => {
            assert!(enforcement.turns_capped);
            assert!(!enforcement.output_refused);
            assert_eq!(partial_usage.turns(), Some(3));
            // The observations after the turn bound were never issued, so
            // the output stays unknown rather than becoming zero.
            assert_eq!(partial_usage.output_chars(), None);
        }
        other => panic!("expected a waiting observation, got {other:?}"),
    }
}

/// "Bounded cancellation: once the bounded wait deadline passes without a
/// termination observation the state is recorded as `uncertain` with
/// reservation and workspace hold intact, and the conflicting successor is
/// refused until termination or effect evidence."
#[test]
fn bounded_cancellation_records_uncertainty_and_waits() {
    let clock = ManualClock::new();
    let (_, scenario) = scenario_with(clock.clone());
    let id = session("bounded-cancellation");
    scenario
        .open_session(id.clone(), task())
        .expect("session opens");
    scenario.scan().expect("scan succeeds");
    scenario.script_outcome(ScriptedOutcome::cancelled_without_confirmation());
    let assignment = scenario
        .admit(&id, default_request(), Revision::new(1))
        .expect("admission commits");
    scenario
        .invoke(&id, assignment.invocation(), Revision::new(2))
        .expect("starts");

    let cancellation = scenario
        .cancel(&id, assignment.invocation(), Revision::new(3))
        .expect("cancellation request records");
    assert!(cancellation.backend_acknowledged());
    assert_eq!(
        cancellation.invocation_phase(),
        InvocationStatus::Cancelling
    );
    assert_eq!(
        scenario
            .execution_view(&id)
            .expect("view")
            .invocation(assignment.invocation())
            .expect("invocation")
            .status(),
        InvocationStatus::Cancelling
    );

    // Within the deadline the invocation keeps waiting.
    assert!(matches!(
        scenario
            .observe(&id, assignment.invocation(), Revision::new(4))
            .expect("observation resolves"),
        ObservationOutcome::WaitingForTermination { .. }
    ));
    assert_eq!(
        scenario
            .execution_view(&id)
            .expect("view")
            .invocation(assignment.invocation())
            .expect("invocation")
            .status(),
        InvocationStatus::Cancelling
    );

    // The deadline passes without a termination observation: uncertain,
    // with reservation and hold intact.
    clock.advance(Duration::from_millis(60_000));
    assert!(matches!(
        scenario
            .observe(&id, assignment.invocation(), Revision::new(4))
            .expect("deadline observation resolves"),
        ObservationOutcome::UncertainAfterDeadline { .. }
    ));
    let view = scenario.execution_view(&id).expect("view replays");
    let invocation = view
        .invocation(assignment.invocation())
        .expect("invocation");
    assert_eq!(invocation.status(), InvocationStatus::Uncertain);
    assert!(invocation.holds_reservation());
    assert_eq!(scenario.treasury_held().value(), 4);
    assert_eq!(
        scenario.workspace_hold_of(assignment.invocation()),
        Some(scope())
    );
    assert_eq!(scenario.accounting(&id).expect("accounting").uncertain(), 1);

    // The conflicting successor is refused while the predecessor holds.
    let successor = request_for(other_agent(), "reviewer", scope(), 1, default_settings());
    assert!(matches!(
        scenario
            .admit(&id, successor.clone(), Revision::new(5))
            .unwrap_err(),
        AdmissionFailure::Denied(AdmissionDenial::WorkspaceNotEnforceable {
            refusal: WorkspaceAccessRefusal::HeldByPredecessor { held_by },
            ..
        }) if held_by == *assignment.invocation()
    ));

    // Effect evidence: the predecessor's own stream later reports that its
    // writes to the held scope have ended. Recording the evidence releases
    // the workspace hold and lifts the successor bar in one append, while
    // the invocation itself remains uncertain with its reservation held.
    assert_eq!(
        scenario
            .evidence(&id, assignment.invocation(), Revision::new(5))
            .expect("evidence step resolves"),
        ymp_runtime::EvidenceOutcome::NotObserved
    );
    scenario.with_backend_mut(|backend| {
        backend.deliver_observation(assignment.invocation(), ExecutionObservation::WritesEnded);
    });
    assert_eq!(
        scenario
            .evidence(&id, assignment.invocation(), Revision::new(5))
            .expect("evidence records"),
        ymp_runtime::EvidenceOutcome::Recorded
    );
    let view = scenario.execution_view(&id).expect("view replays");
    let invocation = view
        .invocation(assignment.invocation())
        .expect("invocation");
    assert_eq!(invocation.status(), InvocationStatus::Uncertain);
    assert!(invocation.effect_evidence_recorded());
    // The reservation is still held; the workspace hold is not.
    assert_eq!(scenario.treasury_held().value(), 4);
    assert!(
        scenario
            .workspace_hold_of(assignment.invocation())
            .is_none()
    );
    let successor_assignment = scenario
        .admit(&id, successor, Revision::new(6))
        .expect("the successor is admitted after the effect evidence");
    assert_eq!(successor_assignment.invocation().as_str(), "invocation-2");

    // Contrast: with a termination observation the successor is admitted
    // after the settlement.
    let (_, scenario, id) = prepared(ManualClock::new());
    scenario.script_outcome(ScriptedOutcome::cancelled_with_confirmation(
        ObservedUsage::unknown(),
    ));
    let predecessor = scenario
        .admit(&id, default_request(), Revision::new(1))
        .expect("admission commits");
    scenario
        .invoke(&id, predecessor.invocation(), Revision::new(2))
        .expect("starts");
    scenario
        .cancel(&id, predecessor.invocation(), Revision::new(3))
        .expect("cancels");
    match scenario
        .observe(&id, predecessor.invocation(), Revision::new(4))
        .expect("terminates")
    {
        ObservationOutcome::Terminated {
            termination: Termination::Cancelled,
            ..
        } => {}
        other => panic!("expected a cancelled termination, got {other:?}"),
    }
    scenario
        .settle(&id, predecessor.invocation(), Revision::new(5))
        .expect("settles");
    let successor = scenario
        .admit(&id, default_request(), Revision::new(6))
        .expect("successor is admitted after termination evidence");
    assert_eq!(successor.invocation().as_str(), "invocation-2");
    assert_eq!(scenario.treasury_held().value(), 4);
}

/// "Accounting totals include failed invocations: completed, failed,
/// cancelled and coordination work all appear in the session totals,
/// unreported usage is accounted as unknown, not zero, and a failed
/// settlement append returns a typed error and is retried with the
/// reservation still held."
#[test]
fn accounting_totals_settlement_retry_and_unknown_usage() {
    let (_, scenario, id) = prepared(ManualClock::new());

    // Completed with fully reported usage.
    scenario.script_outcome(ScriptedOutcome::completes(
        None,
        ObservedUsage::unknown()
            .with_turns(4)
            .with_output_chars(1200)
            .with_wall_clock(Duration::from_millis(10_000)),
    ));
    let completed = scenario
        .admit(&id, default_request(), Revision::new(1))
        .expect("first admission commits");
    scenario
        .invoke(&id, completed.invocation(), Revision::new(2))
        .expect("starts");
    scenario
        .observe(&id, completed.invocation(), Revision::new(3))
        .expect("terminates");
    scenario
        .settle(&id, completed.invocation(), Revision::new(4))
        .expect("settles");

    // Failed with entirely unreported usage: still counted, still settled.
    scenario.script_outcome(ScriptedOutcome::fails(
        "provider_overloaded",
        ObservedUsage::unknown(),
    ));
    let failed = scenario
        .admit(&id, default_request(), Revision::new(5))
        .expect("second admission commits");
    scenario
        .invoke(&id, failed.invocation(), Revision::new(6))
        .expect("starts");
    scenario
        .observe(&id, failed.invocation(), Revision::new(7))
        .expect("terminates");
    scenario
        .settle(&id, failed.invocation(), Revision::new(8))
        .expect("settles");

    // Cancelled with confirmation and partly reported usage.
    scenario.script_outcome(ScriptedOutcome::cancelled_with_confirmation(
        ObservedUsage::unknown().with_turns(1),
    ));
    let cancelled = scenario
        .admit(&id, default_request(), Revision::new(9))
        .expect("third admission commits");
    scenario
        .invoke(&id, cancelled.invocation(), Revision::new(10))
        .expect("starts");
    scenario
        .cancel(&id, cancelled.invocation(), Revision::new(11))
        .expect("cancels");
    scenario
        .observe(&id, cancelled.invocation(), Revision::new(12))
        .expect("terminates");
    scenario
        .settle(&id, cancelled.invocation(), Revision::new(13))
        .expect("settles");

    let accounting = scenario.accounting(&id).expect("accounting replays");
    assert_eq!(accounting.completed(), 1);
    assert_eq!(accounting.failed(), 1);
    assert_eq!(accounting.cancelled(), 1);
    assert_eq!(accounting.timed_out(), 0);
    // Three admissions around the invocations are coordination work.
    assert_eq!(accounting.coordination(), 3);
    let usage = accounting.usage();
    assert_eq!(usage.turns_known(), 5);
    // The failed invocation reported no turns: one unknown report, not
    // zero.
    assert_eq!(usage.turns_unknown_reports(), 1);
    assert_eq!(usage.output_chars_known(), 1200);
    // The failed and cancelled invocations reported no output: unknown,
    // never zero.
    assert_eq!(usage.output_chars_unknown_reports(), 2);
    assert_eq!(usage.wall_clock_known(), Duration::from_millis(10_000));
    assert_eq!(usage.wall_clock_unknown_reports(), 2);
    assert_eq!(scenario.treasury_held().value(), 0);

    // A failed settlement append returns a typed error, keeps the
    // reservation held, and is retried.
    let journal = FlakySettlementJournal::new();
    let scenario = scenario_over(journal, ManualClock::new());
    let id = session("settlement-retry");
    scenario
        .open_session(id.clone(), task())
        .expect("session opens");
    scenario.scan().expect("scan succeeds");
    scenario.script_outcome(ScriptedOutcome::completes(
        None,
        ObservedUsage::unknown().with_turns(2),
    ));
    let assignment = scenario
        .admit(&id, default_request(), Revision::new(1))
        .expect("admission commits");
    scenario
        .invoke(&id, assignment.invocation(), Revision::new(2))
        .expect("starts");
    match scenario
        .observe(&id, assignment.invocation(), Revision::new(3))
        .expect("terminates")
    {
        ObservationOutcome::Terminated {
            termination: Termination::Completed,
            ..
        } => {}
        other => panic!("expected a completed termination, got {other:?}"),
    }
    // The settlement append fails once with a typed error.
    assert!(matches!(
        scenario
            .settle(&id, assignment.invocation(), Revision::new(4))
            .unwrap_err(),
        ExecutionError::Journal(JournalError::AdapterFailure { .. })
    ));
    // The reservation is still held and the invocation is not accounted.
    assert_eq!(scenario.treasury_held().value(), 4);
    let view = scenario.execution_view(&id).expect("view replays");
    let invocation = view
        .invocation(assignment.invocation())
        .expect("invocation");
    assert_eq!(invocation.status(), InvocationStatus::Terminated);
    assert!(invocation.settled_usage().is_none());
    assert_eq!(
        scenario
            .accounting(&id)
            .expect("accounting")
            .usage()
            .turns_known(),
        0
    );
    // The retry commits the settlement.
    assert_eq!(
        scenario
            .settle(&id, assignment.invocation(), Revision::new(4))
            .expect("settlement retry commits")
            .value(),
        4
    );
    assert_eq!(scenario.treasury_held().value(), 0);
    assert_eq!(
        scenario
            .accounting(&id)
            .expect("accounting")
            .usage()
            .turns_known(),
        2
    );
}

/// "No real provider execution: every check runs against scripted
/// adapters, discovery is simulated by the scripted scan, and no check
/// accesses the real native environment, credentials, network or user
/// data."
#[test]
fn no_real_provider_execution_implied_by_any_test() {
    // Structural facts behind this check, documented in the module header:
    // the scripted adapters spawn no process, open no socket and touch no
    // credential store; their only inputs are the in-test configuration.
    // The assertions below pin the observable side of that guarantee.

    // Discovery is explicit and simulated: reading the pool never scans,
    // and the scan reports exactly the configured identity and offering.
    let (journal, scenario) = scenario_with(ManualClock::new());
    let id = session("no-real-provider");
    scenario
        .open_session(id.clone(), task())
        .expect("session opens");
    assert_eq!(
        scenario.pool().empty_reason(),
        Some(EmptyPoolReason::NotScanned)
    );
    let pool = scenario.scan().expect("scripted scan succeeds");
    assert_eq!(pool.entries().len(), 2);
    let claude = pool
        .entries()
        .iter()
        .find(|entry| entry.agent().as_str() == AGENT)
        .expect("the configured agent is discovered");
    assert_eq!(claude.offering().id().as_str(), "scripted-offering");
    assert!(claude.exclusion().is_none());

    // The full admitted scenario runs against the in-memory journal: the
    // only artifacts are the session events this harness drove.
    scenario.script_outcome(ScriptedOutcome::completes(
        Some(settings(&[("effort", "high")])),
        ObservedUsage::unknown().with_turns(1),
    ));
    let assignment = scenario
        .admit(&id, default_request(), Revision::new(1))
        .expect("admission commits");
    scenario
        .invoke(&id, assignment.invocation(), Revision::new(2))
        .expect("starts");
    scenario
        .observe(&id, assignment.invocation(), Revision::new(3))
        .expect("terminates");
    scenario
        .settle(&id, assignment.invocation(), Revision::new(4))
        .expect("settles");
    let history = journal.read(&id).expect("history reads");
    assert_eq!(history.len(), 5);
    assert!(matches!(
        history[0].event(),
        SessionEvent::SessionOpened { .. }
    ));
    assert!(matches!(
        history[1].event(),
        SessionEvent::AssignmentAdmitted { .. }
    ));
    assert!(matches!(
        history[2].event(),
        SessionEvent::InvocationStarted { .. }
    ));
    assert!(matches!(
        history[3].event(),
        SessionEvent::InvocationObserved { .. }
    ));
    assert!(matches!(
        history[4].event(),
        SessionEvent::InvocationAccounted { .. }
    ));
}
