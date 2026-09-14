#![forbid(unsafe_code)]

use std::sync::{Arc, Barrier};
use std::thread;

use ymp_runtime::{
    AcceptanceContract, Application, Constraints, Criterion, CriterionId, DispatchError, Goal,
    Journal, JournalError, MemoryJournal, Revision, SessionEvent, SessionId, SessionStatus, Task,
    TaskId,
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
