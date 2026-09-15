//! Corruption handling (DEV-0005 acceptance criterion 5): damage inside the
//! committed prefix, framing rejections and malformed payloads all fail with
//! the typed `Corruption` variant, never an empty, shorter or valid history,
//! while other session streams stay usable. Also covers read-side
//! `AdapterFailure` for ordinary I/O failures.

#![forbid(unsafe_code)]

mod support;

use std::fs;
use std::path::Path;

use support::{
    TempRoot, committed_descriptor, completion_frame, descriptor, framed_stream, header,
    open_dispatcher, record_frame, sample_task, session_id, write_store,
};
use ymp_kernel::{DispatchError, Journal, JournalError};
use ymp_storage::FileJournal;

fn opened_payload(session: &str, task_tag: &str) -> Vec<u8> {
    format!(
        r#"{{"type":"session_opened","session_id":"{session}","task":{{"id":"t-{task_tag}","goal":{{"request":"g"}},"acceptance_contract":{{"criteria":[{{"id":"c1","description":"d"}}]}},"constraints":{{"conditions":[]}}}}}}"#
    )
    .into_bytes()
}

fn cancelled_payload(session: &str) -> Vec<u8> {
    format!(r#"{{"type":"session_cancelled","session_id":"{session}"}}"#).into_bytes()
}

/// A healthy two-record store built entirely by the test's own builders.
fn healthy_store(root: &Path, session: &str) {
    let log = framed_stream(&[vec![
        opened_payload(session, "fixture"),
        cancelled_payload(session),
    ]]);
    write_store(root, session, &log, Some(&committed_descriptor(&log)));
}

#[test]
fn damaged_committed_record_fails_as_typed_corruption() {
    let root = TempRoot::new("corrupt-middle");
    let sid = session_id("damaged");
    let other = session_id("healthy-neighbor");

    let dispatcher = open_dispatcher(root.path());
    dispatcher
        .open(other.clone(), sample_task("neighbor"))
        .expect("neighbor opens");
    dispatcher
        .open(sid.clone(), sample_task("damaged"))
        .expect("damaged session opens");
    dispatcher
        .cancel(&sid, ymp_kernel::Revision::new(1))
        .expect("cancel commits");

    // Damage one byte inside the first committed record's payload.
    let log_path = support::log_path(root.path(), sid.as_str());
    let mut log = fs::read(&log_path).expect("log reads");
    let payload_start = 6 + 1 + 4 + 8; // header + tag + length + revision
    log[payload_start + 2] ^= 0x40;
    fs::write(&log_path, log).expect("damaged log writes");

    // Reads of that stream fail with the typed Corruption variant; the stream
    // is never reported as empty, shorter or valid.
    let journal = FileJournal::open(root.path()).expect("journal opens");
    assert!(matches!(
        journal.read(&sid),
        Err(JournalError::Corruption { .. })
    ));
    assert!(matches!(
        dispatcher.read(&sid),
        Err(DispatchError::Journal(JournalError::Corruption { .. }))
    ));

    // Appends for that stream fail the same way, without repair.
    assert!(matches!(
        journal.append(
            &sid,
            ymp_kernel::Revision::new(2),
            vec![support::cancelled_event(&sid)]
        ),
        Err(JournalError::Corruption { .. })
    ));

    // Other session streams in the same root remain usable.
    let neighbor = dispatcher
        .read(&other)
        .expect("other sessions remain readable");
    assert_eq!(neighbor.revision(), ymp_kernel::Revision::new(1));
}

#[test]
fn damaged_committed_record_is_not_repaired_by_reopen() {
    let root = TempRoot::new("corrupt-no-repair");
    let sid = session_id("damaged-reopen");
    healthy_store(root.path(), sid.as_str());

    let log_path = support::log_path(root.path(), sid.as_str());
    let mut log = fs::read(&log_path).expect("log reads");
    let payload_start = 6 + 13;
    log[payload_start + 2] ^= 0x40;
    let damaged = log.clone();
    fs::write(&log_path, &damaged).expect("damaged log writes");

    // Reopening the journal does not repair or truncate below the prefix.
    let journal = FileJournal::open(root.path()).expect("journal opens");
    assert!(matches!(
        journal.read(&sid),
        Err(JournalError::Corruption { .. })
    ));
    assert_eq!(
        fs::read(&log_path).expect("log reads"),
        damaged,
        "open never truncates below the durable prefix"
    );
}

#[test]
fn descriptor_inconsistencies_fail_as_corruption() {
    type Damage = Box<dyn Fn(&Path, &Path)>;

    struct Case {
        name: &'static str,
        damage: Damage,
    }

    let cases = vec![
        Case {
            name: "descriptor without its log",
            damage: Box::new(|log, _| {
                fs::remove_file(log).expect("log removes");
            }),
        },
        Case {
            name: "descriptor naming missing bytes",
            damage: Box::new(|_, commit| {
                let bytes = fs::read(commit).expect("descriptor reads");
                fs::write(commit, descriptor(1 << 40, support::crc32(&bytes))).expect("writes");
            }),
        },
        Case {
            name: "digest mismatch",
            damage: Box::new(|_, commit| {
                let mut bytes = fs::read(commit).expect("descriptor reads");
                bytes[11] ^= 0xFF;
                fs::write(commit, bytes).expect("writes");
            }),
        },
        Case {
            name: "short descriptor",
            damage: Box::new(|_, commit| {
                let bytes = fs::read(commit).expect("descriptor reads");
                fs::write(commit, &bytes[..6]).expect("writes");
            }),
        },
    ];

    for case in cases {
        let root = TempRoot::new("descriptor-damage");
        let sid = session_id("descriptor");
        healthy_store(root.path(), sid.as_str());
        (case.damage)(
            &support::log_path(root.path(), sid.as_str()),
            &support::commit_path(root.path(), sid.as_str()),
        );
        let journal = FileJournal::open(root.path()).expect("journal opens");
        assert!(
            matches!(journal.read(&sid), Err(JournalError::Corruption { .. })),
            "case '{}' fails with typed Corruption",
            case.name
        );
    }
}

#[test]
fn framing_rejections_fail_as_corruption() {
    struct Case {
        name: &'static str,
        log: Vec<u8>,
    }

    let payload = opened_payload("framing", "fixture");
    let frame = record_frame(1, &payload);
    let healthy_completion = completion_frame(&frame, 1);

    let cases = vec![
        Case {
            name: "unknown record tag",
            log: {
                let mut bytes = header();
                bytes.push(0x03);
                bytes.extend_from_slice(&frame[1..]);
                bytes.extend_from_slice(&healthy_completion);
                bytes
            },
        },
        Case {
            name: "record checksum mismatch",
            log: {
                let mut broken = frame.clone();
                let last = broken.len() - 1;
                broken[last] ^= 0xFF;
                let mut bytes = header();
                bytes.extend_from_slice(&broken);
                bytes.extend_from_slice(&healthy_completion);
                bytes
            },
        },
        Case {
            name: "completion checksum mismatch",
            log: {
                let mut bytes = header();
                bytes.extend_from_slice(&frame);
                bytes.extend_from_slice(&healthy_completion);
                let last = bytes.len() - 1;
                bytes[last] ^= 0xFF;
                bytes
            },
        },
        Case {
            name: "completion count mismatch",
            log: {
                let mut bytes = header();
                bytes.extend_from_slice(&frame);
                bytes.extend_from_slice(&completion_frame(&frame, 2));
                bytes
            },
        },
        Case {
            name: "completion with zero records",
            log: {
                let mut bytes = header();
                bytes.extend_from_slice(&completion_frame(&[], 0));
                bytes
            },
        },
        Case {
            name: "committed prefix ends inside a batch",
            log: {
                let mut bytes = header();
                bytes.extend_from_slice(&frame);
                bytes
            },
        },
        Case {
            name: "non-contiguous revisions",
            log: {
                let mut bytes = header();
                let skipped = record_frame(2, &payload);
                bytes.extend_from_slice(&skipped);
                bytes.extend_from_slice(&completion_frame(&skipped, 1));
                bytes
            },
        },
        Case {
            name: "declared payload length exceeds the limit",
            log: {
                let mut bytes = header();
                bytes.push(0x01);
                bytes.extend_from_slice(&(1u32 << 21).to_le_bytes());
                bytes.extend_from_slice(&1u64.to_le_bytes());
                bytes.extend_from_slice(&healthy_completion);
                bytes
            },
        },
        Case {
            name: "declared payload length crosses the boundary",
            log: {
                let mut bytes = header();
                bytes.push(0x01);
                bytes.extend_from_slice(&(payload.len() as u32 + 128).to_le_bytes());
                bytes.extend_from_slice(&1u64.to_le_bytes());
                bytes.extend_from_slice(&payload);
                bytes.extend_from_slice(&0u32.to_le_bytes());
                bytes.extend_from_slice(&healthy_completion);
                bytes
            },
        },
        Case {
            name: "header magic corruption",
            log: {
                let mut bytes = header();
                bytes.extend_from_slice(&frame);
                bytes.extend_from_slice(&healthy_completion);
                bytes[1] ^= 0xFF;
                bytes
            },
        },
    ];

    for case in cases {
        let root = TempRoot::new("framing");
        let sid = session_id("framing");
        write_store(
            root.path(),
            sid.as_str(),
            &case.log,
            Some(&committed_descriptor(&case.log)),
        );
        let journal = FileJournal::open(root.path()).expect("journal opens");
        assert!(
            matches!(journal.read(&sid), Err(JournalError::Corruption { .. })),
            "case '{}' fails with typed Corruption",
            case.name
        );
    }
}

#[test]
fn malformed_payloads_fail_as_corruption() {
    let cases: Vec<(&str, Vec<u8>)> = vec![
        (
            "unknown field",
            br#"{"type":"session_cancelled","session_id":"s","priority":"high"}"#.to_vec(),
        ),
        (
            "duplicate field",
            br#"{"type":"session_cancelled","session_id":"s","session_id":"s"}"#.to_vec(),
        ),
        (
            "unknown type",
            br#"{"type":"session_paused","session_id":"s"}"#.to_vec(),
        ),
        (
            "missing required field",
            br#"{"type":"session_cancelled"}"#.to_vec(),
        ),
        (
            "wrong value type",
            br#"{"type":"session_cancelled","session_id":42}"#.to_vec(),
        ),
    ];

    for (name, payload) in cases {
        let root = TempRoot::new("payload");
        let sid = session_id("payload");
        let log = framed_stream(&[vec![payload]]);
        write_store(
            root.path(),
            sid.as_str(),
            &log,
            Some(&committed_descriptor(&log)),
        );
        let journal = FileJournal::open(root.path()).expect("journal opens");
        assert!(
            matches!(journal.read(&sid), Err(JournalError::Corruption { .. })),
            "payload case '{}' fails with typed Corruption",
            name
        );
    }
}

#[cfg(unix)]
#[test]
fn ordinary_read_io_failure_is_adapter_failure() {
    use std::os::unix::fs::PermissionsExt;

    let root = TempRoot::new("read-io");
    let sid = session_id("read-io");
    let dispatcher = open_dispatcher(root.path());
    dispatcher
        .open(sid.clone(), sample_task("read-io"))
        .expect("session opens");

    let sessions = root.path().join("sessions");
    let mode = fs::metadata(&sessions)
        .expect("metadata reads")
        .permissions()
        .mode();
    fs::set_permissions(&sessions, fs::Permissions::from_mode(0o000))
        .expect("sessions directory becomes unreadable");

    let journal = FileJournal::open(root.path());
    match journal {
        Ok(journal) => {
            let result = journal.read(&sid);
            assert!(
                matches!(result, Err(JournalError::AdapterFailure { .. })),
                "an unreadable root fails as AdapterFailure, not Corruption"
            );
        }
        Err(error) => {
            // Open-time recovery could not scan the directory; also an
            // ordinary I/O failure.
            assert!(matches!(error, JournalError::AdapterFailure { .. }));
        }
    }

    fs::set_permissions(&sessions, fs::Permissions::from_mode(mode))
        .expect("sessions directory permissions restore");
}

#[test]
fn unwritable_lock_file_fails_as_adapter_failure() {
    let root = TempRoot::new("bad-lock");
    fs::create_dir_all(root.path().join("journal.lock")).expect("lock path becomes a directory");
    assert!(matches!(
        FileJournal::open(root.path()),
        Err(JournalError::AdapterFailure { .. })
    ));
}
