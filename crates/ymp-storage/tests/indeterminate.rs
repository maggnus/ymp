//! Indeterminate-commit outcomes (DEV-0005 acceptance criterion 4).
//!
//! Both sides of an unproven commit are exercised through the public
//! fault-injection seam, and each is resolved by the contract's rule: compare
//! the attempted revision range and the exact events; never blindly resubmit a
//! visible batch; retry only while the history still ends at the original
//! `expected`; any other advancement is an explicit `StaleRevision`.
//!
//! Evidence boundary: these checks prove the adapter's reported outcomes and
//! the resolution rule for the injected states. They do not inject real power
//! loss, process kills inside SQLite's commit, or VFS-level I/O errors; that
//! behavior is not experimentally covered here.

#![forbid(unsafe_code)]

mod support;

use support::{TempRoot, open_dispatcher, open_journal, sample_task, session_id};
use ymp_kernel::{Journal, JournalError, Revision, SessionStatus};

#[test]
fn visible_indeterminate_commit_is_not_resubmitted() {
    let root = TempRoot::new("indeterminate-visible");
    let sid = session_id("indeterminate-visible");
    let dispatcher = open_dispatcher(root.path());
    dispatcher
        .open(sid.clone(), sample_task("visible"))
        .expect("session opens");

    // The commit lands, but the caller is told the outcome is unknown.
    let faulting = open_journal(root.path());
    faulting.inject_fault(ymp_storage::FaultPoint::IndeterminateCommitted);
    let error = faulting
        .append(
            &sid,
            Revision::new(1),
            vec![support::cancelled_event(&sid)],
        )
        .unwrap_err();
    let (expected, attempted) = match &error {
        JournalError::IndeterminateCommit {
            expected,
            attempted,
            message,
        } => {
            assert!(message.contains("committed"), "message: {message}");
            (*expected, *attempted)
        }
        other => panic!("expected IndeterminateCommit, got {other:?}"),
    };
    assert_eq!(expected, Revision::new(1));
    assert_eq!(attempted, Revision::new(2));

    // Resolution: read the attempted range and compare the exact events.
    let entries = faulting.read(&sid).expect("history reads");
    let range: Vec<_> = entries
        .iter()
        .filter(|entry| {
            let value = entry.revision().value();
            value > expected.value() && value <= attempted.value()
        })
        .map(|entry| entry.event().clone())
        .collect();
    assert_eq!(
        range,
        vec![support::cancelled_event(&sid)],
        "the stored range fully matches the submitted batch: it is committed"
    );

    // A blind resubmission at the same expected revision can only collide
    // with the durable state — exactly the explicit StaleRevision the rule
    // requires instead of a duplicate write.
    assert_eq!(
        faulting
            .append(
                &sid,
                Revision::new(1),
                vec![support::cancelled_event(&sid)]
            )
            .unwrap_err(),
        JournalError::StaleRevision {
            expected: Revision::new(1),
            actual: Revision::new(2)
        }
    );
    assert_eq!(
        faulting.read(&sid).expect("history reads").len(),
        2,
        "the batch is stored exactly once"
    );
}

#[test]
fn absent_indeterminate_commit_retries_once_at_the_same_expected() {
    let root = TempRoot::new("indeterminate-absent");
    let sid = session_id("indeterminate-absent");
    let dispatcher = open_dispatcher(root.path());
    dispatcher
        .open(sid.clone(), sample_task("absent"))
        .expect("session opens");

    // Nothing lands, but the caller is told the outcome is unknown.
    let faulting = open_journal(root.path());
    faulting.inject_fault(ymp_storage::FaultPoint::IndeterminateRolledBack);
    let error = faulting
        .append(&sid, Revision::new(1), vec![support::cancelled_event(&sid)])
        .unwrap_err();
    match &error {
        JournalError::IndeterminateCommit {
            expected,
            attempted,
            message,
        } => {
            assert_eq!(*expected, Revision::new(1));
            assert_eq!(*attempted, Revision::new(2));
            assert!(message.contains("absent"), "message: {message}");
        }
        other => panic!("expected IndeterminateCommit, got {other:?}"),
    }

    // Resolution: the history still ends at the original expected revision,
    // so a retry at the SAME expected is admissible and lands exactly once.
    let entries = faulting.read(&sid).expect("history reads");
    assert_eq!(
        entries.last().map(|entry| entry.revision()),
        Some(Revision::new(1)),
        "the history still ends at the original expected revision"
    );
    let retried = faulting
        .append(&sid, Revision::new(1), vec![support::cancelled_event(&sid)])
        .expect("retry at the same expected lands");
    assert_eq!(retried, Revision::new(2));
    let entries = faulting.read(&sid).expect("history reads");
    assert_eq!(entries.len(), 2, "the retried batch is stored exactly once");
}

#[test]
fn absent_indeterminate_commit_with_advanced_history_is_stale() {
    let root = TempRoot::new("indeterminate-advanced");
    let sid = session_id("indeterminate-advanced");
    let dispatcher = open_dispatcher(root.path());
    dispatcher
        .open(sid.clone(), sample_task("advanced"))
        .expect("session opens");

    let faulting = open_journal(root.path());
    faulting.inject_fault(ymp_storage::FaultPoint::IndeterminateRolledBack);
    let error = faulting
        .append(&sid, Revision::new(1), vec![support::cancelled_event(&sid)])
        .unwrap_err();
    assert!(matches!(
        &error,
        JournalError::IndeterminateCommit { .. }
    ));

    // A different writer advances the history past the original expected
    // revision before the retry.
    let other = open_journal(root.path());
    other
        .append(&sid, Revision::new(1), vec![support::cancelled_event(&sid)])
        .expect("the other writer wins");

    // The resolution rule: any other advancement of the history is an
    // explicit StaleRevision, never a raised expected revision.
    assert_eq!(
        faulting
            .append(&sid, Revision::new(1), vec![support::cancelled_event(&sid)])
            .unwrap_err(),
        JournalError::StaleRevision {
            expected: Revision::new(1),
            actual: Revision::new(2)
        }
    );
    assert_eq!(
        open_dispatcher(root.path()).read(&sid).unwrap().status(),
        SessionStatus::Cancelled
    );
}
