use ymp_domain::{Evidence, SessionId};

use crate::execution::InvocationId;
use crate::{
    DispatchError, Journal, JournalEntry, JournalError, Revision, SessionEvent, SessionStatus,
    SessionView, replay_session,
};

/// Trusted entry point for binding a check observation to one completed
/// invocation and committing it as preliminary criterion evidence.
pub struct AcceptanceAuthority<'a, J> {
    journal: &'a J,
}

impl<'a, J> AcceptanceAuthority<'a, J>
where
    J: Journal,
{
    pub fn new(journal: &'a J) -> Self {
        Self { journal }
    }

    pub fn record_evidence(
        &self,
        session_id: &SessionId,
        invocation: InvocationId,
        evidence: Evidence,
        expected_revision: Revision,
    ) -> Result<SessionView, DispatchError> {
        let mut entries = self.journal.read(session_id)?;
        let current = replay_session(session_id, &entries)?.ok_or_else(|| {
            DispatchError::SessionNotFound {
                session_id: session_id.clone(),
            }
        })?;
        if current.revision() != expected_revision {
            return Err(DispatchError::StaleRevision {
                expected: expected_revision,
                actual: current.revision(),
            });
        }
        if current.status() == SessionStatus::Cancelled {
            return Err(DispatchError::SessionAlreadyCancelled {
                session_id: session_id.clone(),
                revision: current.revision(),
            });
        }

        let next = expected_revision
            .checked_next()
            .ok_or(DispatchError::Journal(JournalError::RevisionOverflow))?;
        let event = SessionEvent::CriterionEvaluated {
            session_id: session_id.clone(),
            invocation,
            evidence,
        };
        entries.push(JournalEntry::new(next, event.clone()));
        let validated =
            replay_session(session_id, &entries)?.expect("existing session remains open");
        commit_evidence(self.journal, session_id, expected_revision, next, event)?;
        Ok(validated)
    }
}

fn commit_evidence<J: Journal>(
    journal: &J,
    session_id: &SessionId,
    expected: Revision,
    next: Revision,
    event: SessionEvent,
) -> Result<(), DispatchError> {
    let verify_revision = |actual| {
        if actual == next {
            Ok(())
        } else {
            Err(DispatchError::Journal(JournalError::AdapterFailure {
                message: format!(
                    "journal returned revision {actual} after committing expected revision {next}"
                ),
            }))
        }
    };
    match journal.append(session_id, expected, vec![event.clone()]) {
        Ok(actual) => verify_revision(actual),
        Err(JournalError::StaleRevision { expected, actual }) => {
            Err(DispatchError::StaleRevision { expected, actual })
        }
        Err(JournalError::IndeterminateCommit { attempted, .. }) => {
            let history = journal.read(session_id)?;
            let head = history
                .last()
                .map_or(Revision::INITIAL, JournalEntry::revision);
            if head == attempted && history.last().is_some_and(|entry| entry.event() == &event) {
                return verify_revision(attempted);
            }
            if head == expected {
                return match journal.append(session_id, expected, vec![event]) {
                    Ok(actual) => verify_revision(actual),
                    Err(JournalError::StaleRevision { expected, actual }) => {
                        Err(DispatchError::StaleRevision { expected, actual })
                    }
                    Err(error) => Err(DispatchError::Journal(error)),
                };
            }
            Err(DispatchError::StaleRevision {
                expected,
                actual: head,
            })
        }
        Err(error) => Err(DispatchError::Journal(error)),
    }
}
