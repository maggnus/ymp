//! Indeterminate-commit behavior and resolution (DEV-0005 acceptance
//! criterion 3): an injected failure after the commit-descriptor rename
//! returns `IndeterminateCommit` carrying `expected` and `attempted`, and the
//! resolution rule then decides by comparing the stored content with the
//! submitted batch — a fully matching range is not resubmitted, a retry runs
//! only while the history still ends at the original `expected`, and any
//! other advancement is an explicit `StaleRevision`.

#![forbid(unsafe_code)]

mod support;

use std::fs;

use support::{TempRoot, cancelled_event, open_dispatcher, sample_task, session_id};
use ymp_kernel::{Journal, JournalError, Revision, SessionStatus};
use ymp_storage::{FileJournal, InjectedFault};

#[test]
fn injected_post_rename_failure_is_indeterminate_commit() {
    let root = TempRoot::new("indeterminate");
    let sid = session_id("indeterminate");

    // The first batch is written by a healthy handle; only the second append
    // runs with the fault installed, so the fault's first firing targets it.
    {
        let dispatcher = open_dispatcher(root.path());
        dispatcher
            .open(sid.clone(), sample_task("indeterminate"))
            .expect("session opens");
    }

    let faulting =
        FileJournal::open_with_injected_fault(root.path(), InjectedFault::IndeterminateAfterCommit)
            .expect("faulting journal opens");
    let failure = faulting
        .append(&sid, Revision::new(1), vec![cancelled_event(&sid)])
        .unwrap_err();

    // `attempted` is the last revision of the submitted batch, not an
    // acknowledgment of writing.
    assert_eq!(
        failure,
        JournalError::IndeterminateCommit {
            expected: Revision::new(1),
            attempted: Revision::new(2),
            message: String::from(
                "injected failure after the commit descriptor rename: the \
                 sessions-directory sync was not performed"
            )
        }
    );

    // Resolution: the durable stream shows the batch at exactly
    // expected+1..attempted and it matches the submission — it is committed
    // and is not resubmitted.
    let resolver = FileJournal::open(root.path()).expect("journal opens");
    let entries = resolver.read(&sid).expect("history reads");
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].revision(), Revision::new(1));
    assert_eq!(entries[1].revision(), Revision::new(2));
    assert_eq!(entries[1].event(), &cancelled_event(&sid));

    // Resubmitting the same batch is an explicit StaleRevision: the history
    // advanced past the original expected revision.
    assert_eq!(
        resolver
            .append(&sid, Revision::new(1), vec![cancelled_event(&sid)])
            .unwrap_err(),
        JournalError::StaleRevision {
            expected: Revision::new(1),
            actual: Revision::new(2)
        }
    );

    let dispatcher = open_dispatcher(root.path());
    let view = dispatcher.read(&sid).expect("session reads");
    assert_eq!(view.status(), SessionStatus::Cancelled);
    assert_eq!(view.revision(), Revision::new(2));
}

#[test]
fn indeterminate_absent_outcome_allows_one_retry_at_the_same_expected() {
    let root = TempRoot::new("indeterminate-absent");
    let sid = session_id("absent");

    {
        let dispatcher = open_dispatcher(root.path());
        dispatcher
            .open(sid.clone(), sample_task("absent"))
            .expect("session opens");
    }

    // The prefix as it stands before the indeterminate append.
    let descriptor_path = support::commit_path(root.path(), sid.as_str());
    let prefix_descriptor = fs::read(&descriptor_path).expect("descriptor reads");

    // The append reaches the commit rename and then reports
    // IndeterminateCommit.
    let faulting =
        FileJournal::open_with_injected_fault(root.path(), InjectedFault::IndeterminateAfterCommit)
            .expect("faulting journal opens");
    assert_eq!(
        faulting
            .append(&sid, Revision::new(1), vec![cancelled_event(&sid)])
            .unwrap_err(),
        JournalError::IndeterminateCommit {
            expected: Revision::new(1),
            attempted: Revision::new(2),
            message: String::from(
                "injected failure after the commit descriptor rename: the \
                 sessions-directory sync was not performed"
            )
        }
    );

    // The other way the store can settle after an indeterminate commit: the
    // rename did not survive. Restore the previous prefix descriptor, exactly
    // the "absent" branch of criterion 3.
    fs::write(&descriptor_path, prefix_descriptor).expect("settled-absent descriptor writes");

    // Reopen: recovery truncates the now-uncommitted batch suffix, so the
    // history still ends at the original expected revision.
    let journal = FileJournal::open(root.path()).expect("journal opens");
    let entries = journal.read(&sid).expect("history reads");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].revision(), Revision::new(1));

    // The retry of the same batch is allowed, uses the same expected
    // revision, and the batch lands exactly once.
    let attempted = journal
        .append(&sid, Revision::new(1), vec![cancelled_event(&sid)])
        .expect("retry with the same expected commits");
    assert_eq!(attempted, Revision::new(2));

    let dispatcher = open_dispatcher(root.path());
    let view = dispatcher.read(&sid).expect("session reads");
    assert_eq!(view.status(), SessionStatus::Cancelled);
    assert_eq!(view.revision(), Revision::new(2));
    let entries = FileJournal::open(root.path())
        .and_then(|handle| handle.read(&sid))
        .expect("history reads");
    assert_eq!(entries.len(), 2, "the retry landed exactly once");
}

#[test]
fn retry_is_refused_once_the_history_advances() {
    let root = TempRoot::new("retry-refused");
    let sid = session_id("advanced");

    {
        let dispatcher = open_dispatcher(root.path());
        dispatcher
            .open(sid.clone(), sample_task("advanced"))
            .expect("session opens");
    }

    // Another writer advances the history past the original expected
    // revision.
    let winner = FileJournal::open(root.path()).expect("journal opens");
    winner
        .append(&sid, Revision::new(1), vec![cancelled_event(&sid)])
        .expect("the winner commits");

    // A late retry of the same batch at the original expected revision is an
    // explicit StaleRevision, never a raise of the expected revision.
    let late = FileJournal::open(root.path()).expect("journal opens");
    assert_eq!(
        late.append(&sid, Revision::new(1), vec![cancelled_event(&sid)])
            .unwrap_err(),
        JournalError::StaleRevision {
            expected: Revision::new(1),
            actual: Revision::new(2)
        }
    );
}

#[test]
fn unacknowledged_batch_is_discarded_and_retryable_after_adapter_failure() {
    let root = TempRoot::new("unacknowledged");
    let sid = session_id("unacknowledged");

    {
        let dispatcher = open_dispatcher(root.path());
        dispatcher
            .open(sid.clone(), sample_task("unacknowledged"))
            .expect("session opens");
    }

    // An AdapterFailure before the commit point leaves the batch
    // unacknowledged; after reopen the history still ends at the original
    // expected revision, so the same batch may be retried.
    let faulting = FileJournal::open_with_injected_fault(
        root.path(),
        InjectedFault::AdapterFailureBeforeCommit,
    )
    .expect("faulting journal opens");
    assert!(matches!(
        faulting.append(&sid, Revision::new(1), vec![cancelled_event(&sid)]),
        Err(JournalError::AdapterFailure { .. })
    ));
    drop(faulting);

    let retry = FileJournal::open(root.path()).expect("journal opens");
    assert_eq!(
        retry
            .append(&sid, Revision::new(1), vec![cancelled_event(&sid)])
            .expect("the retry commits"),
        Revision::new(2)
    );
    let entries = retry.read(&sid).expect("history reads");
    assert_eq!(entries.len(), 2);
}
