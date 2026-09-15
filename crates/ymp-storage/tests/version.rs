//! Version mismatch (DEV-0005 acceptance criterion 7): a stream header
//! carrying an unknown format version makes every operation on it fail with
//! `UnsupportedFormat`, naming the version, and leaves the file bytes
//! unchanged.

#![forbid(unsafe_code)]

mod support;

use std::fs;

use support::{TempRoot, sample_task, session_id};
use ymp_kernel::{DispatchError, Journal, JournalError, Revision};
use ymp_storage::FileJournal;

fn versioned_header(version: u16) -> Vec<u8> {
    let mut bytes = b"YMPJ".to_vec();
    bytes.extend_from_slice(&version.to_le_bytes());
    bytes
}

#[test]
fn unknown_version_fails_every_operation_without_mutation() {
    let root = TempRoot::new("version");
    let sid = session_id("future-version");
    let healthy = session_id("healthy-version");

    // A foreign stream with a recognized header of version 2, next to a
    // healthy version-1 stream.
    let sessions = root.path().join("sessions");
    fs::create_dir_all(&sessions).expect("sessions directory creates");
    let stem = support::hex_stem(sid.as_str());
    let future_log = versioned_header(2);
    fs::write(sessions.join(format!("{stem}.log")), &future_log).expect("future log writes");

    {
        let dispatcher = support::open_dispatcher(root.path());
        dispatcher
            .open(healthy.clone(), sample_task("healthy"))
            .expect("healthy session opens");
    }

    // Opening never migrates, rewrites or repairs format versions: the file
    // bytes are unchanged after open.
    let snapshot_before = support::snapshot_tree(root.path());
    let journal = FileJournal::open(root.path()).expect("journal opens");
    assert_eq!(
        support::snapshot_tree(root.path()),
        snapshot_before,
        "opening performs no repair or write"
    );

    // Every operation on the future stream fails with UnsupportedFormat,
    // naming the version.
    assert_eq!(
        journal.read(&sid).unwrap_err(),
        JournalError::UnsupportedFormat { version: 2 }
    );
    assert_eq!(
        journal
            .append(
                &sid,
                Revision::INITIAL,
                vec![support::opened_event(&sid, &sample_task("x"))]
            )
            .unwrap_err(),
        JournalError::UnsupportedFormat { version: 2 }
    );
    let dispatcher = support::open_dispatcher(root.path());
    assert_eq!(
        dispatcher.read(&sid).unwrap_err(),
        DispatchError::Journal(JournalError::UnsupportedFormat { version: 2 })
    );

    // And the file bytes are still unchanged.
    assert_eq!(
        fs::read(sessions.join(format!("{stem}.log"))).expect("future log reads"),
        future_log
    );
    assert_eq!(support::snapshot_tree(root.path()), snapshot_before);

    // The healthy neighbor is unaffected.
    let view = dispatcher
        .read(&healthy)
        .expect("healthy streams remain usable");
    assert_eq!(view.revision(), Revision::new(1));
}

#[test]
fn unknown_version_with_published_empty_descriptor_also_fails() {
    let root = TempRoot::new("version-descriptor");
    let sid = session_id("version-descriptor");

    // Even with an empty published prefix, a recognized header of an unknown
    // version is never recovered, repaired or read.
    let sessions = root.path().join("sessions");
    fs::create_dir_all(&sessions).expect("sessions directory creates");
    let stem = support::hex_stem(sid.as_str());
    let future_log = versioned_header(7);
    fs::write(sessions.join(format!("{stem}.log")), &future_log).expect("future log writes");
    fs::write(
        sessions.join(format!("{stem}.commit")),
        support::descriptor(0, support::crc32(&[])),
    )
    .expect("empty descriptor writes");

    // One open creates the root's lock file; the snapshot then covers every
    // byte the store holds, and the second open must not change any of them.
    drop(FileJournal::open(root.path()).expect("first journal opens"));
    let snapshot_before = support::snapshot_tree(root.path());
    let journal = FileJournal::open(root.path()).expect("journal opens");
    assert_eq!(
        journal.read(&sid).unwrap_err(),
        JournalError::UnsupportedFormat { version: 7 }
    );
    assert!(matches!(
        journal.append(
            &sid,
            Revision::INITIAL,
            vec![support::opened_event(&sid, &sample_task("x"))]
        ),
        Err(JournalError::UnsupportedFormat { .. })
    ));
    assert_eq!(support::snapshot_tree(root.path()), snapshot_before);
}
