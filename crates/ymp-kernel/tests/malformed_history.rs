#![forbid(unsafe_code)]

use ymp_domain::{
    AcceptanceContract, Constraints, Criterion, CriterionId, Goal, SessionId, Task, TaskId,
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
    );
}
