use tempfile::tempdir;
use ymp_domain::{Budget, EventEnvelope, EventKind};
use ymp_storage::{Journal, JournalError, JournalLimits, ObjectStore, ObjectStoreError};

#[test]
fn object_paths_require_canonical_lowercase_sha256() {
    let temporary = tempdir().expect("temporary object root");
    let store = ObjectStore::open(temporary.path()).expect("open object store");
    assert!(matches!(
        store.path_for(&"A".repeat(64)),
        Err(ObjectStoreError::InvalidDigest(_))
    ));
    assert!(matches!(
        store.path_for(&"g".repeat(64)),
        Err(ObjectStoreError::InvalidDigest(_))
    ));
}

#[test]
fn altered_object_bytes_are_rejected_on_read() {
    let temporary = tempdir().expect("temporary object root");
    let store = ObjectStore::open(temporary.path()).expect("open object store");
    let digest = store.put(b"original").expect("store object");
    let path = store.path_for(&digest).expect("object path");
    std::fs::write(path, b"altered").expect("alter object");
    assert!(matches!(
        store.read(&digest),
        Err(ObjectStoreError::DigestMismatch(_))
    ));
}

#[test]
fn bounded_journal_preserves_space_for_one_terminal_event() {
    let temporary = tempdir().expect("temporary journal root");
    let path = temporary.path().join("events.jsonl");
    let limits = JournalLimits {
        max_event_bytes: 1024,
        max_journal_bytes: 2048,
        terminal_reserve_bytes: 1024,
    };
    let (mut journal, events) =
        Journal::open_with_limits(&path, limits).expect("open bounded journal");
    assert!(events.is_empty());
    let mut previous = None;
    let mut sequence = 1;

    loop {
        let event = EventEnvelope::new(
            "run-1",
            sequence,
            format!("command-{sequence}"),
            "0".repeat(64),
            previous.clone(),
            if sequence == 1 {
                EventKind::RunStarted {
                    budget: Budget::new(8, 4),
                }
            } else {
                EventKind::AttemptStarted {
                    attempt_id: format!("attempt-{sequence}"),
                }
            },
        )
        .expect("build event");
        match journal.append(&event) {
            Ok(()) => {
                previous = Some(event.digest);
                sequence += 1;
            }
            Err(JournalError::CapacityExhausted { .. }) => break,
            Err(error) => panic!("unexpected append error: {error}"),
        }
    }

    let terminal = EventEnvelope::new(
        "run-1",
        sequence,
        "ymp.infrastructure.journal-capacity",
        "f".repeat(64),
        previous,
        EventKind::RunFailed {
            reason: "journal capacity exhausted".to_owned(),
        },
    )
    .expect("build terminal event");
    journal
        .append_terminal(&terminal)
        .expect("terminal reserve accepts failure event");
    let recovered = Journal::read_all_with_limits(&path, limits).expect("recover bounded journal");
    assert!(matches!(
        recovered.last().expect("terminal event").event,
        EventKind::RunFailed { .. }
    ));
    assert!(path.metadata().expect("journal metadata").len() <= limits.max_journal_bytes);
}

#[test]
fn oversized_records_and_journals_are_rejected_before_parsing() {
    let temporary = tempdir().expect("temporary journal root");
    let path = temporary.path().join("events.jsonl");
    let limits = JournalLimits {
        max_event_bytes: 512,
        max_journal_bytes: 2048,
        terminal_reserve_bytes: 512,
    };
    let (mut journal, _) = Journal::open_with_limits(&path, limits).expect("open journal");
    let oversized = EventEnvelope::new(
        "run-1",
        1,
        "command-1",
        "0".repeat(64),
        None,
        EventKind::RunFailed {
            reason: "x".repeat(1024),
        },
    )
    .expect("build oversized event");
    assert!(matches!(
        journal.append(&oversized),
        Err(JournalError::EventTooLarge { .. })
    ));
    drop(journal);

    std::fs::write(&path, vec![b'x'; 2049]).expect("write oversized journal");
    assert!(matches!(
        Journal::read_all_with_limits(&path, limits),
        Err(JournalError::JournalTooLarge { .. })
    ));
}
