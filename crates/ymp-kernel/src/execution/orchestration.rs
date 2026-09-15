//! Dispatcher-driven orchestration of the admitted scenario.
//!
//! Plain functions over the execution ports and the [`Journal`]: discovery,
//! admission, start, observation, cancellation and settlement. The
//! Dispatcher stays the session entry point and the journal stays the only
//! durable record: every execution fact is appended as session events under
//! the existing revision rules, and the in-memory port state is adjusted
//! only after the journal commit succeeds. Callers serialize access to the
//! shared ports; the journal's revision check makes concurrent commits
//! exactly-one-winner.
//!
//! Denials are returned to the caller without a journal append: no revision
//! advances and no grant or reservation exists. Sent settings are resolved
//! during admission from the requested settings and the offering's reported
//! controls and are recorded in the admission batch; `start` passes exactly
//! those recorded settings. Termination requires a termination observation:
//! an expired wall-clock, a lost stream or a lost receipt records
//! `uncertain` after the bounded deadline and never `timed out`. Settlement
//! is its own journal append whose failure returns a typed error, keeps the
//! reservation held and is retried.

use std::error::Error;
use std::fmt;
use std::time::Duration;

use super::ports::{
    AdmissionSnapshot, BackendInvocation, ExecutionBackend, ExecutionObservation, Gatekeeper,
    LiveAssignment, Registry, RegistryFailure, Treasury, WorkspaceGuard,
};
use super::types::{
    AdmissionDenial, AgentIneligibility, Assignment, AssignmentRequest, ErrorClass,
    ExecutionTypeError, Grant, GrantId, InvocationId, InvocationStatus, ModelOffering,
    ObservedUsage, Pool, PoolEligibility, ResourceAmount, Settings, Termination, UncertaintyCause,
};
use super::view::{SessionExecutionView, replay_execution};
use crate::{
    HistoryError, Journal, JournalEntry, JournalError, Revision, SessionEvent, SessionId,
    replay_session,
};

/// Failures of one admission attempt. A typed denial appends nothing: no
/// revision advances, and no grant or reservation exists.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AdmissionFailure {
    Denied(AdmissionDenial),
    SessionNotFound {
        session_id: SessionId,
    },
    MalformedHistory(HistoryError),
    Journal(JournalError),
    InvalidValue(ExecutionTypeError),
    /// The journal committed but an in-memory port refused the follow-up;
    /// the journal is authoritative and the ledger must be rebuilt. Not
    /// reachable with serialized callers and honest ports.
    PostCommitConflict {
        detail: String,
    },
}

impl fmt::Display for AdmissionFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Denied(denial) => write!(formatter, "admission denied: {denial}"),
            Self::SessionNotFound { session_id } => {
                write!(formatter, "session '{session_id}' was not found")
            }
            Self::MalformedHistory(error) => {
                write!(formatter, "malformed session history: {error}")
            }
            Self::Journal(error) => error.fmt(formatter),
            Self::InvalidValue(error) => write!(formatter, "invalid execution value: {error}"),
            Self::PostCommitConflict { detail } => {
                write!(formatter, "post-commit port conflict: {detail}")
            }
        }
    }
}

impl Error for AdmissionFailure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Denied(denial) => Some(denial),
            Self::MalformedHistory(error) => Some(error),
            Self::Journal(error) => Some(error),
            Self::InvalidValue(error) => Some(error),
            Self::SessionNotFound { .. } | Self::PostCommitConflict { .. } => None,
        }
    }
}

/// Failures of one execution step (start, cancellation request, observation,
/// settlement, read).
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExecutionError {
    SessionNotFound {
        session_id: SessionId,
    },
    StaleRevision {
        expected: Revision,
        actual: Revision,
    },
    UnknownInvocation {
        session_id: SessionId,
        invocation: InvocationId,
    },
    WrongPhase {
        invocation: InvocationId,
        actual: InvocationStatus,
    },
    /// The invocation was not started through the host, so no start time
    /// exists for wall-clock adjudication.
    InvocationNotStarted {
        invocation: InvocationId,
    },
    InvalidValue(ExecutionTypeError),
    MalformedHistory(HistoryError),
    Journal(JournalError),
    PostCommitConflict {
        detail: String,
    },
}

impl fmt::Display for ExecutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SessionNotFound { session_id } => {
                write!(formatter, "session '{session_id}' was not found")
            }
            Self::StaleRevision { expected, actual } => write!(
                formatter,
                "session revision is stale: expected {expected}, actual {actual}"
            ),
            Self::UnknownInvocation {
                session_id,
                invocation,
            } => write!(
                formatter,
                "invocation '{invocation}' is not admitted in session '{session_id}'"
            ),
            Self::WrongPhase { invocation, actual } => write!(
                formatter,
                "invocation '{invocation}' is in state {actual}, which does not allow this step"
            ),
            Self::InvocationNotStarted { invocation } => write!(
                formatter,
                "invocation '{invocation}' has no host-recorded start time"
            ),
            Self::InvalidValue(error) => write!(formatter, "invalid execution value: {error}"),
            Self::MalformedHistory(error) => {
                write!(formatter, "malformed session history: {error}")
            }
            Self::Journal(error) => error.fmt(formatter),
            Self::PostCommitConflict { detail } => {
                write!(formatter, "post-commit port conflict: {detail}")
            }
        }
    }
}

impl Error for ExecutionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::MalformedHistory(error) => Some(error),
            Self::Journal(error) => Some(error),
            Self::InvalidValue(error) => Some(error),
            _ => None,
        }
    }
}

/// Runs one explicit discovery scan. Reading a pool never triggers
/// discovery; only this call does.
pub fn scan_registry<R: Registry>(registry: &mut R) -> Result<Pool, RegistryFailure> {
    registry.scan()
}

/// Resolves the sent settings during admission: the requested settings the
/// offering reports a control for, passed through unchanged. The host never
/// invents a setting the native environment did not report, and `start`
/// passes exactly the recorded result.
pub fn resolve_sent_settings(requested: &Settings, offering: &ModelOffering) -> Settings {
    let mut sent = Settings::new();
    for (key, value) in requested.iter() {
        if offering.has_control(key) {
            sent.insert(key.clone(), value.clone());
        }
    }
    sent
}

/// Everything one admission is reviewed and committed against.
pub struct AdmissionContext<'a, J, G, T, W>
where
    J: Journal,
    G: Gatekeeper,
    T: Treasury,
    W: WorkspaceGuard,
{
    pub journal: &'a J,
    pub session_id: &'a SessionId,
    pub expected_revision: Revision,
    pub pool: &'a Pool,
    pub gatekeeper: &'a G,
    pub treasury: &'a mut T,
    pub workspace: &'a mut W,
}

/// Admits one assignment at the expected revision.
///
/// The requirements are reviewed against current state; any typed denial is
/// returned to the caller without a journal append, leaving journal,
/// revision, grants and reservations unchanged. A passing review commits
/// assignment, grant, allowance, reservation and the resolved sent settings
/// as one journal batch: a competing commit makes the loser stale with no
/// partial effects, and an indeterminate commit is resolved by the durable
/// journal's rule before anything else happens. The committed admission
/// returns an [`Assignment`] carrying one role, a bounded allowance and a
/// [`Grant`].
pub fn admit_assignment<J, G, T, W>(
    context: AdmissionContext<'_, J, G, T, W>,
    request: AssignmentRequest,
) -> Result<Assignment, AdmissionFailure>
where
    J: Journal,
    G: Gatekeeper,
    T: Treasury,
    W: WorkspaceGuard,
{
    let AdmissionContext {
        journal,
        session_id,
        expected_revision,
        pool,
        gatekeeper,
        treasury,
        workspace,
    } = context;

    let history = journal
        .read(session_id)
        .map_err(AdmissionFailure::Journal)?;
    let session =
        replay_session(session_id, &history).map_err(AdmissionFailure::MalformedHistory)?;
    let current = session.ok_or_else(|| AdmissionFailure::SessionNotFound {
        session_id: session_id.clone(),
    })?;
    let execution_view =
        replay_execution(session_id, &history).map_err(AdmissionFailure::MalformedHistory)?;
    let live_assignments: Vec<LiveAssignment> = execution_view
        .invocations()
        .filter(|(_, view)| view.is_live())
        .map(|(invocation, view)| {
            LiveAssignment::new(
                invocation.clone(),
                view.assignment().agent().clone(),
                view.assignment().role().clone(),
            )
        })
        .collect();

    let denial = {
        let snapshot = AdmissionSnapshot::new(
            pool,
            &live_assignments,
            &*treasury,
            &*workspace,
            current.revision(),
        );
        gatekeeper
            .review(&request, expected_revision, &snapshot)
            .err()
    };
    if let Some(denial) = denial {
        return Err(AdmissionFailure::Denied(denial));
    }

    let offering = match pool.eligibility(request.agent()) {
        PoolEligibility::Eligible { offering } => offering,
        PoolEligibility::NotDiscovered => {
            return Err(AdmissionFailure::Denied(AdmissionDenial::IneligibleAgent {
                agent: request.agent().clone(),
                ineligibility: AgentIneligibility::NotDiscovered,
            }));
        }
        PoolEligibility::Excluded(reason) => {
            return Err(AdmissionFailure::Denied(AdmissionDenial::IneligibleAgent {
                agent: request.agent().clone(),
                ineligibility: AgentIneligibility::Excluded(reason),
            }));
        }
    };
    let sent_settings = resolve_sent_settings(request.requested_settings(), &offering);

    let sequence = 1 + history
        .iter()
        .filter(|entry| matches!(entry.event(), SessionEvent::AssignmentAdmitted { .. }))
        .count();
    let invocation = InvocationId::new(format!("invocation-{sequence}"))
        .map_err(AdmissionFailure::InvalidValue)?;
    let grant_id =
        GrantId::new(format!("grant-{sequence}")).map_err(AdmissionFailure::InvalidValue)?;
    let grant = Grant::new(
        grant_id,
        invocation.clone(),
        request.allowance().reservation(),
        request.allowance().reservation_purpose(),
    );
    let assignment = Assignment::new(
        invocation,
        request.agent().clone(),
        request.role().clone(),
        request.requested_settings().clone(),
        sent_settings,
        request.allowance().clone(),
        grant.clone(),
        request.workspace_accesses().to_vec(),
    )
    .map_err(AdmissionFailure::InvalidValue)?;

    let event = SessionEvent::AssignmentAdmitted {
        session_id: session_id.clone(),
        invocation: assignment.invocation().clone(),
        assignment: assignment.clone(),
    };
    commit_resolving_indeterminate(journal, session_id, expected_revision, event)?;

    treasury
        .reserve(&grant)
        .map_err(|refused| AdmissionFailure::PostCommitConflict {
            detail: format!("committed admission could not hold its reservation: {refused}"),
        })?;
    workspace
        .hold(request.workspace_accesses(), assignment.invocation())
        .map_err(|conflict| AdmissionFailure::PostCommitConflict {
            detail: format!("committed admission could not take its workspace hold: {conflict}"),
        })?;
    Ok(assignment)
}

/// Commits one admission event, resolving an indeterminate commit by the
/// durable journal's rule: a stored exact match of the batch is committed,
/// a retry happens only while the history still ends at the original
/// expected revision, and any other advancement is stale.
fn commit_resolving_indeterminate<J: Journal>(
    journal: &J,
    session_id: &SessionId,
    expected_revision: Revision,
    event: SessionEvent,
) -> Result<(), AdmissionFailure> {
    match journal.append(session_id, expected_revision, vec![event.clone()]) {
        Ok(_) => Ok(()),
        Err(JournalError::StaleRevision { expected, actual }) => {
            Err(AdmissionFailure::Denied(AdmissionDenial::StaleRevision {
                expected,
                actual,
            }))
        }
        Err(JournalError::IndeterminateCommit { attempted, .. }) => {
            let history = journal
                .read(session_id)
                .map_err(AdmissionFailure::Journal)?;
            let head = history
                .last()
                .map_or(Revision::INITIAL, JournalEntry::revision);
            if head == attempted && history.last().is_some_and(|entry| entry.event() == &event) {
                // The stored exact match of the batch is committed.
                return Ok(());
            }
            if head == expected_revision {
                // Retry only while the history still ends at the original
                // expected revision.
                return match journal.append(session_id, expected_revision, vec![event]) {
                    Ok(_) => Ok(()),
                    Err(JournalError::StaleRevision { expected, actual }) => {
                        Err(AdmissionFailure::Denied(AdmissionDenial::StaleRevision {
                            expected,
                            actual,
                        }))
                    }
                    Err(error) => Err(AdmissionFailure::Journal(error)),
                };
            }
            // Any other advancement is stale.
            Err(AdmissionFailure::Denied(AdmissionDenial::StaleRevision {
                expected: expected_revision,
                actual: head,
            }))
        }
        Err(error) => Err(AdmissionFailure::Journal(error)),
    }
}

/// Everything one invocation start needs. The start input is only the
/// [`InvocationId`]: every executed parameter comes from the journaled
/// assignment, so an unrecorded caller-stated setting, agent, workspace or
/// limit can never reach the backend.
pub struct StartContext<'a, J, B, T, W>
where
    J: Journal,
    B: ExecutionBackend,
    T: Treasury,
    W: WorkspaceGuard,
{
    pub journal: &'a J,
    pub backend: &'a mut B,
    pub session_id: &'a SessionId,
    pub expected_revision: Revision,
    pub invocation: &'a InvocationId,
    pub treasury: &'a mut T,
    pub workspace: &'a mut W,
}

/// The outcome of one invocation start.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StartOutcome {
    Started,
    /// A typed start failure that confirms the invocation never started:
    /// the invocation skips directly to accounting as `failed` with its
    /// error class.
    FailedAtStart {
        class: ErrorClass,
    },
    /// A start error whose outcome is unknown: not a confirmed failure. The
    /// invocation stands `uncertain` with its reservation and workspace
    /// hold intact.
    UncertainAtStart {
        class: ErrorClass,
    },
}

/// Starts one admitted invocation with the settings recorded in the
/// admission batch.
///
/// The [`BackendInvocation`] handed to the backend is built exclusively from
/// the invocation's journaled assignment: agent, sent settings, workspace and
/// limits all come from the committed admission batch, never from caller
/// state beside the invocation ID.
///
/// The start attempt is journaled before the external start action, so a
/// competing write or journal error can never leave history at `admitted`
/// while a process may already be running. If the attempt's outcome append
/// fails, the invocation stands at a journaled unresolved attempt in its
/// admitted phase; calling `start_invocation` again re-asks the backend's
/// idempotent start — which returns the same outcome for the same invocation
/// — and appends the outcome at the then-current revision.
pub fn start_invocation<J, B, T, W>(
    context: StartContext<'_, J, B, T, W>,
) -> Result<StartOutcome, ExecutionError>
where
    J: Journal,
    B: ExecutionBackend,
    T: Treasury,
    W: WorkspaceGuard,
{
    let StartContext {
        journal,
        backend,
        session_id,
        expected_revision,
        invocation,
        treasury,
        workspace,
    } = context;

    let (_, view) = read_validated(journal, session_id)?;
    ensure_revision(view.revision(), expected_revision)?;
    let invocation_view =
        view.invocation(invocation)
            .ok_or_else(|| ExecutionError::UnknownInvocation {
                session_id: session_id.clone(),
                invocation: invocation.clone(),
            })?;
    if invocation_view.status() != InvocationStatus::Admitted {
        return Err(ExecutionError::WrongPhase {
            invocation: invocation.clone(),
            actual: invocation_view.status(),
        });
    }

    let assignment = invocation_view.assignment().clone();
    let mut base_revision = expected_revision;
    if !invocation_view.start_attempted() {
        // The attempt is durable before the external action happens.
        base_revision = append_events(
            journal,
            session_id,
            expected_revision,
            vec![SessionEvent::InvocationStartAttempted {
                session_id: session_id.clone(),
                invocation: invocation.clone(),
            }],
        )?;
    }

    let backend_invocation = BackendInvocation::new(
        assignment.invocation().clone(),
        assignment.agent().clone(),
        assignment.sent_settings().clone(),
        assignment.workspace_accesses().to_vec(),
        *assignment.allowance().limits(),
    );
    match backend.start(&backend_invocation) {
        Err(failure) if failure.confirmed_never_started() => {
            // Terminal failed without a termination observation: the
            // reservation and workspace hold are released and accounting
            // is permitted. The failure and its settlement land as one
            // batch, so a competing commit cannot split them.
            let usage = ObservedUsage::unknown();
            append_events(
                journal,
                session_id,
                base_revision,
                vec![
                    SessionEvent::InvocationFailedAtStart {
                        session_id: session_id.clone(),
                        invocation: invocation.clone(),
                        class: failure.class().clone(),
                    },
                    SessionEvent::InvocationAccounted {
                        session_id: session_id.clone(),
                        invocation: invocation.clone(),
                        usage,
                        reservation: assignment.grant().reservation(),
                    },
                ],
            )?;
            workspace.release(invocation);
            settle_committed_invocation(treasury, session_id, invocation, &usage)?;
            Ok(StartOutcome::FailedAtStart {
                class: failure.class().clone(),
            })
        }
        Err(failure) => {
            // A start error with an unknown outcome is not a confirmed
            // failure: the invocation stands uncertain.
            let event = SessionEvent::InvocationUncertain {
                session_id: session_id.clone(),
                invocation: invocation.clone(),
                cause: UncertaintyCause::StartOutcomeUnknown,
            };
            append_events(journal, session_id, base_revision, vec![event])?;
            Ok(StartOutcome::UncertainAtStart {
                class: failure.class().clone(),
            })
        }
        Ok(()) => {
            let event = SessionEvent::InvocationStarted {
                session_id: session_id.clone(),
                invocation: invocation.clone(),
            };
            append_events(journal, session_id, base_revision, vec![event])?;
            Ok(StartOutcome::Started)
        }
    }
}

/// Everything one cancellation request needs.
pub struct CancellationContext<'a, J, B>
where
    J: Journal,
    B: ExecutionBackend,
{
    pub journal: &'a J,
    pub backend: &'a mut B,
    pub session_id: &'a SessionId,
    pub expected_revision: Revision,
    pub invocation: &'a InvocationId,
}

/// The outcome of one cancellation request. Cancellation does not prove
/// termination: without a termination observation the invocation stands
/// uncertain after the bounded deadline, holding its reservation and
/// workspace hold.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CancellationOutcome {
    backend_acknowledged: bool,
    refusal_detail: Option<String>,
}

impl CancellationOutcome {
    pub const fn backend_acknowledged(&self) -> bool {
        self.backend_acknowledged
    }

    pub fn refusal_detail(&self) -> Option<&str> {
        self.refusal_detail.as_deref()
    }

    /// The recorded phase after this request: `cancelling` until a
    /// termination observation moves the invocation on, or the bounded
    /// deadline records `uncertain`.
    pub const fn invocation_phase(&self) -> InvocationStatus {
        InvocationStatus::Cancelling
    }
}

/// Requests cancellation of one started invocation.
///
/// The invocation moves to `cancelling`; the backend's acknowledgment or
/// refusal is an observation and changes nothing about the recorded state.
pub fn request_invocation_cancellation<J, B>(
    context: CancellationContext<'_, J, B>,
) -> Result<CancellationOutcome, ExecutionError>
where
    J: Journal,
    B: ExecutionBackend,
{
    let CancellationContext {
        journal,
        backend,
        session_id,
        expected_revision,
        invocation,
    } = context;

    let (_, view) = read_validated(journal, session_id)?;
    ensure_revision(view.revision(), expected_revision)?;
    let invocation_view =
        view.invocation(invocation)
            .ok_or_else(|| ExecutionError::UnknownInvocation {
                session_id: session_id.clone(),
                invocation: invocation.clone(),
            })?;
    if invocation_view.status() != InvocationStatus::Started {
        return Err(ExecutionError::WrongPhase {
            invocation: invocation.clone(),
            actual: invocation_view.status(),
        });
    }

    let event = SessionEvent::InvocationCancellationRequested {
        session_id: session_id.clone(),
        invocation: invocation.clone(),
    };
    append_events(journal, session_id, expected_revision, vec![event])?;

    match backend.cancel(invocation) {
        Ok(()) => Ok(CancellationOutcome {
            backend_acknowledged: true,
            refusal_detail: None,
        }),
        Err(refused) => Ok(CancellationOutcome {
            backend_acknowledged: false,
            refusal_detail: Some(refused.detail().to_owned()),
        }),
    }
}

/// What the host enforced while observing one invocation's stream.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct HostEnforcement {
    /// Output past the size bound was refused at the host-controlled
    /// boundary.
    pub output_refused: bool,
    /// The reported turns reached the turn bound, so no further
    /// observations were issued.
    pub turns_capped: bool,
}

/// The host's per-invocation accumulation across observation attempts.
///
/// Turn and output bounds are enforced against the totals accumulated over
/// every attempt, not per attempt: the caller carries the accumulation
/// returned by one [`ObservationOutcome::WaitingForTermination`] into the
/// next attempt's [`ObservationContext::prior`], so no sequence of observe
/// calls can accept output past the size bound or issue observations past
/// the turn bound. The accumulation is host-side projection state, not a
/// journal fact: after a restart it begins empty, and the durable usage
/// record of an invocation is its termination observation.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ObservationAccumulation {
    /// Output characters accepted at the host boundary so far.
    accepted_output_chars: u64,
    /// Usage merged from every accepted observation so far.
    usage: ObservedUsage,
    /// Settings the backend reported so far, if any.
    reported_settings: Option<Settings>,
    /// Whether output was ever refused at the boundary; sticky.
    output_refused: bool,
    /// Whether the reported turns ever reached the turn bound; sticky.
    turns_capped: bool,
}

impl ObservationAccumulation {
    /// The usage accumulated so far; unknown components stay unknown.
    pub fn usage(&self) -> ObservedUsage {
        self.usage
    }

    /// The settings reported so far, if the backend reported any.
    pub fn reported_settings(&self) -> Option<&Settings> {
        self.reported_settings.as_ref()
    }

    /// The output characters accepted so far.
    pub fn accepted_output_chars(&self) -> u64 {
        self.accepted_output_chars
    }

    /// What the host enforced so far.
    pub fn enforcement(&self) -> HostEnforcement {
        HostEnforcement {
            output_refused: self.output_refused,
            turns_capped: self.turns_capped,
        }
    }
}

/// Everything one termination observation needs.
pub struct ObservationContext<'a, J, B, T, W>
where
    J: Journal,
    B: ExecutionBackend,
    T: Treasury,
    W: WorkspaceGuard,
{
    pub journal: &'a J,
    pub backend: &'a mut B,
    pub session_id: &'a SessionId,
    pub expected_revision: Revision,
    pub invocation: &'a InvocationId,
    /// The accumulation carried from every previous attempt of this
    /// invocation; bounds apply against its totals.
    pub prior: ObservationAccumulation,
    /// Logical elapsed time when the invocation started.
    pub started_at: Duration,
    /// Logical elapsed time now.
    pub now: Duration,
    pub treasury: &'a mut T,
    pub workspace: &'a mut W,
}

/// The outcome of one observation attempt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ObservationOutcome {
    /// One termination state was observed; the workspace hold is released
    /// on this termination evidence, and the reservation is settled by the
    /// settlement append.
    Terminated {
        termination: Termination,
        reported_settings: Option<Settings>,
        usage: ObservedUsage,
        enforcement: HostEnforcement,
    },
    /// No termination observation exists and the bounded deadline has not
    /// passed: the invocation keeps waiting, and the caller carries the
    /// returned accumulation into the next attempt so the bounds hold
    /// across attempts.
    WaitingForTermination {
        accumulation: ObservationAccumulation,
        enforcement: HostEnforcement,
    },
    /// The bounded deadline passed without a termination observation: the
    /// kernel recorded `uncertain`, and the reservation and workspace hold
    /// stay intact.
    UncertainAfterDeadline { partial_usage: ObservedUsage },
}

/// Observes one in-flight invocation.
///
/// The event stream is scanned one observation at a time and the receipt is
/// asked for one termination state; reported settings are recorded beside
/// requested and sent. Nothing the scan read is consumed until the append
/// that records its facts resolves: on a competing write or journal error
/// the scan is rewound, so the observations survive for the retry. Effect
/// evidence on the stream is left pending for its own step. A lost stream
/// or a receipt that never arrives is not a termination: after the bounded
/// deadline the invocation is recorded `uncertain`, never `timed out`.
/// Host-enforced limits apply while scanning: output past the size bound is
/// refused at the host-controlled boundary, and once reported turns reach
/// the turn bound no further observations are issued.
pub fn observe_invocation<J, B, T, W>(
    context: ObservationContext<'_, J, B, T, W>,
) -> Result<ObservationOutcome, ExecutionError>
where
    J: Journal,
    B: ExecutionBackend,
    T: Treasury,
    W: WorkspaceGuard,
{
    let ObservationContext {
        journal,
        backend,
        session_id,
        expected_revision,
        invocation,
        prior,
        started_at,
        now,
        treasury: _,
        workspace,
    } = context;

    let (_, view) = read_validated(journal, session_id)?;
    ensure_revision(view.revision(), expected_revision)?;
    let invocation_view =
        view.invocation(invocation)
            .ok_or_else(|| ExecutionError::UnknownInvocation {
                session_id: session_id.clone(),
                invocation: invocation.clone(),
            })?;
    let status = invocation_view.status();
    if status != InvocationStatus::Started && status != InvocationStatus::Cancelling {
        return Err(ExecutionError::WrongPhase {
            invocation: invocation.clone(),
            actual: status,
        });
    }
    let limits = *invocation_view.assignment().allowance().limits();

    // Bounds apply against the totals accumulated across every attempt,
    // carried in by the caller; the counters never reset per attempt.
    let mut accumulation = prior;
    let mut termination: Option<Termination> = None;

    loop {
        if accumulation.turns_capped {
            // No further turns are issued past the turn bound.
            break;
        }
        let Some(observation) = backend.next_event(invocation) else {
            break;
        };
        if matches!(observation, ExecutionObservation::WritesEnded) {
            // Effect evidence is recorded by its own step and stays
            // pending on the stream.
            backend.unread_last(invocation);
            break;
        }
        match observation {
            ExecutionObservation::OutputObserved { chars } => {
                let remaining = limits
                    .max_output_chars()
                    .saturating_sub(accumulation.accepted_output_chars);
                let accepted = chars.min(remaining);
                accumulation.accepted_output_chars += accepted;
                if accepted < chars {
                    accumulation.output_refused = true;
                }
                accumulation.usage = accumulation.usage.with_output_chars(
                    // The accepted total is the host's own usage report for
                    // output: what was refused at the boundary was never
                    // accepted, so it is not counted as observed output.
                    accumulation.accepted_output_chars,
                );
            }
            ExecutionObservation::SettingsReported { settings } => {
                accumulation.reported_settings = Some(settings);
            }
            ExecutionObservation::UsageObserved { usage } => {
                accumulation.usage = accumulation.usage.merge_later(usage);
                if accumulation
                    .usage
                    .turns()
                    .is_some_and(|turns| turns >= u64::from(limits.max_turns()))
                {
                    accumulation.turns_capped = true;
                }
            }
            ExecutionObservation::Terminated {
                termination: observed,
            } => {
                if termination.is_none() {
                    termination = Some(observed);
                }
            }
            ExecutionObservation::WritesEnded => {}
        }
    }

    let mut reported_settings = accumulation.reported_settings.clone();
    if termination.is_none()
        && let Some(receipt) = backend.receipt(invocation)
    {
        termination = Some(receipt.termination().clone());
        reported_settings = receipt.reported_settings().cloned().or(reported_settings);
        accumulation.usage = accumulation.usage.merge_later(*receipt.usage());
    }
    let partial_usage = accumulation.usage;
    let enforcement = accumulation.enforcement();

    match termination {
        Some(termination) => {
            let event = SessionEvent::InvocationObserved {
                session_id: session_id.clone(),
                invocation: invocation.clone(),
                termination: termination.clone(),
                reported_settings: reported_settings.clone(),
                usage: partial_usage,
            };
            if let Err(error) = append_events(journal, session_id, expected_revision, vec![event]) {
                // The append did not resolve: nothing the scan read is
                // consumed, so the retry sees the same observations.
                backend.reset_scan(invocation);
                return Err(error);
            }
            backend.commit_scan(invocation);
            // The termination evidence releases the workspace hold; the
            // reservation is settled by its own append.
            workspace.release(invocation);
            Ok(ObservationOutcome::Terminated {
                termination,
                reported_settings,
                usage: partial_usage,
                enforcement,
            })
        }
        None => {
            if now.saturating_sub(started_at) >= limits.max_wall_clock() {
                // The bounded deadline passed without a termination
                // observation: record uncertain, never timed out.
                let event = SessionEvent::InvocationUncertain {
                    session_id: session_id.clone(),
                    invocation: invocation.clone(),
                    cause: UncertaintyCause::BoundedWaitExpired,
                };
                if let Err(error) =
                    append_events(journal, session_id, expected_revision, vec![event])
                {
                    backend.reset_scan(invocation);
                    return Err(error);
                }
                backend.commit_scan(invocation);
                Ok(ObservationOutcome::UncertainAfterDeadline { partial_usage })
            } else {
                // No termination observation exists and the deadline has
                // not passed: the partial facts are delivered to the
                // caller, which resolves their consumption and carries the
                // accumulation into the next attempt.
                backend.commit_scan(invocation);
                Ok(ObservationOutcome::WaitingForTermination {
                    accumulation,
                    enforcement,
                })
            }
        }
    }
}

/// Everything one settlement needs.
pub struct SettlementContext<'a, J, T>
where
    J: Journal,
    T: Treasury,
{
    pub journal: &'a J,
    pub session_id: &'a SessionId,
    pub expected_revision: Revision,
    pub invocation: &'a InvocationId,
    pub treasury: &'a mut T,
}

/// Settles one terminated invocation: the reservation is released against
/// the observed usage.
///
/// Settlement is its own journal append: a failed append returns a typed
/// error, keeps the reservation held, and is retried by calling again.
pub fn settle_invocation<J, T>(
    context: SettlementContext<'_, J, T>,
) -> Result<ResourceAmount, ExecutionError>
where
    J: Journal,
    T: Treasury,
{
    let SettlementContext {
        journal,
        session_id,
        expected_revision,
        invocation,
        treasury,
    } = context;

    let (_, view) = read_validated(journal, session_id)?;
    ensure_revision(view.revision(), expected_revision)?;
    let invocation_view =
        view.invocation(invocation)
            .ok_or_else(|| ExecutionError::UnknownInvocation {
                session_id: session_id.clone(),
                invocation: invocation.clone(),
            })?;
    if invocation_view.status() != InvocationStatus::Terminated
        && invocation_view.status() != InvocationStatus::Failed
    {
        return Err(ExecutionError::WrongPhase {
            invocation: invocation.clone(),
            actual: invocation_view.status(),
        });
    }
    if invocation_view.settled_usage().is_some() {
        return Err(ExecutionError::WrongPhase {
            invocation: invocation.clone(),
            actual: InvocationStatus::Terminated,
        });
    }

    let usage = invocation_view
        .observed_usage()
        .copied()
        .unwrap_or_default();
    let reservation = invocation_view.assignment().grant().reservation();
    let event = SessionEvent::InvocationAccounted {
        session_id: session_id.clone(),
        invocation: invocation.clone(),
        usage,
        reservation,
    };
    append_events(journal, session_id, expected_revision, vec![event])?;
    settle_committed_invocation(treasury, session_id, invocation, &usage)?;
    Ok(reservation)
}

/// Everything one effect-evidence recording needs.
pub struct EvidenceContext<'a, J, B, W>
where
    J: Journal,
    B: ExecutionBackend,
    W: WorkspaceGuard,
{
    pub journal: &'a J,
    pub backend: &'a mut B,
    pub session_id: &'a SessionId,
    pub expected_revision: Revision,
    pub invocation: &'a InvocationId,
    pub workspace: &'a mut W,
}

/// The outcome of one effect-evidence attempt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EvidenceOutcome {
    /// The writes-ended observation was recorded: the workspace hold is
    /// released and the successor bar lifted, while the invocation itself
    /// remains `uncertain` with its reservation held.
    Recorded,
    /// No writes-ended observation exists on the invocation's stream.
    NotObserved,
}

/// Records effect evidence for one invocation.
///
/// Evidence is an observation on the invocation's own event stream
/// reporting that its writes to the held scope have ended. The kernel
/// checks only that it is attributed to that invocation, then records the
/// evidence, releases the workspace hold and lifts the successor bar in
/// one journal append. The report is the backend's observation, not kernel
/// proof that a process stopped writing.
///
/// The stream is read at its head: a writes-ended observation behind
/// pending observations waits until the observation step consumes those
/// first, and the evidence observation itself is consumed only after the
/// recording append resolves — a failed append leaves it pending, and no
/// other observation kind is consumed by this step.
pub fn record_effect_evidence<J, B, W>(
    context: EvidenceContext<'_, J, B, W>,
) -> Result<EvidenceOutcome, ExecutionError>
where
    J: Journal,
    B: ExecutionBackend,
    W: WorkspaceGuard,
{
    let EvidenceContext {
        journal,
        backend,
        session_id,
        expected_revision,
        invocation,
        workspace,
    } = context;

    let (_, view) = read_validated(journal, session_id)?;
    ensure_revision(view.revision(), expected_revision)?;
    let invocation_view =
        view.invocation(invocation)
            .ok_or_else(|| ExecutionError::UnknownInvocation {
                session_id: session_id.clone(),
                invocation: invocation.clone(),
            })?;
    let status = invocation_view.status();
    if status != InvocationStatus::Started
        && status != InvocationStatus::Cancelling
        && status != InvocationStatus::Uncertain
    {
        return Err(ExecutionError::WrongPhase {
            invocation: invocation.clone(),
            actual: status,
        });
    }
    if invocation_view.effect_evidence_recorded() {
        return Err(ExecutionError::WrongPhase {
            invocation: invocation.clone(),
            actual: status,
        });
    }

    let stream_reported = matches!(
        backend.next_event(invocation),
        Some(ExecutionObservation::WritesEnded)
    );
    let receipt_reported = if stream_reported {
        false
    } else {
        // No writes-ended observation is pending at the stream head;
        // everything the peek read stays pending. A receipt is an
        // independent final-report source for the same observation.
        backend.reset_scan(invocation);
        backend
            .receipt(invocation)
            .is_some_and(|receipt| receipt.writes_ended())
    };
    if !stream_reported && !receipt_reported {
        return Ok(EvidenceOutcome::NotObserved);
    }

    let event = SessionEvent::EffectEvidenceRecorded {
        session_id: session_id.clone(),
        invocation: invocation.clone(),
    };
    match append_events(journal, session_id, expected_revision, vec![event]) {
        Ok(_) => {
            if stream_reported {
                backend.commit_scan(invocation);
            }
            workspace.release(invocation);
            Ok(EvidenceOutcome::Recorded)
        }
        Err(error) => {
            if stream_reported {
                backend.reset_scan(invocation);
            }
            Err(error)
        }
    }
}

/// Applies one committed settlement to the treasury. The journal append has
/// already succeeded, so a port refusal here is a post-commit conflict.
fn settle_committed_invocation<T: Treasury>(
    treasury: &mut T,
    session_id: &SessionId,
    invocation: &InvocationId,
    usage: &ObservedUsage,
) -> Result<(), ExecutionError> {
    treasury
        .settle(invocation, usage)
        .map_err(|refused| ExecutionError::PostCommitConflict {
            detail: format!(
                "committed settlement of '{invocation}' in session '{session_id}' could not \
                 release its reservation: {refused}"
            ),
        })?;
    Ok(())
}

/// Reads and replays one session's execution view from the journal.
pub fn read_execution<J: Journal>(
    journal: &J,
    session_id: &SessionId,
) -> Result<SessionExecutionView, ExecutionError> {
    let (_, view) = read_validated(journal, session_id)?;
    Ok(view)
}

fn read_validated<J: Journal>(
    journal: &J,
    session_id: &SessionId,
) -> Result<(Revision, SessionExecutionView), ExecutionError> {
    let history = journal.read(session_id).map_err(ExecutionError::Journal)?;
    if replay_session(session_id, &history)
        .map_err(ExecutionError::MalformedHistory)?
        .is_none()
    {
        return Err(ExecutionError::SessionNotFound {
            session_id: session_id.clone(),
        });
    }
    let view = replay_execution(session_id, &history).map_err(ExecutionError::MalformedHistory)?;
    Ok((view.revision(), view))
}

fn ensure_revision(current: Revision, expected: Revision) -> Result<(), ExecutionError> {
    if current != expected {
        Err(ExecutionError::StaleRevision {
            expected,
            actual: current,
        })
    } else {
        Ok(())
    }
}

fn append_events<J: Journal>(
    journal: &J,
    session_id: &SessionId,
    expected_revision: Revision,
    events: Vec<SessionEvent>,
) -> Result<Revision, ExecutionError> {
    match journal.append(session_id, expected_revision, events) {
        Ok(revision) => Ok(revision),
        Err(JournalError::StaleRevision { expected, actual }) => {
            Err(ExecutionError::StaleRevision { expected, actual })
        }
        Err(error) => Err(ExecutionError::Journal(error)),
    }
}
