mod support;
use std::sync::Arc;
use ymp_domain::{Id, journal::encode};
use ymp_kernel::journal::{ContentStore, Journal, ParameterSchemas};
use ymp_runtime::memory_journal::MemoryJournal;
use ymp_storage::journal::SqliteJournal;

#[test]
fn reopening_matches_memory_and_retains_original_parameters_and_exact_content() {
    let directory = support::Directory::new();
    let durable =
        Arc::new(SqliteJournal::open(directory.database(), ParameterSchemas::default()).unwrap());
    let memory = Arc::new(MemoryJournal::new());
    support::populate(durable.clone());
    support::populate(memory.clone());
    let bytes = [0, 255, 10, 13, 42, 0];
    let digest = durable.content_store().put(&bytes).unwrap();
    drop(durable);
    let durable = SqliteJournal::open(directory.database(), ParameterSchemas::default()).unwrap();
    for name in ["methods", "intake"] {
        let session = Id::new(name).unwrap();
        assert_eq!(
            durable.read(&session).unwrap(),
            memory.read(&session).unwrap()
        );
        assert_eq!(
            durable.view(&session, None).unwrap(),
            memory.view(&session, None).unwrap()
        );
    }
    assert_eq!(
        durable.content_store().get(&digest, bytes.len()).unwrap(),
        bytes
    );
    assert!(
        durable
            .content_store()
            .get(&digest, bytes.len() - 1)
            .is_err()
    );
    for kind in ["SoloWithVerifier", "Solo"] {
        let selected = support::policy(kind);
        assert_eq!(
            durable
                .content_store()
                .get(&selected.policy.params, 4096)
                .unwrap(),
            encode(&selected.parameters).unwrap()
        );
    }
    let view = durable.view(&Id::new("methods").unwrap(), Some(2)).unwrap();
    assert_eq!(
        view.policies()["MethodRouter"],
        support::policy("SoloWithVerifier")
    );
}

#[test]
fn an_existing_handle_never_recreates_a_deleted_database() {
    let directory = support::Directory::new();
    let journal = SqliteJournal::open(directory.database(), ParameterSchemas::default()).unwrap();
    std::fs::remove_file(directory.database()).unwrap();
    assert!(journal.read(&Id::new("missing").unwrap()).is_err());
    assert!(!directory.database().exists());
}
