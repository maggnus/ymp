#![forbid(unsafe_code)]

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Barrier, Mutex};
use std::thread;

use ymp_runtime::{
    AcceptanceContract, Application, Constraints, Criterion, CriterionId, DispatchError, Goal,
    Journal, JournalEntry, JournalError, MemoryJournal, Revision, SessionEvent, SessionId,
    SessionStatus, Task, TaskId,
};

fn session_id(value: &str) -> SessionId {
    SessionId::new(value).expect("valid session ID")
}

fn task(value: &str) -> Task {
    Task::new(
        TaskId::new(format!("task-{value}")).expect("valid task ID"),
        Goal::new(format!("  Produce {value} exactly.\n")).expect("valid goal"),
        AcceptanceContract::new(vec![
            Criterion::new(
                CriterionId::new("content").expect("valid criterion ID"),
                "Content matches the request.  ",
            )
            .expect("valid criterion"),
            Criterion::new(
                CriterionId::new("check").expect("valid criterion ID"),
                "A deterministic check passes.",
            )
            .expect("valid criterion"),
        ])
        .expect("valid acceptance contract"),
        Constraints::new(vec![
            "Do not access the network.".to_owned(),
            "  Retain surrounding spaces.  ".to_owned(),
        ])
        .expect("valid constraints"),
    )
}

#[test]
fn dispatcher_and_memory_journal_preserve_the_complete_task_lifecycle() {
    let journal = MemoryJournal::new();
    let application = Application::new(journal.clone());
    let id = session_id("lifecycle");
    let expected_task = task("a checked result");

    let opened = application
        .open_session(id.clone(), expected_task.clone())
        .expect("session opens");
    assert_eq!(opened.session_id(), &id);
    assert_eq!(opened.task(), &expected_task);
    assert_eq!(opened.status(), SessionStatus::Open);
    assert_eq!(opened.revision(), Revision::new(1));

    let read = application.read_session(&id).expect("session reads");
    assert_eq!(read, opened);

    let cancelled = application
        .cancel_session(&id, opened.revision())
        .expect("open session cancels");
    assert_eq!(cancelled.task(), &expected_task);
    assert_eq!(cancelled.status(), SessionStatus::Cancelled);
    assert_eq!(cancelled.revision(), Revision::new(2));

    assert_eq!(opened.status(), SessionStatus::Open);
    assert_eq!(opened.revision(), Revision::new(1));
    assert_eq!(
        application.read_session(&id).expect("session reads"),
        cancelled
    );
    assert_eq!(journal.read(&id).expect("history reads").len(), 2);
}

#[test]
fn denied_lifecycle_operations_leave_history_unchanged() {
    let journal = MemoryJournal::new();
    let application = Application::new(journal.clone());
    let id = session_id("denials");
    let expected_task = task("one result");
    let opened = application
        .open_session(id.clone(), expected_task.clone())
        .expect("session opens");

    let one_entry = journal.read(&id).expect("history reads");
    assert!(matches!(
        application.open_session(id.clone(), expected_task),
        Err(DispatchError::SessionAlreadyExists { revision, .. })
            if revision == Revision::new(1)
    ));
    assert_eq!(journal.read(&id).expect("history reads"), one_entry);

    assert_eq!(
        application.cancel_session(&id, Revision::INITIAL),
        Err(DispatchError::StaleRevision {
            expected: Revision::INITIAL,
            actual: Revision::new(1),
        })
    );
    assert_eq!(journal.read(&id).expect("history reads"), one_entry);

    let cancelled = application
        .cancel_session(&id, opened.revision())
        .expect("session cancels");
    let two_entries = journal.read(&id).expect("history reads");
    assert_eq!(two_entries.len(), 2);
    assert!(matches!(
        application.cancel_session(&id, cancelled.revision()),
        Err(DispatchError::SessionAlreadyCancelled { revision, .. })
            if revision == Revision::new(2)
    ));
    assert_eq!(journal.read(&id).expect("history reads"), two_entries);

    let unknown = session_id("unknown");
    assert!(matches!(
        application.read_session(&unknown),
        Err(DispatchError::SessionNotFound { .. })
    ));
    assert!(matches!(
        application.cancel_session(&unknown, Revision::INITIAL),
        Err(DispatchError::SessionNotFound { .. })
    ));
    assert!(journal.read(&unknown).expect("history reads").is_empty());
}

#[test]
fn memory_journal_rejects_empty_and_stale_batches_without_partial_state() {
    let journal = MemoryJournal::new();
    let id = session_id("atomic-errors");

    assert_eq!(
        journal.append(&id, Revision::INITIAL, Vec::new()),
        Err(JournalError::EmptyBatch)
    );
    assert!(journal.read(&id).expect("history reads").is_empty());

    let events = vec![SessionEvent::SessionOpened {
        session_id: id.clone(),
        task: task("atomic state"),
    }];
    assert_eq!(
        journal.append(&id, Revision::new(4), events),
        Err(JournalError::StaleRevision {
            expected: Revision::new(4),
            actual: Revision::INITIAL,
        })
    );
    assert!(journal.read(&id).expect("history reads").is_empty());
}

#[test]
fn memory_journal_commits_a_nonempty_batch_with_contiguous_revisions() {
    let journal = MemoryJournal::new();
    let id = session_id("batch");
    let events = vec![
        SessionEvent::SessionOpened {
            session_id: id.clone(),
            task: task("batched state"),
        },
        SessionEvent::SessionCancelled {
            session_id: id.clone(),
        },
    ];

    assert_eq!(
        journal.append(&id, Revision::INITIAL, events),
        Ok(Revision::new(2))
    );
    let history = journal.read(&id).expect("history reads");
    assert_eq!(
        history
            .iter()
            .map(|entry| entry.revision())
            .collect::<Vec<_>>(),
        vec![Revision::new(1), Revision::new(2)]
    );
}

#[test]
fn concurrent_expected_revision_admission_has_one_winner() {
    let journal = MemoryJournal::new();
    let id = session_id("concurrent");
    let barrier = Arc::new(Barrier::new(3));
    let mut handles = Vec::new();

    for label in ["first", "second"] {
        let worker_journal = journal.clone();
        let worker_id = id.clone();
        let worker_barrier = Arc::clone(&barrier);
        handles.push(thread::spawn(move || {
            let event = SessionEvent::SessionOpened {
                session_id: worker_id.clone(),
                task: task(label),
            };
            worker_barrier.wait();
            worker_journal.append(&worker_id, Revision::INITIAL, vec![event])
        }));
    }

    barrier.wait();
    let results: Vec<_> = handles
        .into_iter()
        .map(|handle| handle.join().expect("worker does not panic"))
        .collect();

    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter(|result| matches!(
                result,
                Err(JournalError::StaleRevision {
                    expected: Revision::INITIAL,
                    actual,
                }) if *actual == Revision::new(1)
            ))
            .count(),
        1
    );
    assert_eq!(journal.read(&id).expect("history reads").len(), 1);
}

const UNAVAILABLE_MESSAGE: &str = "alternate journal is unavailable by design";

/// Test double storing one flat append-only log instead of per-session maps.
#[derive(Clone, Debug, Default)]
struct FlatLogJournal {
    log: Arc<Mutex<Vec<(SessionId, JournalEntry)>>>,
}

impl Journal for FlatLogJournal {
    fn read(&self, session_id: &SessionId) -> Result<Vec<JournalEntry>, JournalError> {
        let log = self.log.lock().expect("flat log lock is available");
        Ok(log
            .iter()
            .filter(|(stream, _)| stream == session_id)
            .map(|(_, entry)| entry.clone())
            .collect())
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

        let mut log = self.log.lock().expect("flat log lock is available");
        let actual_revision = log
            .iter()
            .rev()
            .find(|(stream, _)| stream == session_id)
            .map_or(Revision::INITIAL, |(_, entry)| entry.revision());
        if actual_revision != expected_revision {
            return Err(JournalError::StaleRevision {
                expected: expected_revision,
                actual: actual_revision,
            });
        }

        let mut next_revision = actual_revision;
        for event in events {
            next_revision = next_revision
                .checked_next()
                .ok_or(JournalError::RevisionOverflow)?;
            log.push((session_id.clone(), JournalEntry::new(next_revision, event)));
        }
        Ok(next_revision)
    }
}

/// Test double whose adapter fails every operation with one typed error.
#[derive(Clone, Debug, Default)]
struct UnavailableJournal {
    read_attempts: Arc<AtomicUsize>,
    append_attempts: Arc<AtomicUsize>,
}

impl UnavailableJournal {
    fn read_attempts(&self) -> usize {
        self.read_attempts.load(Ordering::SeqCst)
    }

    fn append_attempts(&self) -> usize {
        self.append_attempts.load(Ordering::SeqCst)
    }
}

impl Journal for UnavailableJournal {
    fn read(&self, _session_id: &SessionId) -> Result<Vec<JournalEntry>, JournalError> {
        self.read_attempts.fetch_add(1, Ordering::SeqCst);
        Err(JournalError::AdapterFailure {
            message: UNAVAILABLE_MESSAGE.to_owned(),
        })
    }

    fn append(
        &self,
        _session_id: &SessionId,
        _expected_revision: Revision,
        _events: Vec<SessionEvent>,
    ) -> Result<Revision, JournalError> {
        self.append_attempts.fetch_add(1, Ordering::SeqCst);
        Err(JournalError::AdapterFailure {
            message: UNAVAILABLE_MESSAGE.to_owned(),
        })
    }
}

#[test]
fn application_runs_the_task_lifecycle_over_an_alternative_journal_adapter() {
    let journal = FlatLogJournal::default();
    let application = Application::new(journal.clone());
    let first_id = session_id("flat-first");
    let second_id = session_id("flat-second");
    let expected_task = task("an alternative adapter");

    let opened = application
        .open_session(first_id.clone(), expected_task.clone())
        .expect("session opens through the alternative adapter");
    assert_eq!(opened.session_id(), &first_id);
    assert_eq!(opened.task(), &expected_task);
    assert_eq!(opened.status(), SessionStatus::Open);
    assert_eq!(opened.revision(), Revision::new(1));

    assert_eq!(
        application.read_session(&first_id).expect("session reads"),
        opened
    );

    let second = application
        .open_session(second_id.clone(), task("a second stream"))
        .expect("second session opens in the same log");
    let cancelled = application
        .cancel_session(&first_id, opened.revision())
        .expect("open session cancels through the alternative adapter");
    assert_eq!(cancelled.status(), SessionStatus::Cancelled);
    assert_eq!(cancelled.revision(), Revision::new(2));
    assert_eq!(
        application.read_session(&first_id).expect("session reads"),
        cancelled
    );
    assert_eq!(
        application.read_session(&second_id).expect("session reads"),
        second
    );

    let log = journal.log.lock().expect("flat log lock is available");
    assert_eq!(log.len(), 3);
    assert_eq!(
        log.iter()
            .filter(|(stream, _)| stream == &first_id)
            .map(|(_, entry)| entry.revision())
            .collect::<Vec<_>>(),
        vec![Revision::new(1), Revision::new(2)]
    );
}

#[test]
fn application_surfaces_alternate_adapter_typed_failures_at_the_public_boundary() {
    let journal = UnavailableJournal::default();
    let application = Application::new(journal.clone());
    let id = session_id("unavailable");
    let adapter_failure = DispatchError::Journal(JournalError::AdapterFailure {
        message: UNAVAILABLE_MESSAGE.to_owned(),
    });

    assert_eq!(
        application.open_session(id.clone(), task("unavailable adapter")),
        Err(adapter_failure.clone())
    );
    assert_eq!(application.read_session(&id), Err(adapter_failure.clone()));
    assert_eq!(
        application.cancel_session(&id, Revision::INITIAL),
        Err(adapter_failure)
    );

    assert_eq!(journal.read_attempts(), 3);
    assert_eq!(journal.append_attempts(), 0);
}

#[test]
fn memory_entry_points_preserve_the_current_application_assembly() {
    let defaulted: Application = Application::default();
    let explicit: Application<MemoryJournal> = Application::in_memory();
    let constructed: Application<MemoryJournal> = Application::new(MemoryJournal::new());
    let id = session_id("memory-entry-points");
    let expected_task = task("in-memory entry point");

    for application in [&defaulted, &explicit, &constructed] {
        let opened = application
            .open_session(id.clone(), expected_task.clone())
            .expect("in-memory session opens");
        assert_eq!(opened.revision(), Revision::new(1));
        assert_eq!(
            application
                .cancel_session(&id, opened.revision())
                .expect("in-memory session cancels")
                .revision(),
            Revision::new(2)
        );
    }
}
