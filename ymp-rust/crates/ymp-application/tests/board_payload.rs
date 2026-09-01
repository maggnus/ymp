//! Acceptance: application publication makes exact inert bytes durable before their board fact,
//! and the typed operator projection resolves only complete, verified payload objects.

use std::fs;
use std::path::{Path, PathBuf};

use tempfile::tempdir;
use ymp_application::{Application, ApplicationError, BOARD_SECTION};
use ymp_board::store::FACT_RECORD;
use ymp_board::{Audience, BoardCommand, MessageKind, Payload, Publish, Relation};
use ymp_domain::Budget;
use ymp_storage::ObjectStoreError;

const PAYLOAD: &[u8] = b"exact inert collaboration payload";

fn publication(message_id: &str, payload: Payload) -> Publish {
    Publish {
        message_id: message_id.to_owned(),
        author: "participant-root".to_owned(),
        audience: Audience::ProjectDiscovery,
        kind: MessageKind::Observation,
        payload,
        salience_ms: 5_000,
        references: Vec::new(),
        relation: Relation::Standalone,
        claimed_decision_basis: Vec::new(),
    }
}

fn positions(application: &Application) -> (usize, u64) {
    let observation = application.board_observation();
    (observation.audit_messages, observation.recorded_facts)
}

fn object_path(root: &Path, digest: &str) -> PathBuf {
    root.join("objects/sha256")
        .join(&digest[..2])
        .join(&digest[2..])
}

#[test]
fn exact_payload_survives_reopen_and_projection_values_are_isolated() {
    let temporary = tempdir().expect("temporary directory");
    let root = temporary.path();
    let publish = publication("message-1", Payload::of(PAYLOAD));
    let digest = publish.payload.digest().to_owned();
    let mut application =
        Application::create(root, "run-1", Budget::new(1, 1)).expect("create run");
    application
        .publish_board(&publish, PAYLOAD)
        .expect("publish exact payload");
    drop(application);

    let application = Application::open(root).expect("reopen run");
    let stable_positions = positions(&application);
    let mut projection = application
        .operator_board_projection()
        .expect("resolve persisted payload");
    assert_eq!(projection.messages.len(), 1);
    assert_eq!(projection.messages[0].message.payload_digest, digest);
    assert_eq!(projection.messages[0].payload, PAYLOAD);

    projection.messages[0].message.author.clear();
    projection.messages[0].payload.fill(0);
    let repeated = application
        .operator_board_projection()
        .expect("resolve independent owned copies");
    assert_eq!(repeated.messages[0].message.author, "participant-root");
    assert_eq!(repeated.messages[0].payload, PAYLOAD);
    assert_eq!(positions(&application), stable_positions);
    drop(application);

    let reopened = Application::open(root).expect("reopen after projection mutation");
    let persisted = reopened
        .operator_board_projection()
        .expect("resolve persisted copies again");
    assert_eq!(persisted.messages[0].message.author, "participant-root");
    assert_eq!(persisted.messages[0].payload, PAYLOAD);
    assert_eq!(positions(&reopened), stable_positions);
}

#[test]
fn publication_mismatches_and_metadata_only_bypass_leave_board_unchanged() {
    let temporary = tempdir().expect("temporary directory");
    let mut application =
        Application::create(temporary.path(), "run-1", Budget::new(1, 1)).expect("create run");
    application
        .publish_board(&publication("existing", Payload::of(PAYLOAD)), PAYLOAD)
        .expect("publish the complete projection that refusals must preserve");
    let stable_positions = positions(&application);

    let wrong_length = publication(
        "wrong-length",
        Payload::stated(ymp_board::digest_bytes(PAYLOAD), PAYLOAD.len() as u64 + 1),
    );
    assert!(matches!(
        application.publish_board(&wrong_length, PAYLOAD),
        Err(ApplicationError::BoardPayloadLengthMismatch { .. })
    ));
    assert_eq!(positions(&application), stable_positions);

    let wrong_digest = publication(
        "wrong-digest",
        Payload::stated("a".repeat(64), PAYLOAD.len() as u64),
    );
    assert!(matches!(
        application.publish_board(&wrong_digest, PAYLOAD),
        Err(ApplicationError::BoardPayloadDigestMismatch { .. })
    ));
    assert_eq!(positions(&application), stable_positions);

    let metadata_only = publication("metadata-only", Payload::of(PAYLOAD));
    assert!(matches!(
        application.record_board(&BoardCommand::Publish(metadata_only)),
        Err(ApplicationError::BoardPayloadRequired)
    ));
    assert_eq!(positions(&application), stable_positions);

    let projection = application
        .operator_board_projection()
        .expect("refused mismatches leave the prior projection whole");
    assert_eq!(projection.messages.len(), 1);
    assert_eq!(projection.messages[0].message.message_id, "existing");
    assert_eq!(projection.messages[0].payload, PAYLOAD);
}

#[test]
fn board_record_refusal_leaves_only_a_verified_unreferenced_object() {
    let temporary = tempdir().expect("temporary directory");
    let root = temporary.path();
    let mut application =
        Application::create(root, "run-1", Budget::new(1, 1)).expect("create run");
    let publish = publication("message-1", Payload::of(PAYLOAD));
    let object = object_path(root, publish.payload.digest());
    let stable_positions = positions(&application);

    let fact_record = root.join(BOARD_SECTION).join(FACT_RECORD);
    fs::remove_file(&fact_record).expect("remove fact record");
    fs::create_dir(&fact_record).expect("replace fact record with unwritable directory");

    assert!(matches!(
        application.publish_board(&publish, PAYLOAD),
        Err(ApplicationError::BoardSectionUnusable(_))
    ));
    assert_eq!(fs::read(object).expect("durable object remains"), PAYLOAD);
    assert_eq!(positions(&application), stable_positions);
}

#[test]
fn missing_or_corrupt_payload_object_refuses_projection_without_moving_board() {
    for corruption in ["missing", "corrupt"] {
        let temporary = tempdir().expect("temporary directory");
        let root = temporary.path();
        let second_payload = b"second exact inert payload";
        let publish = publication("message-2", Payload::of(second_payload));
        let digest = publish.payload.digest().to_owned();
        let mut application =
            Application::create(root, "run-1", Budget::new(1, 1)).expect("create run");
        application
            .publish_board(&publication("message-1", Payload::of(PAYLOAD)), PAYLOAD)
            .expect("publish valid first payload");
        application
            .publish_board(&publish, second_payload)
            .expect("publish exact second payload");
        let stable_positions = positions(&application);
        let object = object_path(root, &digest);
        if corruption == "missing" {
            fs::remove_file(object).expect("remove payload object");
        } else {
            fs::write(object, vec![b'x'; second_payload.len()]).expect("corrupt payload object");
        }

        let result = application.operator_board_projection();
        assert!(
            matches!(
                (&result, corruption),
                (
                    Err(ApplicationError::ObjectStore(ObjectStoreError::Missing(actual))),
                    "missing"
                ) if actual == &digest
            ) || matches!(
                (&result, corruption),
                (
                    Err(ApplicationError::ObjectStore(ObjectStoreError::DigestMismatch(actual))),
                    "corrupt"
                ) if actual == &digest
            ),
            "{corruption} object did not fail closed: {result:?}"
        );
        assert_eq!(positions(&application), stable_positions);
    }
}
