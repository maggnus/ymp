#![forbid(unsafe_code)]

use std::error::Error;
use std::fmt;

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
    SessionOpened { session_id: SessionId, task: Task },
    SessionCancelled { session_id: SessionId },
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
