//! Competing writers (DEV-0005 acceptance criterion 5): of two simultaneous
//! appends at the same expected revision in two real OS processes on one
//! database, exactly one returns the new revision; the other receives
//! `StaleRevision` with the winner's revision; the winner's batch appears
//! exactly once.

#![forbid(unsafe_code)]

mod support;

use std::fs;
use std::sync::{Arc, Barrier};
use std::thread;

use support::{TempRoot, open_dispatcher, open_journal, sample_task, session_id};
use ymp_kernel::{DispatchError, Journal, JournalError, Revision, SessionStatus};

const CHILD_TEST: &str = "two_os_processes_compete_on_one_database";

#[test]
fn two_os_processes_compete_on_one_database() {
    if let Some(mode) = support::child_mode() {
        child_scenario(&mode);
        return;
    }

    let root = TempRoot::new("competing");
    let sid = session_id("competing");
    let barrier = root.path().join("barrier");

    // The session already rests at revision 1.
    let dispatcher = open_dispatcher(root.path());
    dispatcher
        .open(sid.clone(), sample_task("competing"))
        .expect("session opens");

    // Spawn a real second process that waits for the barrier and then appends
    // a cancel at the same expected revision.
    let child = support::spawn_child(
        CHILD_TEST,
        "compete",
        &[
            ("YMP_TEST_ROOT", root.path().to_string_lossy().into()),
            ("YMP_TEST_BARRIER", barrier.to_string_lossy().into()),
        ],
    );

    // Release the barrier and immediately compete with the child.
    fs::write(&barrier, b"go").expect("barrier releases");
    let parent_outcome = match dispatcher.cancel(&sid, Revision::new(1)) {
        Ok(view) => format!("OK {}", view.revision().value()),
        Err(DispatchError::StaleRevision { expected, actual }) => {
            format!("STALE {} {}", expected.value(), actual.value())
        }
        Err(other) => panic!("parent append failed unexpectedly: {other}"),
    };

    let child_outcome = child.join().expect("child scenario completes");
    let mut outcomes = vec![parent_outcome, child_outcome];
    outcomes.sort();
    assert_eq!(
        outcomes,
        vec!["OK 2".to_owned(), "STALE 1 2".to_owned()],
        "exactly one append commits; the loser receives the winner's revision"
    );

    // The winner's batch appears exactly once.
    let entries = open_journal(root.path()).read(&sid).expect("history reads");
    assert_eq!(entries.len(), 2, "the winner's batch appears exactly once");
    assert_eq!(entries[1].revision(), Revision::new(2));
    assert_eq!(entries[1].event(), &support::cancelled_event(&sid));
    let view = open_dispatcher(root.path())
        .read(&sid)
        .expect("session reads");
    assert_eq!(view.status(), SessionStatus::Cancelled);
    assert_eq!(view.revision(), Revision::new(2));
}

fn child_scenario(mode: &str) {
    assert_eq!(mode, "compete");
    let root: std::path::PathBuf = support::child_var("YMP_TEST_ROOT").into();
    let barrier: std::path::PathBuf = support::child_var("YMP_TEST_BARRIER").into();
    support::wait_for_barrier(&barrier);
    let sid = session_id("competing");
    let journal = open_journal(&root);
    let outcome = match journal.append(&sid, Revision::new(1), vec![support::cancelled_event(&sid)])
    {
        Ok(revision) => format!("OK {}", revision.value()),
        Err(JournalError::StaleRevision { expected, actual }) => {
            format!("STALE {} {}", expected.value(), actual.value())
        }
        Err(other) => panic!("child append failed unexpectedly: {other}"),
    };
    support::write_outcome(&outcome);
}

#[test]
fn in_process_threads_on_separate_handles_also_serialize() {
    let root = TempRoot::new("threads");
    let sid = session_id("threads");

    {
        let dispatcher = open_dispatcher(root.path());
        dispatcher
            .open(sid.clone(), sample_task("threads"))
            .expect("session opens");
    }

    // Separate journal handles (separate connections) in this process must
    // still serialize on the same database: one winner, typed losers.
    let barrier = Arc::new(Barrier::new(8));
    let handles: Vec<_> = (0..8)
        .map(|_| {
            let barrier = Arc::clone(&barrier);
            let root = root.path().to_path_buf();
            let sid = sid.clone();
            thread::spawn(move || {
                barrier.wait();
                let journal = open_journal(&root);
                match journal.append(&sid, Revision::new(1), vec![support::cancelled_event(&sid)]) {
                    Ok(revision) => format!("OK {}", revision.value()),
                    Err(JournalError::StaleRevision { expected, actual }) => {
                        format!("STALE {} {}", expected.value(), actual.value())
                    }
                    Err(other) => panic!("thread append failed unexpectedly: {other}"),
                }
            })
        })
        .collect();
    let mut outcomes: Vec<String> = handles
        .into_iter()
        .map(|handle| handle.join().expect("thread completes"))
        .collect();
    outcomes.sort();
    assert_eq!(outcomes[0], "OK 2");
    assert!(
        outcomes[1..].iter().all(|outcome| outcome == "STALE 1 2"),
        "every loser receives the winner's revision: {outcomes:?}"
    );

    let entries = open_journal(root.path()).read(&sid).expect("history reads");
    assert_eq!(entries.len(), 2, "the winner's batch appears exactly once");
}
