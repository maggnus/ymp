//! Whole-batch atomicity and injected failure (DEV-0005 acceptance criterion
//! 3): a batch appears as contiguous ordered revisions or not at all, an
//! injected pre-commit failure returns `AdapterFailure` with the durable
//! stream unchanged, and `EmptyBatch` touches no storage.

#![forbid(unsafe_code)]

mod support;

use support::{TempRoot, open_dispatcher, open_journal, sample_task, session_id};
use ymp_kernel::{Dispatcher, Journal, JournalError, Revision};

#[test]
fn multi_event_batch_lands_as_contiguous_revisions() {
    let root = TempRoot::new("batch");
    let sid = session_id("batch");
    // A journal-level batch of two events (the journal port admits any batch;
    // domain admission stays with the kernel).
    let events = vec![
        support::opened_event(&sid, &sample_task("batch")),
        support::cancelled_event(&sid),
    ];
    let journal = open_journal(root.path());
    let committed = journal
        .append(&sid, Revision::INITIAL, events.clone())
        .expect("batch commits");
    assert_eq!(committed, Revision::new(2));

    let entries = journal.read(&sid).expect("history reads");
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].revision(), Revision::new(1));
    assert_eq!(entries[0].event(), &events[0]);
    assert_eq!(entries[1].revision(), Revision::new(2));
    assert_eq!(entries[1].event(), &events[1]);
}

#[test]
fn empty_batch_is_rejected_without_touching_storage() {
    let root = TempRoot::new("empty");
    let sid = session_id("empty");
    let journal = open_journal(root.path());
    // Let open-time side files settle, then prove the rejected append writes
    // nothing at all.
    std::thread::sleep(std::time::Duration::from_millis(20));
    let before = support::snapshot_tree(root.path());
    assert_eq!(
        journal
            .append(&sid, Revision::INITIAL, Vec::new())
            .unwrap_err(),
        JournalError::EmptyBatch
    );
    assert_eq!(
        support::snapshot_tree(root.path()),
        before,
        "EmptyBatch writes no byte"
    );
}

#[test]
fn injected_pre_commit_failure_leaves_no_partial_result() {
    let root = TempRoot::new("pre-commit");
    let sid = session_id("pre-commit");
    let task = sample_task("pre-commit");

    let dispatcher = open_dispatcher(root.path());
    let opened = dispatcher
        .open(sid.clone(), task.clone())
        .expect("session opens");
    assert_eq!(opened.revision(), Revision::new(1));
    let before = dispatcher.read(&sid).expect("prior history reads");

    // Inject a failure after the batch rows are written but before the
    // transaction commits: the rollback is confirmed and the failure is an
    // AdapterFailure, never a partial committed result.
    let faulting = open_journal(root.path());
    faulting.inject_fault(ymp_storage::FaultPoint::BeforeCommit);
    let error = Dispatcher::new(faulting)
        .cancel(&sid, Revision::new(1))
        .unwrap_err();
    assert!(
        matches!(
            error,
            ymp_kernel::DispatchError::Journal(JournalError::AdapterFailure { .. })
        ),
        "expected AdapterFailure, got {error:?}"
    );

    // A subsequent read returns the identical prior history and the rows are
    // physically absent. (The WAL side file may retain the aborted frames;
    // what matters is that no partial batch is committed or observable.)
    let after = dispatcher.read(&sid).expect("history still reads");
    assert_eq!(after, before, "the durable stream is unchanged");
    let connection = support::direct_connection(root.path());
    let rows: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM journal_entries WHERE session_id = ?1",
            [sid.as_str()],
            |row| row.get(0),
        )
        .expect("rows count");
    assert_eq!(rows, 1, "no partial batch row was committed");

    // The seam fires once: a retry without re-arming succeeds normally.
    let retried = open_dispatcher(root.path())
        .cancel(&sid, Revision::new(1))
        .expect("retry after the injected failure commits");
    assert_eq!(retried.revision(), Revision::new(2));
}
