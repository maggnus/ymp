//! Restart replay across real processes (DEV-0005 acceptance criteria 1 and
//! 2): a session opened in one process that exits reads back exactly in a
//! fresh process, and a cancellation stays visible after another reopen.
//! Exercises the kernel `Dispatcher` over the SQLite adapter.

#![forbid(unsafe_code)]

mod support;

use support::{TempRoot, open_dispatcher, open_journal, opened_event, sample_task, session_id};
use ymp_kernel::{DispatchError, Journal, Revision, SessionStatus};

const CHILD_TEST: &str = "restart_replay_uses_real_child_processes";

#[test]
fn restart_replay_uses_real_child_processes() {
    // Parent side: a child process performs the write and exits; this process
    // and a second child perform independent reads.
    if let Some(mode) = support::child_mode() {
        child_scenario(&mode);
        return;
    }

    let root = TempRoot::new("restart");
    let sid = session_id("restart-session");
    let task = sample_task("restart");

    // Process 1: open the session, then the process exits.
    let outcome = support::run_child(
        CHILD_TEST,
        "open",
        &[("YMP_TEST_ROOT", root.path().to_string_lossy().into())],
    );
    assert_eq!(outcome, "OPENED 1");

    // Process 2 (this one): reopen from the same root and read back the exact
    // identity, task content, status and revision.
    let dispatcher = open_dispatcher(root.path());
    let view = dispatcher.read(&sid).expect("session reads after restart");
    assert_eq!(view.session_id(), &sid);
    assert_eq!(view.task(), &task, "the exact task content survives restart");
    assert_eq!(view.status(), SessionStatus::Open);
    assert_eq!(view.revision(), Revision::new(1));
    let entries = open_journal(root.path())
        .read(&sid)
        .expect("raw history reads");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].revision(), Revision::new(1));
    assert_eq!(entries[0].event(), &opened_event(&sid, &task));

    // An unknown identifier still yields SessionNotFound.
    assert_eq!(
        dispatcher.read(&session_id("never-opened")).unwrap_err(),
        DispatchError::SessionNotFound {
            session_id: session_id("never-opened")
        }
    );

    // Cancel in this process, then verify the cancellation is visible to a
    // third, fresh process reading the same root.
    let cancelled = dispatcher
        .cancel(&sid, Revision::new(1))
        .expect("cancel commits");
    assert_eq!(cancelled.status(), SessionStatus::Cancelled);
    assert_eq!(cancelled.revision(), Revision::new(2));

    let outcome = support::run_child(
        CHILD_TEST,
        "read-cancelled",
        &[("YMP_TEST_ROOT", root.path().to_string_lossy().into())],
    );
    assert_eq!(outcome, "READ Cancelled 2");
}

fn child_scenario(mode: &str) {
    let root: std::path::PathBuf = support::child_var("YMP_TEST_ROOT").into();
    let sid = session_id("restart-session");
    match mode {
        "open" => {
            let dispatcher = support::open_dispatcher(&root);
            let view = dispatcher
                .open(sid.clone(), sample_task("restart"))
                .expect("child session opens");
            support::write_outcome(&format!("OPENED {}", view.revision().value()));
        }
        "read-cancelled" => {
            let dispatcher = support::open_dispatcher(&root);
            let view = dispatcher.read(&sid).expect("child session reads");
            support::write_outcome(&format!(
                "READ {:?} {}",
                view.status(),
                view.revision().value()
            ));
        }
        other => panic!("unknown child scenario '{other}'"),
    }
}
