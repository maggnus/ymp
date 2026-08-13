//! Acceptance: a store this binary cannot read is refused without a single write.
//!
//! The negative half is the byte comparison: `run.json` is read before the attempted open and
//! again afterwards, and the two must be identical. The behaviour this replaces did the
//! opposite — it rewrote the projection as an infrastructure failure and cleared the active
//! attempts of a run it could not read — so restoring that behaviour fails here.

use std::fs;

use ymp_application::{Application, ApplicationError};

/// A store written by a binary of the previous schema version: a running run, one active
/// attempt, and a projection that says so.
fn previous_version_store(root: &std::path::Path) -> (Vec<u8>, Vec<u8>) {
    fs::create_dir_all(root).expect("data root");
    let events = "\
{\"schema_version\":1,\"run_id\":\"older-run\",\"sequence\":1,\"command_id\":\"ymp.bootstrap\",\"command_digest\":\"00\",\"predecessor_digest\":null,\"event\":{\"type\":\"run_started\",\"budget\":{\"attempts_remaining\":2,\"verification_queries_remaining\":1}},\"digest\":\"11\"}
{\"schema_version\":1,\"run_id\":\"older-run\",\"sequence\":2,\"command_id\":\"attempt.start\",\"command_digest\":\"00\",\"predecessor_digest\":\"11\",\"event\":{\"type\":\"attempt_started\",\"attempt_id\":\"attempt-1\"},\"digest\":\"22\"}
";
    let projection = serde_json::to_vec_pretty(&serde_json::json!({
        "run_id": "older-run",
        "status": "running",
        "budget": {"attempts_remaining": 1, "verification_queries_remaining": 1},
        "active_attempts": ["attempt-1"],
        "candidate_digest": null,
        "last_sequence": 2,
        "last_event_digest": "22"
    }))
    .expect("projection bytes");
    fs::write(root.join("events.jsonl"), events).expect("journal");
    fs::write(root.join("run.json"), &projection).expect("projection");
    (events.as_bytes().to_vec(), projection)
}

#[test]
fn a_store_of_another_schema_version_is_refused_and_left_byte_for_byte_as_found() {
    let root = tempfile::tempdir().expect("temporary root");
    let data_root = root.path().join("data");
    let (events_before, projection_before) = previous_version_store(&data_root);

    let Err(error) = Application::open(&data_root) else {
        panic!("an unreadable store must be refused");
    };
    let reported = error.to_string();
    assert!(
        matches!(
            error,
            ApplicationError::IncompatibleStore {
                actual: 1,
                expected: 2
            }
        ),
        "{reported}"
    );
    assert!(reported.contains("schema version 1"), "{reported}");
    assert!(reported.contains("left the store unchanged"), "{reported}");

    assert_eq!(
        fs::read(data_root.join("run.json")).expect("projection after"),
        projection_before,
        "the refused store had its projection rewritten"
    );
    assert_eq!(
        fs::read(data_root.join("events.jsonl")).expect("journal after"),
        events_before,
        "the refused store had its journal rewritten"
    );
    // Nothing was opened for writing, so the store did not even acquire a writer lock.
    assert!(
        !data_root.join("writer.lock").exists(),
        "a refused store was locked for writing"
    );

    // Repeating the refusal is still a refusal, and still changes nothing.
    assert!(
        Application::open(&data_root).is_err(),
        "the second open must refuse too"
    );
    assert_eq!(
        fs::read(data_root.join("run.json")).expect("projection after"),
        projection_before
    );
}

#[test]
fn starting_a_run_over_an_incompatible_store_is_refused_without_a_write() {
    let root = tempfile::tempdir().expect("temporary root");
    let data_root = root.path().join("data");
    let (_, projection_before) = previous_version_store(&data_root);

    // A contract that is valid in every respect still cannot start a run over a store this
    // binary cannot read.
    let source = root.path().join("source");
    let negative_control = root.path().join("negative-control");
    let program = root.path().join("verify.sh");
    fs::create_dir_all(&source).expect("source");
    fs::create_dir_all(&negative_control).expect("negative control");
    fs::write(&program, b"#!/bin/sh\nexit 1\n").expect("program");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(&program).expect("metadata").permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(&program, permissions).expect("make executable");
    }
    let prepared = ymp_application::prepare_contract(&ymp_application::RunRequest {
        prompt: "keep the replay path idempotent".to_owned(),
        source,
        acceptance: Some(ymp_application::AcceptanceCondition::new(
            &program,
            &negative_control,
        )),
        capture_exclusions: Vec::new(),
        contract_id: None,
        budget: None,
    })
    .expect("prepared contract");

    let Err(error) = Application::create_with_contract(&data_root, &prepared) else {
        panic!("a run must not start over a store of another schema version");
    };
    assert!(
        matches!(error, ApplicationError::IncompatibleStore { actual: 1, .. }),
        "{error}"
    );
    assert_eq!(
        fs::read(data_root.join("run.json")).expect("projection after"),
        projection_before,
        "a refused start rewrote the projection"
    );
    assert!(
        !data_root.join("writer.lock").exists(),
        "a refused start locked the store for writing"
    );
}
