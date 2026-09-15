#![forbid(unsafe_code)]

pub mod execution;

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use execution::InvocationId;
use ymp_domain::{SessionId, Task};

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
    /// requested and resolved sent settings, and workspace scope.
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
                ..
            } => {
                ensure_session_id(stream_id, session_id)?;
                ensure_opened(&task)?;
                if invocations.contains_key(invocation) {
                    return Err(HistoryError::DuplicateAdmission {
                        revision: entry.revision,
                        invocation: invocation.clone(),
                    });
                }
                invocations.insert(invocation.clone(), InvocationTrack::default());
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
                if track.started
                    || track.cancel_requested
                    || track.uncertain
                    || track.failed_at_start
                    || track.evidence
                    || track.observed
                    || track.accounted
                {
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
                // A start is journaled as an attempt before the external
                // action; observed, accounted, uncertain or a recorded
                // never-started failure cannot follow or precede it.
                if !track.start_attempted
                    || track.uncertain
                    || track.failed_at_start
                    || track.observed
                    || track.accounted
                {
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
                ..
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
                if track.observed || track.accounted || track.failed_at_start {
                    return Err(HistoryError::LifecycleOutOfOrder {
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
                if !track.started || track.observed || track.accounted {
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
                // Only an invocation that never started can fail at start.
                if track.started || track.observed || track.uncertain || track.accounted {
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
                // Evidence concerns an invocation that has not terminated.
                if track.observed || track.failed_at_start || track.accounted {
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
                if track.accounted || track.uncertain || track.failed_at_start {
                    return Err(HistoryError::LifecycleOutOfOrder {
                        revision: entry.revision,
                        invocation: invocation.clone(),
                    });
                }
                track.observed = true;
            }
            SessionEvent::InvocationAccounted {
                session_id,
                invocation,
                ..
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
                if !track.observed && !track.failed_at_start {
                    return Err(HistoryError::LifecycleOutOfOrder {
                        revision: entry.revision,
                        invocation: invocation.clone(),
                    });
                }
                track.accounted = true;
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
    }))
}

/// The lifecycle facts one admitted invocation has recorded. `observed`
/// without `started` is invalid: a termination observation concerns a started
/// invocation, and the only never-started terminal path is
/// `failed_at_start`. `uncertain` without `started` is a start error with an
/// unknown outcome; every start is first journaled as an attempt.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct InvocationTrack {
    start_attempted: bool,
    started: bool,
    cancel_requested: bool,
    uncertain: bool,
    failed_at_start: bool,
    evidence: bool,
    observed: bool,
    accounted: bool,
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
