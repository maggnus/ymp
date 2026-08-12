use sha2::{Digest, Sha256};
use tempfile::tempdir;
use ymp_application::{Application, ApplicationError};
use ymp_domain::{Budget, Command, EventKind, RunStatus};
use ymp_verifier::ExactDigestVerifier;

#[test]
fn only_bound_verifier_evidence_can_accept_a_candidate() {
    let temporary = tempdir().expect("temporary data root");
    let mut app = Application::create(temporary.path(), "run-1", Budget::new(1, 2))
        .expect("create application");
    let candidate_bytes = b"candidate";
    let candidate_digest = app
        .object_store()
        .put(candidate_bytes)
        .expect("store candidate");
    app.execute(
        "start",
        Command::StartAttempt {
            attempt_id: "attempt-1".to_owned(),
        },
    )
    .expect("start attempt");
    app.execute(
        "submit",
        Command::SubmitCandidate {
            attempt_id: "attempt-1".to_owned(),
            base_digest: "0".repeat(64),
            object_digest: candidate_digest.clone(),
        },
    )
    .expect("submit candidate");

    let verifier = ExactDigestVerifier::new("1".repeat(64), "2".repeat(64), &candidate_digest)
        .expect("configure verifier");
    let evidence = verifier
        .verify_candidate(
            app.object_store()
                .path_for(&candidate_digest)
                .expect("candidate path"),
            &candidate_digest,
        )
        .expect("verify candidate");
    let first = app
        .record_verification("verify", &evidence)
        .expect("commit verifier evidence");
    let replay = app
        .record_verification("verify", &evidence)
        .expect("replay verifier evidence");

    assert_eq!(app.state().status, RunStatus::Accepted);
    app.object_store()
        .verify(evidence.evidence_digest())
        .expect("evidence object is durable");
    assert!(replay.replayed);
    assert_eq!(replay.event, first.event);
    match first.event.event {
        EventKind::VerificationRecorded {
            accepted,
            evidence_digest,
            ..
        } => {
            assert!(accepted);
            assert_eq!(evidence_digest, evidence.evidence_digest());
        }
        unexpected => panic!("unexpected event: {unexpected:?}"),
    }
}

#[test]
fn rejected_evidence_cannot_accept_and_command_ids_remain_content_bound() {
    let temporary = tempdir().expect("temporary data root");
    let mut app = Application::create(temporary.path(), "run-1", Budget::new(1, 2))
        .expect("create application");
    let candidate_digest = app
        .object_store()
        .put(b"candidate")
        .expect("store candidate");
    app.execute(
        "start",
        Command::StartAttempt {
            attempt_id: "attempt-1".to_owned(),
        },
    )
    .expect("start attempt");
    app.execute(
        "submit",
        Command::SubmitCandidate {
            attempt_id: "attempt-1".to_owned(),
            base_digest: "0".repeat(64),
            object_digest: candidate_digest.clone(),
        },
    )
    .expect("submit candidate");

    let wrong_expected = hex::encode(Sha256::digest(b"different candidate"));
    let rejecting_verifier =
        ExactDigestVerifier::new("1".repeat(64), "2".repeat(64), wrong_expected)
            .expect("configure rejecting verifier");
    let rejected = rejecting_verifier
        .verify_candidate(
            app.object_store()
                .path_for(&candidate_digest)
                .expect("candidate path"),
            &candidate_digest,
        )
        .expect("verify candidate");
    app.record_verification("verify", &rejected)
        .expect("commit rejection");
    assert_eq!(app.state().status, RunStatus::Running);

    let accepting_verifier =
        ExactDigestVerifier::new("1".repeat(64), "2".repeat(64), &candidate_digest)
            .expect("configure accepting verifier");
    let accepted = accepting_verifier
        .verify_candidate(
            app.object_store()
                .path_for(&candidate_digest)
                .expect("candidate path"),
            &candidate_digest,
        )
        .expect("verify candidate");
    assert!(matches!(
        app.record_verification("verify", &accepted),
        Err(ApplicationError::IdempotencyConflict { .. })
    ));
    assert_eq!(app.state().status, RunStatus::Running);
}

#[test]
fn recovery_rejects_a_missing_verifier_evidence_object() {
    let temporary = tempdir().expect("temporary data root");
    let evidence_digest = {
        let mut app = Application::create(temporary.path(), "run-1", Budget::new(1, 1))
            .expect("create application");
        let candidate_digest = app
            .object_store()
            .put(b"candidate")
            .expect("store candidate");
        app.execute(
            "start",
            Command::StartAttempt {
                attempt_id: "attempt-1".to_owned(),
            },
        )
        .expect("start attempt");
        app.execute(
            "submit",
            Command::SubmitCandidate {
                attempt_id: "attempt-1".to_owned(),
                base_digest: "0".repeat(64),
                object_digest: candidate_digest.clone(),
            },
        )
        .expect("submit candidate");
        let verifier = ExactDigestVerifier::new("1".repeat(64), "2".repeat(64), &candidate_digest)
            .expect("configure verifier");
        let evidence = verifier
            .verify_candidate(
                app.object_store()
                    .path_for(&candidate_digest)
                    .expect("candidate path"),
                candidate_digest,
            )
            .expect("verify candidate");
        app.record_verification("verify", &evidence)
            .expect("commit verifier evidence");
        let path = app
            .object_store()
            .path_for(evidence.evidence_digest())
            .expect("evidence path");
        std::fs::remove_file(path).expect("remove evidence object");
        evidence.evidence_digest().to_owned()
    };

    assert!(matches!(
        Application::open(temporary.path()),
        Err(ApplicationError::ObjectStore(
            ymp_storage::ObjectStoreError::Missing(digest)
        )) if digest == evidence_digest
    ));
}
