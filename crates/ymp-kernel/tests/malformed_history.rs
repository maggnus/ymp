#![forbid(unsafe_code)]

use std::time::Duration;

use ymp_domain::{
    AcceptanceContract, Constraints, Criterion, CriterionId, Goal, SessionId, Task, TaskId,
};
use ymp_kernel::execution::{
    AgentId, Allowance, Assignment, ErrorClass, Grant, GrantId, InvocationId, InvocationLimits,
    ObservedUsage, ResourceAmount, Role, Settings, Termination, UncertaintyCause, WorkspaceScope,
};
use ymp_kernel::{
    DispatchError, Dispatcher, HistoryError, Journal, JournalEntry, JournalError, Revision,
    SessionEvent,
};

#[derive(Clone)]
struct InjectedJournal {
    entries: Vec<JournalEntry>,
}

impl Journal for InjectedJournal {
    fn read(&self, _session_id: &SessionId) -> Result<Vec<JournalEntry>, JournalError> {
        Ok(self.entries.clone())
    }

    fn append(
        &self,
        _session_id: &SessionId,
        _expected_revision: Revision,
        _events: Vec<SessionEvent>,
    ) -> Result<Revision, JournalError> {
        Err(JournalError::AdapterFailure {
            message: "injected journal is read-only".to_owned(),
        })
    }
}

fn session_id(value: &str) -> SessionId {
    SessionId::new(value).expect("valid session ID")
}

fn task() -> Task {
    Task::new(
        TaskId::new("task").expect("valid task ID"),
        Goal::new("Produce a checked result").expect("valid goal"),
        AcceptanceContract::new(vec![
            Criterion::new(
                CriterionId::new("checked").expect("valid criterion ID"),
                "The result is checked",
            )
            .expect("valid criterion"),
        ])
        .expect("valid acceptance contract"),
        Constraints::new(Vec::new()).expect("valid constraints"),
    )
}

fn opened(id: &SessionId, revision: u64) -> JournalEntry {
    JournalEntry::new(
        Revision::new(revision),
        SessionEvent::SessionOpened {
            session_id: id.clone(),
            task: task(),
        },
    )
}

fn cancelled(id: &SessionId, revision: u64) -> JournalEntry {
    JournalEntry::new(
        Revision::new(revision),
        SessionEvent::SessionCancelled {
            session_id: id.clone(),
        },
    )
}

#[test]
fn dispatcher_rejects_revision_gaps_from_an_alternate_journal() {
    let id = session_id("gap");
    let dispatcher = Dispatcher::new(InjectedJournal {
        entries: vec![opened(&id, 2)],
    });

    assert_eq!(
        dispatcher.read(&id),
        Err(DispatchError::MalformedHistory(HistoryError::RevisionGap {
            expected: Revision::new(1),
            actual: Revision::new(2),
        }))
    );
}

#[test]
fn dispatcher_rejects_invalid_event_order_from_an_alternate_journal() {
    let id = session_id("event-order");
    let histories = [
        (vec![cancelled(&id, 1)], HistoryError::FirstEventMustOpen),
        (
            vec![opened(&id, 1), opened(&id, 2)],
            HistoryError::DuplicateOpening {
                revision: Revision::new(2),
            },
        ),
        (
            vec![opened(&id, 1), cancelled(&id, 2), cancelled(&id, 3)],
            HistoryError::EventAfterCancellation {
                revision: Revision::new(3),
            },
        ),
    ];

    for (entries, expected) in histories {
        let dispatcher = Dispatcher::new(InjectedJournal { entries });
        assert_eq!(
            dispatcher.read(&id),
            Err(DispatchError::MalformedHistory(expected))
        );
    }
}

#[test]
fn dispatcher_rejects_an_event_for_another_session() {
    let stream_id = session_id("stream");
    let event_id = session_id("other");
    let dispatcher = Dispatcher::new(InjectedJournal {
        entries: vec![opened(&event_id, 1)],
    });

    assert_eq!(
        dispatcher.read(&stream_id),
        Err(DispatchError::MalformedHistory(
            HistoryError::SessionIdMismatch {
                stream: stream_id,
                event: event_id,
            }
        ))
    )
}

// ---------------------------------------------------------------------------
// Execution lifecycle: deliberately malformed histories from an alternative
// journal must be rejected by the exact transition table and its cross-field
// validations.
// ---------------------------------------------------------------------------

fn invocation(name: &str) -> InvocationId {
    InvocationId::new(name).expect("valid invocation ID")
}

fn assignment_for(invocation_id: &InvocationId) -> Assignment {
    let reservation = ResourceAmount::new(5);
    Assignment::new(
        invocation_id.clone(),
        AgentId::new("claude-opus-5").expect("valid agent ID"),
        Role::new("implementer").expect("valid role"),
        Settings::new(),
        Settings::new(),
        Allowance::new(
            reservation,
            InvocationLimits::new(3, 100, Duration::from_millis(1000)).expect("valid limits"),
        )
        .expect("valid allowance"),
        Grant::new(
            GrantId::new("grant-1").expect("valid grant ID"),
            invocation_id.clone(),
            reservation,
        ),
        WorkspaceScope::new("session-primary").expect("valid scope"),
    )
}

fn admitted(id: &SessionId, revision: u64, assignment: &Assignment) -> JournalEntry {
    admitted_as(id, revision, assignment.invocation(), assignment)
}

fn admitted_as(
    id: &SessionId,
    revision: u64,
    invocation_id: &InvocationId,
    assignment: &Assignment,
) -> JournalEntry {
    JournalEntry::new(
        Revision::new(revision),
        SessionEvent::AssignmentAdmitted {
            session_id: id.clone(),
            invocation: invocation_id.clone(),
            assignment: assignment.clone(),
        },
    )
}

fn start_attempted(id: &SessionId, invocation_id: &InvocationId, revision: u64) -> JournalEntry {
    JournalEntry::new(
        Revision::new(revision),
        SessionEvent::InvocationStartAttempted {
            session_id: id.clone(),
            invocation: invocation_id.clone(),
        },
    )
}

fn started(id: &SessionId, invocation_id: &InvocationId, revision: u64) -> JournalEntry {
    JournalEntry::new(
        Revision::new(revision),
        SessionEvent::InvocationStarted {
            session_id: id.clone(),
            invocation: invocation_id.clone(),
        },
    )
}

fn uncertain(
    id: &SessionId,
    invocation_id: &InvocationId,
    revision: u64,
    cause: UncertaintyCause,
) -> JournalEntry {
    JournalEntry::new(
        Revision::new(revision),
        SessionEvent::InvocationUncertain {
            session_id: id.clone(),
            invocation: invocation_id.clone(),
            cause,
        },
    )
}

fn cancellation_requested(
    id: &SessionId,
    invocation_id: &InvocationId,
    revision: u64,
) -> JournalEntry {
    JournalEntry::new(
        Revision::new(revision),
        SessionEvent::InvocationCancellationRequested {
            session_id: id.clone(),
            invocation: invocation_id.clone(),
        },
    )
}

fn failed_at_start(id: &SessionId, invocation_id: &InvocationId, revision: u64) -> JournalEntry {
    JournalEntry::new(
        Revision::new(revision),
        SessionEvent::InvocationFailedAtStart {
            session_id: id.clone(),
            invocation: invocation_id.clone(),
            class: ErrorClass::new("adapter_unavailable").expect("valid error class"),
        },
    )
}

fn effect_evidence(id: &SessionId, invocation_id: &InvocationId, revision: u64) -> JournalEntry {
    JournalEntry::new(
        Revision::new(revision),
        SessionEvent::EffectEvidenceRecorded {
            session_id: id.clone(),
            invocation: invocation_id.clone(),
        },
    )
}

fn observed(
    id: &SessionId,
    invocation_id: &InvocationId,
    revision: u64,
    usage: ObservedUsage,
) -> JournalEntry {
    JournalEntry::new(
        Revision::new(revision),
        SessionEvent::InvocationObserved {
            session_id: id.clone(),
            invocation: invocation_id.clone(),
            termination: Termination::Completed,
            reported_settings: None,
            usage,
        },
    )
}

fn accounted(
    id: &SessionId,
    invocation_id: &InvocationId,
    revision: u64,
    usage: ObservedUsage,
    reservation: ResourceAmount,
) -> JournalEntry {
    JournalEntry::new(
        Revision::new(revision),
        SessionEvent::InvocationAccounted {
            session_id: id.clone(),
            invocation: invocation_id.clone(),
            usage,
            reservation,
        },
    )
}

fn reject(id: &SessionId, entries: Vec<JournalEntry>, expected: HistoryError) {
    assert_eq!(
        Dispatcher::new(InjectedJournal { entries }).read(id),
        Err(DispatchError::MalformedHistory(expected))
    );
}

#[test]
fn replay_rejects_invalid_execution_transitions_from_an_alternate_journal() {
    let id = session_id("execution-transitions");
    let first = invocation("invocation-1");
    let assignment = assignment_for(&first);
    let usage = ObservedUsage::unknown().with_turns(2);
    let reservation = ResourceAmount::new(5);

    // A termination observation before the invocation started.
    reject(
        &id,
        vec![
            opened(&id, 1),
            admitted(&id, 2, &assignment),
            start_attempted(&id, &first, 3),
            observed(&id, &first, 4, usage),
        ],
        HistoryError::LifecycleOutOfOrder {
            revision: Revision::new(4),
            invocation: first.clone(),
        },
    );

    // Effect evidence before the invocation started.
    reject(
        &id,
        vec![
            opened(&id, 1),
            admitted(&id, 2, &assignment),
            start_attempted(&id, &first, 3),
            effect_evidence(&id, &first, 4),
        ],
        HistoryError::LifecycleOutOfOrder {
            revision: Revision::new(4),
            invocation: first.clone(),
        },
    );

    // A start after a recorded never-started failure.
    reject(
        &id,
        vec![
            opened(&id, 1),
            admitted(&id, 2, &assignment),
            start_attempted(&id, &first, 3),
            failed_at_start(&id, &first, 4),
            started(&id, &first, 5),
        ],
        HistoryError::LifecycleOutOfOrder {
            revision: Revision::new(5),
            invocation: first.clone(),
        },
    );

    // A never-started failure without a journaled start attempt.
    reject(
        &id,
        vec![
            opened(&id, 1),
            admitted(&id, 2, &assignment),
            failed_at_start(&id, &first, 3),
        ],
        HistoryError::LifecycleOutOfOrder {
            revision: Revision::new(3),
            invocation: first.clone(),
        },
    );

    // A cancellation request after the invocation became uncertain.
    reject(
        &id,
        vec![
            opened(&id, 1),
            admitted(&id, 2, &assignment),
            start_attempted(&id, &first, 3),
            started(&id, &first, 4),
            uncertain(&id, &first, 5, UncertaintyCause::BoundedWaitExpired),
            cancellation_requested(&id, &first, 6),
        ],
        HistoryError::LifecycleOutOfOrder {
            revision: Revision::new(6),
            invocation: first.clone(),
        },
    );

    // A settlement after the invocation became uncertain.
    reject(
        &id,
        vec![
            opened(&id, 1),
            admitted(&id, 2, &assignment),
            start_attempted(&id, &first, 3),
            started(&id, &first, 4),
            uncertain(&id, &first, 5, UncertaintyCause::BoundedWaitExpired),
            accounted(&id, &first, 6, usage, reservation),
        ],
        HistoryError::LifecycleOutOfOrder {
            revision: Revision::new(6),
            invocation: first.clone(),
        },
    );

    // A duplicate start attempt.
    reject(
        &id,
        vec![
            opened(&id, 1),
            admitted(&id, 2, &assignment),
            start_attempted(&id, &first, 3),
            start_attempted(&id, &first, 4),
        ],
        HistoryError::DuplicateLifecycleEvent {
            revision: Revision::new(4),
            invocation: first.clone(),
            event: ymp_kernel::LifecycleEventKind::StartAttempted,
        },
    );

    // A start without a journaled attempt.
    reject(
        &id,
        vec![
            opened(&id, 1),
            admitted(&id, 2, &assignment),
            started(&id, &first, 3),
        ],
        HistoryError::LifecycleOutOfOrder {
            revision: Revision::new(3),
            invocation: first.clone(),
        },
    );
}

#[test]
fn replay_rejects_uncertainty_causes_that_do_not_match_the_phase() {
    let id = session_id("uncertainty-cause");
    let first = invocation("invocation-1");
    let assignment = assignment_for(&first);

    // An unknown start outcome after the invocation started.
    reject(
        &id,
        vec![
            opened(&id, 1),
            admitted(&id, 2, &assignment),
            start_attempted(&id, &first, 3),
            started(&id, &first, 4),
            uncertain(&id, &first, 5, UncertaintyCause::StartOutcomeUnknown),
        ],
        HistoryError::UncertaintyCauseMismatch {
            revision: Revision::new(5),
            invocation: first.clone(),
        },
    );

    // A bounded-wait expiry before the invocation started.
    reject(
        &id,
        vec![
            opened(&id, 1),
            admitted(&id, 2, &assignment),
            start_attempted(&id, &first, 3),
            uncertain(&id, &first, 4, UncertaintyCause::BoundedWaitExpired),
        ],
        HistoryError::UncertaintyCauseMismatch {
            revision: Revision::new(4),
            invocation: first.clone(),
        },
    );

    // An unknown start outcome without any journaled attempt.
    reject(
        &id,
        vec![
            opened(&id, 1),
            admitted(&id, 2, &assignment),
            uncertain(&id, &first, 3, UncertaintyCause::StartOutcomeUnknown),
        ],
        HistoryError::UncertaintyCauseMismatch {
            revision: Revision::new(3),
            invocation: first.clone(),
        },
    );
}

#[test]
fn replay_rejects_admissions_whose_identities_or_reservations_contradict() {
    let id = session_id("admission-consistency");
    let first = invocation("invocation-1");
    let other = invocation("invocation-2");

    // The event names one invocation while the assignment names another.
    let mismatched = assignment_for(&first);
    reject(
        &id,
        vec![opened(&id, 1), admitted_as(&id, 2, &other, &mismatched)],
        HistoryError::AssignmentInvocationMismatch {
            revision: Revision::new(2),
            invocation: other.clone(),
            assignment: first.clone(),
        },
    );

    // The grant is issued for another invocation.
    let reservation = ResourceAmount::new(5);
    let grant_for_other = Assignment::new(
        first.clone(),
        AgentId::new("claude-opus-5").expect("valid agent ID"),
        Role::new("implementer").expect("valid role"),
        Settings::new(),
        Settings::new(),
        Allowance::new(
            reservation,
            InvocationLimits::new(3, 100, Duration::from_millis(1000)).expect("valid limits"),
        )
        .expect("valid allowance"),
        Grant::new(
            GrantId::new("grant-9").expect("valid grant ID"),
            other.clone(),
            reservation,
        ),
        WorkspaceScope::new("session-primary").expect("valid scope"),
    );
    reject(
        &id,
        vec![opened(&id, 1), admitted(&id, 2, &grant_for_other)],
        HistoryError::GrantInvocationMismatch {
            revision: Revision::new(2),
            invocation: first.clone(),
            grant: other,
        },
    );

    // The grant's reservation contradicts the allowance's reservation.
    let mismatched_reservation = Assignment::new(
        first.clone(),
        AgentId::new("claude-opus-5").expect("valid agent ID"),
        Role::new("implementer").expect("valid role"),
        Settings::new(),
        Settings::new(),
        Allowance::new(
            ResourceAmount::new(5),
            InvocationLimits::new(3, 100, Duration::from_millis(1000)).expect("valid limits"),
        )
        .expect("valid allowance"),
        Grant::new(
            GrantId::new("grant-1").expect("valid grant ID"),
            first.clone(),
            ResourceAmount::new(4),
        ),
        WorkspaceScope::new("session-primary").expect("valid scope"),
    );
    reject(
        &id,
        vec![opened(&id, 1), admitted(&id, 2, &mismatched_reservation)],
        HistoryError::ReservationMismatch {
            revision: Revision::new(2),
            invocation: first.clone(),
            expected: ResourceAmount::new(5),
            actual: ResourceAmount::new(4),
        },
    );
}

#[test]
fn replay_rejects_settlements_that_contradict_the_admitted_facts() {
    let id = session_id("settlement-consistency");
    let first = invocation("invocation-1");
    let assignment = assignment_for(&first);
    let observed_usage = ObservedUsage::unknown().with_turns(2);

    // The settled reservation contradicts the admitted reservation.
    reject(
        &id,
        vec![
            opened(&id, 1),
            admitted(&id, 2, &assignment),
            start_attempted(&id, &first, 3),
            started(&id, &first, 4),
            observed(&id, &first, 5, observed_usage),
            accounted(&id, &first, 6, observed_usage, ResourceAmount::new(4)),
        ],
        HistoryError::ReservationMismatch {
            revision: Revision::new(6),
            invocation: first.clone(),
            expected: ResourceAmount::new(5),
            actual: ResourceAmount::new(4),
        },
    );

    // The settled usage contradicts the observed usage.
    reject(
        &id,
        vec![
            opened(&id, 1),
            admitted(&id, 2, &assignment),
            start_attempted(&id, &first, 3),
            started(&id, &first, 4),
            observed(&id, &first, 5, observed_usage),
            accounted(
                &id,
                &first,
                6,
                ObservedUsage::unknown().with_turns(9),
                ResourceAmount::new(5),
            ),
        ],
        HistoryError::SettledUsageMismatch {
            revision: Revision::new(6),
            invocation: first.clone(),
        },
    );

    // A never-started failure must settle unknown usage, never an invented
    // report.
    reject(
        &id,
        vec![
            opened(&id, 1),
            admitted(&id, 2, &assignment),
            start_attempted(&id, &first, 3),
            failed_at_start(&id, &first, 4),
            accounted(&id, &first, 5, observed_usage, ResourceAmount::new(5)),
        ],
        HistoryError::SettledUsageMismatch {
            revision: Revision::new(5),
            invocation: first.clone(),
        },
    );
}
