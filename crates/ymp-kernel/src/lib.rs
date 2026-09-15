#![forbid(unsafe_code)]

mod acceptance;
pub mod execution;

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use execution::{
    InvocationId, ObservedUsage, ReservationPurpose, ResourceAmount, UncertaintyCause,
};
use ymp_domain::{CriterionId, Evidence, SessionId, Task};

pub use acceptance::AcceptanceAuthority;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Revision(u64);

impl Revision {
    pub const INITIAL: Self = Self(0);

    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn value(self) -> u64 {
        self.0
    }

    pub const fn checked_next(self) -> Option<Self> {
        match self.0.checked_add(1) {
            Some(value) => Some(Self(value)),
            None => None,
        }
    }
}

impl fmt::Display for Revision {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SessionEvent {
    SessionOpened {
        session_id: SessionId,
        task: Task,
    },
    SessionCancelled {
        session_id: SessionId,
    },
    /// One committed admission: the assignment with its grant, allowance,
    /// reservation purpose, requested and resolved sent settings, and the
    /// required operations in each workspace scope.
    /// Assignment, grant, allowance, reservation and sent settings appear
    /// together in this one event or not at all.
    AssignmentAdmitted {
        session_id: SessionId,
        invocation: InvocationId,
        assignment: execution::Assignment,
    },
    /// The host is about to call the backend's `start` for one admitted
    /// invocation. This fact is appended before the external start action so
    /// history never stays at `admitted` while a start may already be in
    /// flight; a retry that finds the attempt unresolved re-asks the backend
    /// idempotently instead of starting twice.
    InvocationStartAttempted {
        session_id: SessionId,
        invocation: InvocationId,
    },
    /// The host started one admitted invocation. The sent settings were
    /// resolved and recorded at admission; `start` passes exactly those.
    InvocationStarted {
        session_id: SessionId,
        invocation: InvocationId,
    },
    /// The kernel recorded `uncertain` for one invocation: the bounded wait
    /// deadline passed without a termination observation, or a start error
    /// had an unknown outcome. The reservation and workspace hold stay
    /// intact.
    InvocationUncertain {
        session_id: SessionId,
        invocation: InvocationId,
        cause: execution::UncertaintyCause,
    },
    /// Cancellation was requested for one started invocation. Cancellation
    /// does not prove termination.
    InvocationCancellationRequested {
        session_id: SessionId,
        invocation: InvocationId,
    },
    /// A typed start failure that confirms the invocation never started:
    /// terminal `failed` without a termination observation. The reservation
    /// and workspace hold are released and accounting is permitted.
    InvocationFailedAtStart {
        session_id: SessionId,
        invocation: InvocationId,
        class: execution::ErrorClass,
    },
    /// Effect evidence for one invocation: an observation on its own event
    /// stream or receipt reported that its writes to the held scope have
    /// ended. Recording the evidence releases the workspace hold and lifts
    /// the successor bar in this one append; the invocation itself remains
    /// `uncertain` with its reservation held.
    EffectEvidenceRecorded {
        session_id: SessionId,
        invocation: InvocationId,
    },
    /// One termination observation for one invocation: the typed
    /// termination, the settings the backend reported (unknown stays
    /// unknown) and the usage observed.
    InvocationObserved {
        session_id: SessionId,
        invocation: InvocationId,
        termination: execution::Termination,
        reported_settings: Option<execution::Settings>,
        usage: execution::ObservedUsage,
    },
    /// One settlement: the reservation released and the usage the
    /// settlement recorded. Settled at the termination observation.
    InvocationAccounted {
        session_id: SessionId,
        invocation: InvocationId,
        usage: execution::ObservedUsage,
        reservation: execution::ResourceAmount,
    },
    /// Preliminary evidence for declared criteria, attributed to the
    /// completed invocation whose admitted workspace the check observed. It
    /// does not create a result version or a final Acceptance decision.
    CriterionEvaluated {
        session_id: SessionId,
        invocation: InvocationId,
        evidence: Evidence,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JournalEntry {
    revision: Revision,
    event: SessionEvent,
}

impl JournalEntry {
    pub fn new(revision: Revision, event: SessionEvent) -> Self {
        Self { revision, event }
    }

    pub fn revision(&self) -> Revision {
        self.revision
    }

    pub fn event(&self) -> &SessionEvent {
        &self.event
    }
}

pub trait Journal: Send + Sync {
    fn read(&self, session_id: &SessionId) -> Result<Vec<JournalEntry>, JournalError>;

    fn append(
        &self,
        session_id: &SessionId,
        expected_revision: Revision,
        events: Vec<SessionEvent>,
    ) -> Result<Revision, JournalError>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum JournalError {
    EmptyBatch,
    StaleRevision {
        expected: Revision,
        actual: Revision,
    },
    RevisionOverflow,
    AdapterFailure {
        message: String,
    },
    Corruption {
        message: String,
    },
    UnsupportedFormat {
        version: u16,
    },
    IndeterminateCommit {
        expected: Revision,
        attempted: Revision,
        message: String,
    },
}

impl fmt::Display for JournalError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyBatch => formatter.write_str("journal append batch must not be empty"),
            Self::StaleRevision { expected, actual } => write!(
                formatter,
                "journal revision is stale: expected {expected}, actual {actual}"
            ),
            Self::RevisionOverflow => formatter.write_str("journal revision would overflow"),
            Self::AdapterFailure { message } => {
                write!(formatter, "journal adapter failed: {message}")
            }
            Self::Corruption { message } => {
                write!(formatter, "journal storage is corrupted: {message}")
            }
            Self::UnsupportedFormat { version } => write!(
                formatter,
                "journal stream format version {version} is not supported"
            ),
            Self::IndeterminateCommit {
                expected,
                attempted,
                message,
            } => write!(
                formatter,
                "journal commit outcome is indeterminate: expected {expected}, attempted \
                 {attempted}: {message}"
            ),
        }
    }
}

impl Error for JournalError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SessionStatus {
    Open,
    Cancelled,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionView {
    session_id: SessionId,
    task: Task,
    status: SessionStatus,
    revision: Revision,
    evidence: Vec<(InvocationId, Evidence)>,
}

impl SessionView {
    pub fn session_id(&self) -> &SessionId {
        &self.session_id
    }

    pub fn task(&self) -> &Task {
        &self.task
    }

    pub fn status(&self) -> SessionStatus {
        self.status
    }

    pub fn revision(&self) -> Revision {
        self.revision
    }

    pub fn evidence(&self) -> impl Iterator<Item = (&InvocationId, &Evidence)> {
        self.evidence
            .iter()
            .map(|(invocation, evidence)| (invocation, evidence))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HistoryError {
    RevisionGap {
        expected: Revision,
        actual: Revision,
    },
    FirstEventMustOpen,
    SessionIdMismatch {
        stream: SessionId,
        event: SessionId,
    },
    DuplicateOpening {
        revision: Revision,
    },
    EventAfterCancellation {
        revision: Revision,
    },
    UnknownInvocation {
        revision: Revision,
        invocation: InvocationId,
    },
    DuplicateAdmission {
        revision: Revision,
        invocation: InvocationId,
    },
    DuplicateLifecycleEvent {
        revision: Revision,
        invocation: InvocationId,
        event: LifecycleEventKind,
    },
    LifecycleOutOfOrder {
        revision: Revision,
        invocation: InvocationId,
    },
    /// The admission names one invocation while its assignment record names
    /// another.
    AssignmentInvocationMismatch {
        revision: Revision,
        invocation: InvocationId,
        assignment: InvocationId,
    },
    /// The assignment's grant is issued for another invocation.
    GrantInvocationMismatch {
        revision: Revision,
        invocation: InvocationId,
        grant: InvocationId,
    },
    /// A reservation contradicts the admitted reservation of the same
    /// invocation: across the allowance, the grant and the settlement.
    ReservationMismatch {
        revision: Revision,
        invocation: InvocationId,
        expected: ResourceAmount,
        actual: ResourceAmount,
    },
    /// The grant assigns its reservation to a different purpose than the
    /// bounded allowance.
    ReservationPurposeMismatch {
        revision: Revision,
        invocation: InvocationId,
        expected: ReservationPurpose,
        actual: ReservationPurpose,
    },
    /// A settlement records usage that contradicts the invocation's
    /// observed or failed-at-start usage.
    SettledUsageMismatch {
        revision: Revision,
        invocation: InvocationId,
    },
    /// An uncertainty was recorded with a cause that does not match the
    /// invocation's phase.
    UncertaintyCauseMismatch {
        revision: Revision,
        invocation: InvocationId,
    },
    CriterionEvaluationBeforeCompletion {
        revision: Revision,
        invocation: InvocationId,
    },
    UnknownEvidenceCriterion {
        revision: Revision,
        criterion: CriterionId,
    },
    DuplicateCriterionEvaluation {
        revision: Revision,
        criterion: CriterionId,
    },
    EvidenceWorkspaceMismatch {
        revision: Revision,
        invocation: InvocationId,
        workspace: String,
    },
}

/// The lifecycle events one invocation records, for error specificity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LifecycleEventKind {
    StartAttempted,
    Started,
    CancellationRequested,
    Observed,
    Uncertain,
    FailedAtStart,
    EffectEvidence,
    Accounted,
}

impl fmt::Display for LifecycleEventKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::StartAttempted => "start attempted",
            Self::Started => "started",
            Self::CancellationRequested => "cancellation requested",
            Self::Observed => "observed",
            Self::Uncertain => "uncertain",
            Self::FailedAtStart => "failed at start",
            Self::EffectEvidence => "effect evidence",
            Self::Accounted => "accounted",
        };
        formatter.write_str(name)
    }
}

impl fmt::Display for HistoryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RevisionGap { expected, actual } => write!(
                formatter,
                "session history revision is not contiguous: expected {expected}, actual {actual}"
            ),
            Self::FirstEventMustOpen => {
                formatter.write_str("session history must begin with SessionOpened")
            }
            Self::SessionIdMismatch { stream, event } => write!(
                formatter,
                "session event ID '{event}' does not match stream ID '{stream}'"
            ),
            Self::DuplicateOpening { revision } => {
                write!(formatter, "duplicate SessionOpened at revision {revision}")
            }
            Self::EventAfterCancellation { revision } => {
                write!(
                    formatter,
                    "event follows cancellation at revision {revision}"
                )
            }
            Self::UnknownInvocation {
                revision,
                invocation,
            } => write!(
                formatter,
                "event at revision {revision} references invocation '{invocation}', which was \
                 never admitted"
            ),
            Self::DuplicateAdmission {
                revision,
                invocation,
            } => write!(
                formatter,
                "duplicate admission of invocation '{invocation}' at revision {revision}"
            ),
            Self::DuplicateLifecycleEvent {
                revision,
                invocation,
                event,
            } => write!(
                formatter,
                "duplicate {event} event for invocation '{invocation}' at revision {revision}"
            ),
            Self::LifecycleOutOfOrder {
                revision,
                invocation,
            } => write!(
                formatter,
                "lifecycle event at revision {revision} does not follow invocation \
                 '{invocation}''s recorded state"
            ),
            Self::AssignmentInvocationMismatch {
                revision,
                invocation,
                assignment,
            } => write!(
                formatter,
                "admission at revision {revision} names invocation '{invocation}' but its \
                 assignment names '{assignment}'"
            ),
            Self::GrantInvocationMismatch {
                revision,
                invocation,
                grant,
            } => write!(
                formatter,
                "admission at revision {revision} of invocation '{invocation}' carries a grant \
                 issued for '{grant}'"
            ),
            Self::ReservationMismatch {
                revision,
                invocation,
                expected,
                actual,
            } => write!(
                formatter,
                "reservation at revision {revision} of invocation '{invocation}' contradicts its \
                 admitted reservation: expected {}, actual {}",
                expected.value(),
                actual.value()
            ),
            Self::ReservationPurposeMismatch {
                revision,
                invocation,
                expected,
                actual,
            } => write!(
                formatter,
                "reservation at revision {revision} of invocation '{invocation}' contradicts its \
                 admitted purpose: expected {expected}, actual {actual}"
            ),
            Self::SettledUsageMismatch {
                revision,
                invocation,
            } => write!(
                formatter,
                "settlement at revision {revision} of invocation '{invocation}' records usage \
                 that contradicts its observed usage"
            ),
            Self::UncertaintyCauseMismatch {
                revision,
                invocation,
            } => write!(
                formatter,
                "uncertainty at revision {revision} of invocation '{invocation}' names a cause \
                 that does not match its phase"
            ),
            Self::CriterionEvaluationBeforeCompletion {
                revision,
                invocation,
            } => write!(
                formatter,
                "criterion evaluation at revision {revision} requires invocation '{invocation}' \
                 to have an observed completed outcome"
            ),
            Self::UnknownEvidenceCriterion {
                revision,
                criterion,
            } => write!(
                formatter,
                "criterion evaluation at revision {revision} covers undeclared criterion \
                 '{criterion}'"
            ),
            Self::DuplicateCriterionEvaluation {
                revision,
                criterion,
            } => write!(
                formatter,
                "criterion evaluation at revision {revision} repeats criterion '{criterion}'"
            ),
            Self::EvidenceWorkspaceMismatch {
                revision,
                invocation,
                workspace,
            } => write!(
                formatter,
                "criterion evaluation at revision {revision} for invocation '{invocation}' \
                 observed workspace '{workspace}' outside its admitted scopes"
            ),
        }
    }
}

impl Error for HistoryError {}

pub fn replay_session(
    stream_id: &SessionId,
    entries: &[JournalEntry],
) -> Result<Option<SessionView>, HistoryError> {
    if entries.is_empty() {
        return Ok(None);
    }

    let mut previous_revision = Revision::INITIAL;
    let mut task = None;
    let mut status = SessionStatus::Open;
    let mut current_revision = Revision::INITIAL;
    let mut invocations = BTreeMap::new();
    let mut evaluated_criteria = std::collections::HashSet::new();
    let mut evidence = Vec::new();

    for (index, entry) in entries.iter().enumerate() {
        let expected_revision =
            previous_revision
                .checked_next()
                .ok_or(HistoryError::RevisionGap {
                    expected: previous_revision,
                    actual: entry.revision,
                })?;
        if entry.revision != expected_revision {
            return Err(HistoryError::RevisionGap {
                expected: expected_revision,
                actual: entry.revision,
            });
        }

        if status == SessionStatus::Cancelled {
            return Err(HistoryError::EventAfterCancellation {
                revision: entry.revision,
            });
        }

        match &entry.event {
            SessionEvent::SessionOpened {
                session_id,
                task: opened_task,
            } => {
                ensure_session_id(stream_id, session_id)?;
                if index != 0 || task.is_some() {
                    return Err(HistoryError::DuplicateOpening {
                        revision: entry.revision,
                    });
                }
                task = Some(opened_task.clone());
            }
            SessionEvent::SessionCancelled { session_id } => {
                ensure_session_id(stream_id, session_id)?;
                if task.is_none() {
                    return Err(HistoryError::FirstEventMustOpen);
                }
                status = SessionStatus::Cancelled;
            }
            SessionEvent::AssignmentAdmitted {
                session_id,
                invocation,
                assignment,
            } => {
                ensure_session_id(stream_id, session_id)?;
                ensure_opened(&task)?;
                if invocations.contains_key(invocation) {
                    return Err(HistoryError::DuplicateAdmission {
                        revision: entry.revision,
                        invocation: invocation.clone(),
                    });
                }
                // The admission's identities and reservations are one
                // fact: the event, the assignment and the grant name the
                // same invocation, and the grant holds exactly the
                // allowance's reservation.
                if assignment.invocation() != invocation {
                    return Err(HistoryError::AssignmentInvocationMismatch {
                        revision: entry.revision,
                        invocation: invocation.clone(),
                        assignment: assignment.invocation().clone(),
                    });
                }
                if assignment.grant().invocation() != invocation {
                    return Err(HistoryError::GrantInvocationMismatch {
                        revision: entry.revision,
                        invocation: invocation.clone(),
                        grant: assignment.grant().invocation().clone(),
                    });
                }
                let admitted_reservation = assignment.allowance().reservation();
                if assignment.grant().reservation() != admitted_reservation {
                    return Err(HistoryError::ReservationMismatch {
                        revision: entry.revision,
                        invocation: invocation.clone(),
                        expected: admitted_reservation,
                        actual: assignment.grant().reservation(),
                    });
                }
                let admitted_purpose = assignment.allowance().reservation_purpose();
                if assignment.grant().reservation_purpose() != admitted_purpose {
                    return Err(HistoryError::ReservationPurposeMismatch {
                        revision: entry.revision,
                        invocation: invocation.clone(),
                        expected: admitted_purpose,
                        actual: assignment.grant().reservation_purpose(),
                    });
                }
                invocations.insert(
                    invocation.clone(),
                    InvocationTrack {
                        reservation: Some(admitted_reservation),
                        workspaces: assignment
                            .workspace_accesses()
                            .iter()
                            .map(|access| access.scope().as_str().to_owned())
                            .collect(),
                        ..InvocationTrack::default()
                    },
                );
            }
            SessionEvent::InvocationStartAttempted {
                session_id,
                invocation,
            } => {
                ensure_session_id(stream_id, session_id)?;
                let track = invocation_track(&mut invocations, invocation, entry.revision)?;
                if track.start_attempted {
                    return Err(HistoryError::DuplicateLifecycleEvent {
                        revision: entry.revision,
                        invocation: invocation.clone(),
                        event: LifecycleEventKind::StartAttempted,
                    });
                }
                if track.phase() != InvocationReplayPhase::Admitted {
                    return Err(HistoryError::LifecycleOutOfOrder {
                        revision: entry.revision,
                        invocation: invocation.clone(),
                    });
                }
                track.start_attempted = true;
            }
            SessionEvent::InvocationStarted {
                session_id,
                invocation,
            } => {
                ensure_session_id(stream_id, session_id)?;
                let track = invocation_track(&mut invocations, invocation, entry.revision)?;
                if track.started {
                    return Err(HistoryError::DuplicateLifecycleEvent {
                        revision: entry.revision,
                        invocation: invocation.clone(),
                        event: LifecycleEventKind::Started,
                    });
                }
                if track.phase() != InvocationReplayPhase::StartAttempted {
                    return Err(HistoryError::LifecycleOutOfOrder {
                        revision: entry.revision,
                        invocation: invocation.clone(),
                    });
                }
                track.started = true;
            }
            SessionEvent::InvocationUncertain {
                session_id,
                invocation,
                cause,
            } => {
                ensure_session_id(stream_id, session_id)?;
                let track = invocation_track(&mut invocations, invocation, entry.revision)?;
                if track.uncertain {
                    return Err(HistoryError::DuplicateLifecycleEvent {
                        revision: entry.revision,
                        invocation: invocation.clone(),
                        event: LifecycleEventKind::Uncertain,
                    });
                }
                if !matches!(
                    track.phase(),
                    InvocationReplayPhase::Admitted
                        | InvocationReplayPhase::StartAttempted
                        | InvocationReplayPhase::Started
                        | InvocationReplayPhase::Cancelling
                ) {
                    return Err(HistoryError::LifecycleOutOfOrder {
                        revision: entry.revision,
                        invocation: invocation.clone(),
                    });
                }
                // The cause must match the phase: an unknown start outcome
                // exists only before a start, and the bounded wait expires
                // only after one.
                let cause_matches_phase = matches!(
                    (track.phase(), cause),
                    (
                        InvocationReplayPhase::StartAttempted,
                        UncertaintyCause::StartOutcomeUnknown
                    ) | (
                        InvocationReplayPhase::Started | InvocationReplayPhase::Cancelling,
                        UncertaintyCause::BoundedWaitExpired
                    )
                );
                if !cause_matches_phase {
                    return Err(HistoryError::UncertaintyCauseMismatch {
                        revision: entry.revision,
                        invocation: invocation.clone(),
                    });
                }
                track.uncertain = true;
            }
            SessionEvent::InvocationCancellationRequested {
                session_id,
                invocation,
            } => {
                ensure_session_id(stream_id, session_id)?;
                let track = invocation_track(&mut invocations, invocation, entry.revision)?;
                if track.cancel_requested {
                    return Err(HistoryError::DuplicateLifecycleEvent {
                        revision: entry.revision,
                        invocation: invocation.clone(),
                        event: LifecycleEventKind::CancellationRequested,
                    });
                }
                if track.phase() != InvocationReplayPhase::Started {
                    return Err(HistoryError::LifecycleOutOfOrder {
                        revision: entry.revision,
                        invocation: invocation.clone(),
                    });
                }
                track.cancel_requested = true;
            }
            SessionEvent::InvocationFailedAtStart {
                session_id,
                invocation,
                ..
            } => {
                ensure_session_id(stream_id, session_id)?;
                let track = invocation_track(&mut invocations, invocation, entry.revision)?;
                if track.failed_at_start {
                    return Err(HistoryError::DuplicateLifecycleEvent {
                        revision: entry.revision,
                        invocation: invocation.clone(),
                        event: LifecycleEventKind::FailedAtStart,
                    });
                }
                if track.phase() != InvocationReplayPhase::StartAttempted {
                    return Err(HistoryError::LifecycleOutOfOrder {
                        revision: entry.revision,
                        invocation: invocation.clone(),
                    });
                }
                track.failed_at_start = true;
            }
            SessionEvent::EffectEvidenceRecorded {
                session_id,
                invocation,
            } => {
                ensure_session_id(stream_id, session_id)?;
                let track = invocation_track(&mut invocations, invocation, entry.revision)?;
                if track.evidence {
                    return Err(HistoryError::DuplicateLifecycleEvent {
                        revision: entry.revision,
                        invocation: invocation.clone(),
                        event: LifecycleEventKind::EffectEvidence,
                    });
                }
                if track.phase() != InvocationReplayPhase::Uncertain {
                    return Err(HistoryError::LifecycleOutOfOrder {
                        revision: entry.revision,
                        invocation: invocation.clone(),
                    });
                }
                track.evidence = true;
            }
            SessionEvent::InvocationObserved {
                session_id,
                invocation,
                termination,
                usage,
                ..
            } => {
                ensure_session_id(stream_id, session_id)?;
                let track = invocation_track(&mut invocations, invocation, entry.revision)?;
                if track.observed {
                    return Err(HistoryError::DuplicateLifecycleEvent {
                        revision: entry.revision,
                        invocation: invocation.clone(),
                        event: LifecycleEventKind::Observed,
                    });
                }
                if !matches!(
                    track.phase(),
                    InvocationReplayPhase::Started | InvocationReplayPhase::Cancelling
                ) {
                    return Err(HistoryError::LifecycleOutOfOrder {
                        revision: entry.revision,
                        invocation: invocation.clone(),
                    });
                }
                track.observed = true;
                track.termination = Some(termination.clone());
                track.observed_usage = Some(*usage);
            }
            SessionEvent::InvocationAccounted {
                session_id,
                invocation,
                usage,
                reservation,
            } => {
                ensure_session_id(stream_id, session_id)?;
                let track = invocation_track(&mut invocations, invocation, entry.revision)?;
                if track.accounted {
                    return Err(HistoryError::DuplicateLifecycleEvent {
                        revision: entry.revision,
                        invocation: invocation.clone(),
                        event: LifecycleEventKind::Accounted,
                    });
                }
                if !matches!(
                    track.phase(),
                    InvocationReplayPhase::Terminated | InvocationReplayPhase::FailedAtStart
                ) {
                    return Err(HistoryError::LifecycleOutOfOrder {
                        revision: entry.revision,
                        invocation: invocation.clone(),
                    });
                }
                // The settlement releases exactly the admitted reservation.
                let admitted = track
                    .reservation
                    .expect("an admitted invocation always carries its reservation");
                if *reservation != admitted {
                    return Err(HistoryError::ReservationMismatch {
                        revision: entry.revision,
                        invocation: invocation.clone(),
                        expected: admitted,
                        actual: *reservation,
                    });
                }
                // The settlement records exactly the invocation's usage: a
                // termination observation settles its observed usage, and a
                // confirmed never-started failure settles unknown usage.
                let expected_usage = if track.observed {
                    track.observed_usage
                } else {
                    Some(ObservedUsage::unknown())
                };
                if expected_usage != Some(*usage) {
                    return Err(HistoryError::SettledUsageMismatch {
                        revision: entry.revision,
                        invocation: invocation.clone(),
                    });
                }
                track.accounted = true;
            }
            SessionEvent::CriterionEvaluated {
                session_id,
                invocation,
                evidence: observed,
            } => {
                ensure_session_id(stream_id, session_id)?;
                let track = invocation_track(&mut invocations, invocation, entry.revision)?;
                if !matches!(
                    track.phase(),
                    InvocationReplayPhase::Terminated | InvocationReplayPhase::Accounted
                ) || track.termination.as_ref() != Some(&execution::Termination::Completed)
                {
                    return Err(HistoryError::CriterionEvaluationBeforeCompletion {
                        revision: entry.revision,
                        invocation: invocation.clone(),
                    });
                }
                if !track
                    .workspaces
                    .iter()
                    .any(|workspace| workspace == observed.workspace())
                {
                    return Err(HistoryError::EvidenceWorkspaceMismatch {
                        revision: entry.revision,
                        invocation: invocation.clone(),
                        workspace: observed.workspace().to_owned(),
                    });
                }
                let opened_task = task.as_ref().ok_or(HistoryError::FirstEventMustOpen)?;
                for criterion in observed.check().criteria() {
                    if !opened_task
                        .acceptance_contract()
                        .criteria()
                        .iter()
                        .any(|declared| declared.id() == criterion)
                    {
                        return Err(HistoryError::UnknownEvidenceCriterion {
                            revision: entry.revision,
                            criterion: criterion.clone(),
                        });
                    }
                    if !evaluated_criteria.insert(criterion.clone()) {
                        return Err(HistoryError::DuplicateCriterionEvaluation {
                            revision: entry.revision,
                            criterion: criterion.clone(),
                        });
                    }
                }
                evidence.push((invocation.clone(), observed.clone()));
            }
        }

        previous_revision = entry.revision;
        current_revision = entry.revision;
    }

    let task = task.ok_or(HistoryError::FirstEventMustOpen)?;
    Ok(Some(SessionView {
        session_id: stream_id.clone(),
        task,
        status,
        revision: current_revision,
        evidence,
    }))
}

/// The lifecycle facts one admitted invocation has recorded. `observed`
/// without `started` is invalid: a termination observation concerns a started
/// invocation, and the only never-started terminal path is
/// `failed_at_start`. `uncertain` without `started` is a start error with an
/// unknown outcome; every start is first journaled as an attempt. The
/// admitted reservation and observed usage are kept for the settlement's
/// cross-field validation.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct InvocationTrack {
    start_attempted: bool,
    started: bool,
    cancel_requested: bool,
    uncertain: bool,
    failed_at_start: bool,
    evidence: bool,
    observed: bool,
    termination: Option<execution::Termination>,
    observed_usage: Option<ObservedUsage>,
    accounted: bool,
    reservation: Option<ResourceAmount>,
    workspaces: Vec<String>,
}

/// The mutually exclusive lifecycle phase derived from the recorded facts.
///
/// Effect evidence is orthogonal to this phase: from `uncertain` it releases
/// the workspace hold without changing the invocation phase. Every lifecycle
/// event above checks its permitted source phase against this table before
/// adding another fact.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum InvocationReplayPhase {
    Admitted,
    StartAttempted,
    Started,
    Cancelling,
    Uncertain,
    FailedAtStart,
    Terminated,
    Accounted,
}

impl InvocationTrack {
    fn phase(&self) -> InvocationReplayPhase {
        if self.accounted {
            InvocationReplayPhase::Accounted
        } else if self.observed {
            InvocationReplayPhase::Terminated
        } else if self.failed_at_start {
            InvocationReplayPhase::FailedAtStart
        } else if self.uncertain {
            InvocationReplayPhase::Uncertain
        } else if self.cancel_requested {
            InvocationReplayPhase::Cancelling
        } else if self.started {
            InvocationReplayPhase::Started
        } else if self.start_attempted {
            InvocationReplayPhase::StartAttempted
        } else {
            InvocationReplayPhase::Admitted
        }
    }
}

fn ensure_opened(task: &Option<Task>) -> Result<(), HistoryError> {
    if task.is_none() {
        Err(HistoryError::FirstEventMustOpen)
    } else {
        Ok(())
    }
}

fn invocation_track<'a>(
    invocations: &'a mut BTreeMap<InvocationId, InvocationTrack>,
    invocation: &InvocationId,
    revision: Revision,
) -> Result<&'a mut InvocationTrack, HistoryError> {
    invocations
        .get_mut(invocation)
        .ok_or(HistoryError::UnknownInvocation {
            revision,
            invocation: invocation.clone(),
        })
}

fn ensure_session_id(stream_id: &SessionId, event_id: &SessionId) -> Result<(), HistoryError> {
    if stream_id == event_id {
        Ok(())
    } else {
        Err(HistoryError::SessionIdMismatch {
            stream: stream_id.clone(),
            event: event_id.clone(),
        })
    }
}

#[derive(Clone, Debug)]
pub struct Dispatcher<J> {
    journal: J,
}

impl<J> Dispatcher<J>
where
    J: Journal,
{
    pub fn new(journal: J) -> Self {
        Self { journal }
    }

    pub fn open(&self, session_id: SessionId, task: Task) -> Result<SessionView, DispatchError> {
        let history = self.journal.read(&session_id)?;
        if let Some(view) = replay_session(&session_id, &history)? {
            return Err(DispatchError::SessionAlreadyExists {
                session_id,
                revision: view.revision,
            });
        }

        let event = SessionEvent::SessionOpened {
            session_id: session_id.clone(),
            task: task.clone(),
        };
        let committed_revision =
            match self
                .journal
                .append(&session_id, Revision::INITIAL, vec![event])
            {
                Ok(revision) => revision,
                Err(JournalError::StaleRevision { actual, .. }) => {
                    return Err(DispatchError::SessionAlreadyExists {
                        session_id,
                        revision: actual,
                    });
                }
                Err(error) => return Err(error.into()),
            };

        Ok(SessionView {
            session_id,
            task,
            status: SessionStatus::Open,
            revision: committed_revision,
            evidence: Vec::new(),
        })
    }

    pub fn read(&self, session_id: &SessionId) -> Result<SessionView, DispatchError> {
        let history = self.journal.read(session_id)?;
        replay_session(session_id, &history)?.ok_or_else(|| DispatchError::SessionNotFound {
            session_id: session_id.clone(),
        })
    }

    pub fn cancel(
        &self,
        session_id: &SessionId,
        expected_revision: Revision,
    ) -> Result<SessionView, DispatchError> {
        let current = self.read(session_id)?;
        if current.revision != expected_revision {
            return Err(DispatchError::StaleRevision {
                expected: expected_revision,
                actual: current.revision,
            });
        }
        if current.status == SessionStatus::Cancelled {
            return Err(DispatchError::SessionAlreadyCancelled {
                session_id: session_id.clone(),
                revision: current.revision,
            });
        }

        let event = SessionEvent::SessionCancelled {
            session_id: session_id.clone(),
        };
        let committed_revision =
            match self
                .journal
                .append(session_id, expected_revision, vec![event])
            {
                Ok(revision) => revision,
                Err(JournalError::StaleRevision { expected, actual }) => {
                    return Err(DispatchError::StaleRevision { expected, actual });
                }
                Err(error) => return Err(error.into()),
            };

        Ok(SessionView {
            session_id: current.session_id,
            task: current.task,
            status: SessionStatus::Cancelled,
            revision: committed_revision,
            evidence: current.evidence,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DispatchError {
    SessionAlreadyExists {
        session_id: SessionId,
        revision: Revision,
    },
    SessionNotFound {
        session_id: SessionId,
    },
    StaleRevision {
        expected: Revision,
        actual: Revision,
    },
    SessionAlreadyCancelled {
        session_id: SessionId,
        revision: Revision,
    },
    MalformedHistory(HistoryError),
    Journal(JournalError),
}

impl fmt::Display for DispatchError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SessionAlreadyExists {
                session_id,
                revision,
            } => write!(
                formatter,
                "session '{session_id}' already exists at revision {revision}"
            ),
            Self::SessionNotFound { session_id } => {
                write!(formatter, "session '{session_id}' was not found")
            }
            Self::StaleRevision { expected, actual } => write!(
                formatter,
                "session revision is stale: expected {expected}, actual {actual}"
            ),
            Self::SessionAlreadyCancelled {
                session_id,
                revision,
            } => write!(
                formatter,
                "session '{session_id}' is already cancelled at revision {revision}"
            ),
            Self::MalformedHistory(error) => {
                write!(formatter, "malformed session history: {error}")
            }
            Self::Journal(error) => error.fmt(formatter),
        }
    }
}

impl Error for DispatchError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::MalformedHistory(error) => Some(error),
            Self::Journal(error) => Some(error),
            _ => None,
        }
    }
}

impl From<JournalError> for DispatchError {
    fn from(error: JournalError) -> Self {
        Self::Journal(error)
    }
}

impl From<HistoryError> for DispatchError {
    fn from(error: HistoryError) -> Self {
        Self::MalformedHistory(error)
    }
}
