//! Restart replay, crash recovery and batch atomicity (DEV-0005 acceptance
//! criteria 1, 2, 4, 8 and 9), exercised through the kernel `Dispatcher`.

#![forbid(unsafe_code)]

mod support;

use std::fs;

use support::{TempRoot, cancelled_event, open_dispatcher, opened_event, sample_task, session_id};
use ymp_kernel::{DispatchError, Dispatcher, Journal, JournalError, Revision, SessionStatus};
use ymp_storage::{FileJournal, InjectedFault};

const CHILD_TEST: &str = "restart_replay_uses_real_child_processes";

#[test]
fn restart_replay_uses_real_child_processes() {
    // Parent side: two separate child processes perform the writes; this
    // process and a third child perform the reads.
    if let Some(mode) = support::child_mode() {
        child_scenario(&mode);
        return;
    }

    let root = TempRoot::new("restart");
    let sid = session_id("restart-session");
    let task = sample_task("restart");

    // Process 1: open the session and drop the process.
    let outcome = support::run_child(
        CHILD_TEST,
        "open",
        &[("YMP_TEST_ROOT", root.path().to_string_lossy().into())],
    );
    assert_eq!(outcome, "OPENED 1");

    // Process 2 (this one): reopen from the same root and read back the exact
    // same identity, task content, status and revision.
    let dispatcher = open_dispatcher(root.path());
    let view = dispatcher.read(&sid).expect("session reads after restart");
    assert_eq!(view.session_id(), &sid);
    assert_eq!(
        view.task(),
        &task,
        "the exact task content survives restart"
    );
    assert_eq!(view.status(), SessionStatus::Open);
    assert_eq!(view.revision(), Revision::new(1));
    let entries = FileJournal::open(root.path())
        .and_then(|journal| journal.read(&sid))
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

    // Cancel in this process, then verify the cancellation persists for a
    // third process reading the same root.
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
            let dispatcher = open_dispatcher(&root);
            let view = dispatcher
                .open(sid.clone(), sample_task("restart"))
                .expect("child session opens");
            support::write_outcome(&format!("OPENED {}", view.revision().value()));
        }
        "read-cancelled" => {
            let dispatcher = open_dispatcher(&root);
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

#[test]
fn crash_before_commit_leaves_stream_at_previous_revision() {
    let root = TempRoot::new("crash-before-commit");
    let sid = session_id("crash-before");
    let task = sample_task("crash");

    let dispatcher = open_dispatcher(root.path());
    let opened = dispatcher
        .open(sid.clone(), task.clone())
        .expect("session opens");
    assert_eq!(opened.revision(), Revision::new(1));

    // A fully framed batch is appended and made durable in the log, but the
    // commit descriptor is never renamed.
    let faulting = FileJournal::open_with_injected_fault(
        root.path(),
        InjectedFault::AdapterFailureBeforeCommit,
    )
    .expect("faulting journal opens");
    assert!(matches!(
        Dispatcher::new(faulting)
            .cancel(&sid, Revision::new(1))
            .unwrap_err(),
        DispatchError::Journal(JournalError::AdapterFailure { .. })
    ));

    // The visible stream still rests at the previous revision and replay
    // equals the previous view.
    let still_open = dispatcher.read(&sid).expect("session still reads");
    assert_eq!(still_open.revision(), Revision::new(1));
    assert_eq!(still_open.status(), SessionStatus::Open);
    assert_eq!(still_open.task(), &task);

    // The log holds the uncommitted suffix until the next open truncates it.
    let log_bytes = fs::read(support::log_path(root.path(), sid.as_str())).expect("log reads");
    let commit_bytes =
        fs::read(support::commit_path(root.path(), sid.as_str())).expect("descriptor reads");
    let committed_len = u64::from_le_bytes(commit_bytes[..8].try_into().expect("u64 slice"));
    assert!(
        log_bytes.len() as u64 > committed_len,
        "the fully framed batch lies beyond the durable prefix"
    );

    // Reopen: recovery discards the unacknowledged batch, and the descriptor
    // and log agree again. The committed prefix bytes are untouched.
    drop(dispatcher);
    let reopened = open_dispatcher(root.path());
    let view = reopened.read(&sid).expect("session reads after reopen");
    assert_eq!(view.revision(), Revision::new(1));
    assert_eq!(view.status(), SessionStatus::Open);
    let after = fs::read(support::log_path(root.path(), sid.as_str())).expect("log reads");
    assert_eq!(after.len() as u64, committed_len);
    assert_eq!(
        &after[..committed_len as usize],
        &log_bytes[..committed_len as usize]
    );
}

#[test]
fn interrupted_first_creation_recovers_to_initial_stream() {
    let root = TempRoot::new("first-creation");
    let sid = session_id("interrupted");
    let survivor = session_id("committed-neighbor");

    // A committed neighbor stream must survive recovery untouched.
    {
        let dispatcher = open_dispatcher(root.path());
        dispatcher
            .open(survivor.clone(), sample_task("neighbor"))
            .expect("neighbor opens");
    }
    let neighbor_log =
        fs::read(support::log_path(root.path(), survivor.as_str())).expect("neighbor log reads");

    // Interrupted first creation: a partially written first header, no
    // published descriptor, and only a temporary artifact.
    let sessions = root.path().join("sessions");
    fs::create_dir_all(&sessions).expect("sessions directory exists");
    let stem = support::hex_stem(sid.as_str());
    fs::write(sessions.join(format!("{stem}.log")), b"YMP").expect("partial header writes");
    fs::write(
        sessions.join(format!("{stem}.commit.tmp-0123456789abcdef")),
        [0u8; 12],
    )
    .expect("temporary artifact writes");

    // Reopen runs cleanup under the root lock: the stream rests at the
    // initial empty stream and the temporary artifact is gone.
    let dispatcher = open_dispatcher(root.path());
    assert_eq!(
        dispatcher.read(&sid).unwrap_err(),
        DispatchError::SessionNotFound {
            session_id: sid.clone()
        }
    );
    assert!(
        !sessions
            .join(format!("{stem}.commit.tmp-0123456789abcdef"))
            .exists()
    );

    // No committed prefix was deleted: the neighbor is intact.
    let neighbor_view = dispatcher
        .read(&survivor)
        .expect("neighbor survived recovery");
    assert_eq!(neighbor_view.revision(), Revision::new(1));
    assert_eq!(
        fs::read(support::log_path(root.path(), survivor.as_str())).expect("neighbor log reads"),
        neighbor_log
    );

    // The interrupted stream is usable from the initial revision.
    let opened = dispatcher
        .open(sid.clone(), sample_task("after-interruption"))
        .expect("session opens after recovered first creation");
    assert_eq!(opened.revision(), Revision::new(1));
}

#[test]
fn torn_final_write_beyond_prefix_is_discarded() {
    let root = TempRoot::new("torn");
    let sid = session_id("torn");
    let task = sample_task("torn");

    let dispatcher = open_dispatcher(root.path());
    dispatcher.open(sid.clone(), task).expect("session opens");
    let before = dispatcher.read(&sid).expect("view reads");

    // A physically truncated final frame beyond the durable prefix: a torn
    // final write.
    let log_path = support::log_path(root.path(), sid.as_str());
    let committed = fs::read(&log_path).expect("log reads");
    let mut torn = committed.clone();
    torn.extend_from_slice(&[0x01, 0x40, 0x00, 0x00]);
    fs::write(&log_path, torn).expect("torn tail writes");

    // Nothing at or below the prefix is lost and the suffix is invisible.
    let still = dispatcher.read(&sid).expect("torn suffix is invisible");
    assert_eq!(still, before);

    // Reopen discards the torn tail without error.
    drop(dispatcher);
    let reopened = open_dispatcher(root.path());
    let after = reopened.read(&sid).expect("session reads after reopen");
    assert_eq!(after, before);
    assert_eq!(
        fs::read(&log_path).expect("log reads"),
        committed,
        "the torn suffix was truncated at open"
    );
}

#[test]
fn typed_failures_leave_history_and_prefix_unchanged() {
    let root = TempRoot::new("typed-failures");
    let sid = session_id("typed");
    let task = sample_task("typed");

    let dispatcher = open_dispatcher(root.path());
    dispatcher.open(sid.clone(), task).expect("session opens");
    let history_before = {
        let journal = FileJournal::open(root.path()).expect("journal opens");
        journal.read(&sid).expect("history reads")
    };
    let snapshot_before = support::snapshot_tree(root.path());

    // EmptyBatch is rejected without touching the filesystem.
    let journal = FileJournal::open(root.path()).expect("journal opens");
    assert_eq!(
        journal.append(&sid, Revision::new(1), vec![]).unwrap_err(),
        JournalError::EmptyBatch
    );
    assert_eq!(support::snapshot_tree(root.path()), snapshot_before);

    // StaleRevision reports the durable revision and appends nothing.
    assert_eq!(
        journal
            .append(&sid, Revision::new(0), vec![cancelled_event(&sid)])
            .unwrap_err(),
        JournalError::StaleRevision {
            expected: Revision::new(0),
            actual: Revision::new(1)
        }
    );
    assert_eq!(support::snapshot_tree(root.path()), snapshot_before);

    // A stale append against an unknown session creates nothing either.
    let absent = session_id("absent-target");
    assert_eq!(
        journal
            .append(&absent, Revision::new(3), vec![cancelled_event(&absent)])
            .unwrap_err(),
        JournalError::StaleRevision {
            expected: Revision::new(3),
            actual: Revision::INITIAL
        }
    );
    assert!(!support::log_path(root.path(), absent.as_str()).exists());
    assert!(!support::commit_path(root.path(), absent.as_str()).exists());

    // RevisionOverflow is detected before writing; it is unreachable through
    // valid durable state (the decoder enforces contiguous revisions from one
    // and the 2^32-record limit), so the assignment helper is covered by the
    // crate's unit tests (`stream::tests::assign_revisions_detects_overflow`).

    // An injected AdapterFailure before the commit point leaves the durable
    // prefix unchanged and the partial suffix invisible to reads.
    let faulting = FileJournal::open_with_injected_fault(
        root.path(),
        InjectedFault::AdapterFailureBeforeCommit,
    )
    .expect("faulting journal opens");
    assert!(matches!(
        faulting
            .append(&sid, Revision::new(1), vec![cancelled_event(&sid)])
            .unwrap_err(),
        JournalError::AdapterFailure { .. }
    ));
    assert_eq!(
        journal.read(&sid).expect("history reads"),
        history_before,
        "no partial history is observable"
    );

    // Reopen: the uncommitted suffix is removed and the prior history and
    // durable prefix are identical.
    drop(dispatcher);
    drop(journal);
    drop(faulting);
    let reopened = open_dispatcher(root.path());
    let view = reopened.read(&sid).expect("session reads");
    assert_eq!(view.revision(), Revision::new(1));
    assert_eq!(
        FileJournal::open(root.path())
            .and_then(|handle| handle.read(&sid))
            .expect("history reads"),
        history_before
    );
    let log_bytes = fs::read(support::log_path(root.path(), sid.as_str())).expect("log reads");
    let descriptor_bytes =
        fs::read(support::commit_path(root.path(), sid.as_str())).expect("descriptor reads");
    let committed_len = u64::from_le_bytes(descriptor_bytes[..8].try_into().expect("u64 slice"));
    assert_eq!(committed_len as usize, log_bytes.len());
}

#[test]
fn multi_event_batch_appears_whole_or_not_at_all() {
    let root = TempRoot::new("batch");
    let sid = session_id("batch");

    // Success: one journal-level batch of two events appears as contiguous
    // ordered revisions through the kernel consumer.
    {
        let dispatcher = open_dispatcher(root.path());
        let journal = FileJournal::open(root.path()).expect("second handle opens");
        let attempted = journal
            .append(
                &sid,
                Revision::INITIAL,
                vec![
                    opened_event(&sid, &sample_task("batch")),
                    cancelled_event(&sid),
                ],
            )
            .expect("batch commits");
        assert_eq!(attempted, Revision::new(2));
        let view = dispatcher.read(&sid).expect("session reads");
        assert_eq!(view.revision(), Revision::new(2));
        assert_eq!(view.status(), SessionStatus::Cancelled);
    }

    // Failure: the same batch with an injected failure before the commit
    // point exposes no strict prefix of itself.
    let other = session_id("batch-failed");
    let faulting = FileJournal::open_with_injected_fault(
        root.path(),
        InjectedFault::AdapterFailureBeforeCommit,
    )
    .expect("faulting journal opens");
    assert!(matches!(
        faulting
            .append(
                &other,
                Revision::INITIAL,
                vec![
                    opened_event(&other, &sample_task("batch-failed")),
                    cancelled_event(&other),
                ],
            )
            .unwrap_err(),
        JournalError::AdapterFailure { .. }
    ));
    let dispatcher = open_dispatcher(root.path());
    assert_eq!(
        dispatcher.read(&other).unwrap_err(),
        DispatchError::SessionNotFound {
            session_id: other.clone()
        },
        "no strict prefix of the batch is observable"
    );
    // And nothing appears after reopen either.
    drop(dispatcher);
    drop(faulting);
    let reopened = open_dispatcher(root.path());
    assert_eq!(
        reopened.read(&other).unwrap_err(),
        DispatchError::SessionNotFound {
            session_id: other.clone()
        }
    );
}

#[test]
fn child_process_helper_actually_spawns_a_process() {
    // Guard for the support machinery: the child must really be a distinct
    // process (different PID than this one).
    if let Some(mode) = support::child_mode() {
        assert_eq!(mode, "pid");
        support::write_outcome(&format!("{}", std::process::id()));
        return;
    }
    let pid = support::run_child("child_process_helper_actually_spawns_a_process", "pid", &[]);
    let child_pid: u32 = pid.parse().expect("child reports its PID");
    assert_ne!(child_pid, std::process::id());
}
