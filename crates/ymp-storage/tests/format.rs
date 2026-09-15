//! Byte-level verification of the final version-1 format (DEV-0005): exact
//! header, record and completion framing with their CRC scopes, descriptor
//! shape, lowercase-hex stream naming, byte-stable output, and the adapter
//! input limits (100 UTF-8 bytes for the session ID, 2^20-byte payloads and
//! strings, 256-record batches) validated before any change is made.

#![forbid(unsafe_code)]

mod support;

use std::fs;

use support::{TempRoot, open_dispatcher, sample_task, session_id};
use ymp_domain::{Constraints, Goal};
use ymp_kernel::{Journal, JournalError, Revision, SessionEvent};
use ymp_storage::FileJournal;

#[test]
fn written_bytes_match_the_contract_exactly() {
    let root = TempRoot::new("format");
    let sid = session_id("format-check");
    let task = sample_task("format");

    let dispatcher = open_dispatcher(root.path());
    dispatcher
        .open(sid.clone(), task.clone())
        .expect("session opens");
    dispatcher.cancel(&sid, Revision::new(1)).expect("cancel");

    // Independently build the expected bytes from the contract text.
    let opened_payload = r#"{"type":"session_opened","session_id":"format-check","task":{"id":"  task id format  ","goal":{"request":"Goal \"quoted\" \\ backslash\nnewline format émoji 🚀"},"acceptance_contract":{"criteria":[{"id":"crit-format","description":"Criterion one for format with  spaces.\tTab."},{"id":"crit-2","description":"Второй критерий format ✓"}]},"constraints":{"conditions":["Do not access the network.","  Retain surrounding spaces.  "]}}}"#
        .as_bytes()
        .to_vec();
    let cancelled_payload = br#"{"type":"session_cancelled","session_id":"format-check"}"#.to_vec();
    let frame_one = support::record_frame(1, &opened_payload);
    let frame_two = support::record_frame(2, &cancelled_payload);
    let mut expected_log = support::header();
    expected_log.extend_from_slice(&frame_one);
    expected_log.extend_from_slice(&support::completion_frame(&frame_one, 1));
    expected_log.extend_from_slice(&frame_two);
    expected_log.extend_from_slice(&support::completion_frame(&frame_two, 1));

    let log_path = support::log_path(root.path(), sid.as_str());
    let actual_log = fs::read(&log_path).expect("log reads");
    assert_eq!(
        actual_log, expected_log,
        "the log bytes match the contract's version-1 framing exactly"
    );

    let commit_path = support::commit_path(root.path(), sid.as_str());
    let actual_commit = fs::read(&commit_path).expect("descriptor reads");
    assert_eq!(
        actual_commit,
        support::descriptor(expected_log.len() as u64, support::crc32(&expected_log)),
        "the descriptor names the committed length and its digest"
    );

    // The session round-trips through those exact bytes.
    let view = dispatcher.read(&sid).expect("session reads");
    assert_eq!(view.task(), &task);
}

#[test]
fn stream_files_are_named_with_lowercase_hex() {
    let root = TempRoot::new("naming");
    let sid = session_id("Naming Öl 🚀");
    let stem = support::hex_stem(sid.as_str());

    let dispatcher = open_dispatcher(root.path());
    dispatcher
        .open(sid, sample_task("naming"))
        .expect("session opens");

    let sessions = root.path().join("sessions");
    assert!(sessions.join(format!("{stem}.log")).exists());
    assert!(sessions.join(format!("{stem}.commit")).exists());
    assert!(
        stem.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    );

    // No temporary descriptors remain after a clean append.
    let leftovers: Vec<_> = fs::read_dir(&sessions)
        .expect("sessions lists")
        .map(|entry| {
            entry
                .expect("entry reads")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .filter(|name| name.contains(".tmp-"))
        .collect();
    assert!(
        leftovers.is_empty(),
        "temporary artifacts remain: {leftovers:?}"
    );
}

#[test]
fn output_is_byte_stable_across_roots() {
    let first = TempRoot::new("stable-one");
    let second = TempRoot::new("stable-two");
    let sid_a = session_id("stable");
    let sid_b = session_id("stable");

    for root in [&first, &second] {
        open_dispatcher(root.path())
            .open(sid_a.clone(), sample_task("stable"))
            .expect("session opens");
    }

    let a = fs::read(support::log_path(first.path(), sid_a.as_str())).expect("log reads");
    let b = fs::read(support::log_path(second.path(), sid_b.as_str())).expect("log reads");
    assert_eq!(a, b, "identical events encode to identical bytes");
}

#[test]
fn session_id_limit_is_one_hundred_bytes_enforced_before_any_change() {
    let root = TempRoot::new("id-limit");
    let exactly_hundred = session_id(&"x".repeat(100));
    let too_long = session_id(&"x".repeat(101));

    // Exactly 100 UTF-8 bytes is accepted.
    open_dispatcher(root.path())
        .open(exactly_hundred.clone(), sample_task("hundred"))
        .expect("a 100-byte session id is accepted");

    // 101 bytes is rejected before any change is made.
    let before = support::snapshot_tree(root.path());
    let journal = FileJournal::open(root.path()).expect("journal opens");
    assert!(matches!(
        journal
            .append(
                &too_long,
                Revision::INITIAL,
                vec![support::opened_event(&too_long, &sample_task("hundred"))]
            )
            .unwrap_err(),
        JournalError::AdapterFailure { .. }
    ));
    assert_eq!(
        support::snapshot_tree(root.path()),
        before,
        "the rejected identifier mutates nothing"
    );

    // The over-long identifier reads as an unknown session, not an error.
    let entries = journal.read(&too_long).expect("no stream exists");
    assert!(entries.is_empty());
}

#[test]
fn batch_and_payload_limits_are_enforced_before_any_change() {
    let root = TempRoot::new("limits");
    let sid = session_id("limits");

    {
        let dispatcher = open_dispatcher(root.path());
        dispatcher
            .open(sid.clone(), sample_task("limits"))
            .expect("session opens");
    }

    // A 257-record batch is rejected before any change.
    let oversized_batch: Vec<SessionEvent> = (0..257)
        .map(|_| SessionEvent::SessionCancelled {
            session_id: sid.clone(),
        })
        .collect();
    let before = support::snapshot_tree(root.path());
    let journal = FileJournal::open(root.path()).expect("journal opens");
    assert!(matches!(
        journal.append(&sid, Revision::new(1), oversized_batch),
        Err(JournalError::AdapterFailure { .. })
    ));
    assert_eq!(support::snapshot_tree(root.path()), before);

    // A single payload string above 2^20 bytes is rejected before any change.
    let oversized_string_task = sample_task_with_goal("limits", "g".repeat((1 << 20) + 1));
    assert!(matches!(
        journal.append(
            &sid,
            Revision::new(1),
            vec![SessionEvent::SessionOpened {
                session_id: sid.clone(),
                task: oversized_string_task,
            }]
        ),
        Err(JournalError::AdapterFailure { .. })
    ));

    // A payload assembled from many smaller strings above 2^20 bytes in total
    // is also rejected before any change.
    let mut conditions = Vec::new();
    for index in 0..8 {
        conditions.push(format!("condition-{index:04}-{}", "k".repeat(200_000)));
    }
    let wide = Constraints::new(conditions).expect("valid constraints");
    let wide_task = ymp_domain::Task::new(
        ymp_domain::TaskId::new("wide").expect("valid task ID"),
        Goal::new("wide goal").expect("valid goal"),
        sample_task("limits").acceptance_contract().clone(),
        wide,
    );
    assert!(matches!(
        journal.append(
            &sid,
            Revision::new(1),
            vec![SessionEvent::SessionOpened {
                session_id: sid.clone(),
                task: wide_task,
            }]
        ),
        Err(JournalError::AdapterFailure { .. })
    ));
    assert_eq!(support::snapshot_tree(root.path()), before);
}

/// A sample task with a custom goal request.
fn sample_task_with_goal(tag: &str, request: String) -> ymp_domain::Task {
    let base = sample_task(tag);
    ymp_domain::Task::new(
        ymp_domain::TaskId::new(format!("  task id {tag}  ")).expect("valid task ID"),
        Goal::new(request).expect("valid goal"),
        base.acceptance_contract().clone(),
        base.constraints().clone(),
    )
}

#[test]
fn temporary_descriptors_are_cleaned_at_next_open() {
    let root = TempRoot::new("temp-cleanup");
    let sid = session_id("temp-cleanup");
    let sessions = root.path().join("sessions");

    // A crash between writing and renaming can leave a temporary descriptor.
    {
        let dispatcher = open_dispatcher(root.path());
        dispatcher
            .open(sid.clone(), sample_task("temp"))
            .expect("session opens");
    }
    fs::write(
        sessions.join(format!(
            "{}.commit.tmp-0123456789abcdef",
            support::hex_stem(sid.as_str())
        )),
        [0u8; 12],
    )
    .expect("stale temporary descriptor writes");

    FileJournal::open(root.path()).expect("reopen runs cleanup");
    assert!(
        !sessions
            .join(format!(
                "{}.commit.tmp-0123456789abcdef",
                support::hex_stem(sid.as_str())
            ))
            .exists()
    );

    // The committed stream is untouched by the cleanup.
    let dispatcher = open_dispatcher(root.path());
    assert_eq!(
        dispatcher.read(&sid).expect("session reads").revision(),
        Revision::new(1)
    );
}
