use sha2::{Digest, Sha256};
use std::path::Path;
use tempfile::tempdir;
use ymp_application::{Application, ApplicationError, VerificationInfrastructureError};
use ymp_domain::{Budget, Command, EventEnvelope, EventKind, RunStatus};
use ymp_storage::Journal;
use ymp_verifier::{
    EnvironmentBoundVerifier, ExactDigestVerifier, StoredVerificationEvidence, VerifiedEvidence,
    VerifierError,
};

enum EnvironmentMutation {
    ReplaceByte,
    RemoveObject,
    SubstituteDigest,
}

struct MutatingVerifier {
    inner: ExactDigestVerifier,
    mutation: EnvironmentMutation,
}

impl EnvironmentBoundVerifier for MutatingVerifier {
    fn verify_candidate_in_environment(
        &self,
        candidate_path: &Path,
        candidate_digest: &str,
        environment_path: &Path,
        environment_digest: &str,
    ) -> Result<VerifiedEvidence, VerifierError> {
        match self.mutation {
            EnvironmentMutation::ReplaceByte => {
                let mut bytes = std::fs::read(environment_path).expect("read environment");
                bytes[0] ^= 1;
                std::fs::write(environment_path, bytes).expect("replace environment byte");
                self.inner.verify_candidate_in_environment(
                    candidate_path,
                    candidate_digest,
                    environment_path,
                    environment_digest,
                )
            }
            EnvironmentMutation::RemoveObject => {
                std::fs::remove_file(environment_path).expect("remove environment object");
                self.inner.verify_candidate_in_environment(
                    candidate_path,
                    candidate_digest,
                    environment_path,
                    environment_digest,
                )
            }
            EnvironmentMutation::SubstituteDigest => self.inner.verify_candidate_in_environment(
                candidate_path,
                candidate_digest,
                environment_path,
                &"f".repeat(64),
            ),
        }
    }
}

fn submitted_application(data_root: &Path) -> (Application, String) {
    let mut app =
        Application::create(data_root, "run-1", Budget::new(1, 2)).expect("create application");
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
    (app, candidate_digest)
}

#[test]
fn only_bound_verifier_evidence_can_accept_a_candidate() {
    let temporary = tempdir().expect("temporary data root");
    let (mut app, candidate_digest) = submitted_application(temporary.path());
    let environment_object = br#"{"runtime":"exact-digest","toolchain":"sha256-v1"}"#;

    let verifier = ExactDigestVerifier::new("1".repeat(64), "2".repeat(64), &candidate_digest)
        .expect("configure verifier");
    let first = app
        .verify_with_environment("verify", environment_object, &verifier)
        .expect("commit verifier evidence");

    assert_eq!(app.state().status, RunStatus::Accepted);
    let evidence_digest = match &first.event.event {
        EventKind::VerificationRecorded {
            accepted,
            evidence_digest,
            ..
        } => {
            assert!(*accepted);
            evidence_digest.clone()
        }
        unexpected => panic!("unexpected event: {unexpected:?}"),
    };
    let evidence_bytes = app
        .object_store()
        .read(&evidence_digest)
        .expect("read evidence object");
    let restored: StoredVerificationEvidence =
        VerifiedEvidence::from_object_bytes(&evidence_bytes, &evidence_digest)
            .expect("restore evidence");
    let environment_digest = restored
        .environment_digest()
        .expect("bound environment digest");
    assert_eq!(
        environment_digest,
        ymp_domain::digest_bytes(environment_object)
    );
    app.object_store()
        .verify(environment_digest)
        .expect("environment object is durable");

    let reproduced = verifier
        .verify_candidate_in_environment(
            &app.object_store()
                .path_for(&candidate_digest)
                .expect("candidate path"),
            &candidate_digest,
            &app.object_store()
                .path_for(environment_digest)
                .expect("environment path"),
            environment_digest,
        )
        .expect("reproduce evidence from immutable inputs");
    assert_eq!(reproduced.evidence_digest(), evidence_digest);
    drop(app);
    let reopened = Application::open(temporary.path()).expect("recover accepted run");
    assert_eq!(reopened.state().status, RunStatus::Accepted);
}

#[test]
fn exact_verification_retry_returns_the_stored_result_without_spending_budget() {
    let temporary = tempdir().expect("temporary data root");
    let (mut app, candidate_digest) = submitted_application(temporary.path());
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
    let first = app
        .record_verification("verify-first", &evidence)
        .expect("record verification");
    let event_count = app.events_after(0).expect("events").len();
    let budget = app.state().budget.clone();

    let retry = app
        .record_verification("verify-retry", &evidence)
        .expect("return stored verification");

    assert!(retry.replayed);
    assert_eq!(retry.event, first.event);
    assert_eq!(app.events_after(0).expect("events").len(), event_count);
    assert_eq!(app.state().budget, budget);

    drop(app);
    let mut reopened = Application::open(temporary.path()).expect("reopen application");
    let reopened_count = reopened.events_after(0).expect("events").len();
    let reopened_budget = reopened.state().budget.clone();
    let retry = reopened
        .record_verification("verify-after-recovery", &evidence)
        .expect("return recovered verification");
    assert!(retry.replayed);
    assert_eq!(retry.event, first.event);
    assert_eq!(
        reopened.events_after(0).expect("events").len(),
        reopened_count
    );
    assert_eq!(reopened.state().budget, reopened_budget);
}

#[test]
fn altered_missing_or_digest_substituted_environment_is_infrastructure_error() {
    for (index, mutation) in [
        EnvironmentMutation::ReplaceByte,
        EnvironmentMutation::RemoveObject,
        EnvironmentMutation::SubstituteDigest,
    ]
    .into_iter()
    .enumerate()
    {
        let temporary = tempdir().expect("temporary data root");
        let (mut app, candidate_digest) = submitted_application(temporary.path());
        let verifier = MutatingVerifier {
            inner: ExactDigestVerifier::new("1".repeat(64), "2".repeat(64), &candidate_digest)
                .expect("configure verifier"),
            mutation,
        };
        let outcome = app
            .verify_with_environment(
                format!("verify-{index}"),
                b"runtime-environment-v1",
                &verifier,
            )
            .expect("record infrastructure failure");
        assert_eq!(outcome.status, RunStatus::InfrastructureError);
        assert!(matches!(outcome.event.event, EventKind::RunFailed { .. }));
        assert!(
            !app.events_after(0)
                .expect("read events")
                .iter()
                .any(|event| {
                    matches!(
                        event.event,
                        EventKind::VerificationRecorded { accepted: true, .. }
                    )
                })
        );
    }
}

#[test]
fn recovery_refuses_missing_or_altered_environment_objects() {
    for remove in [true, false] {
        let temporary = tempdir().expect("temporary data root");
        let (mut app, candidate_digest) = submitted_application(temporary.path());
        let verifier = ExactDigestVerifier::new("1".repeat(64), "2".repeat(64), candidate_digest)
            .expect("configure verifier");
        app.verify_with_environment("verify", b"runtime-environment-v1", &verifier)
            .expect("record accepted verification");
        let evidence_digest = app
            .events_after(0)
            .expect("read events")
            .into_iter()
            .find_map(|event| match event.event {
                EventKind::VerificationRecorded {
                    evidence_digest, ..
                } => Some(evidence_digest),
                _ => None,
            })
            .expect("verification evidence digest");
        let evidence_bytes = app
            .object_store()
            .read(&evidence_digest)
            .expect("read evidence");
        let evidence: StoredVerificationEvidence =
            VerifiedEvidence::from_object_bytes(&evidence_bytes, evidence_digest)
                .expect("restore evidence");
        let environment_digest = evidence
            .environment_digest()
            .expect("environment digest")
            .to_owned();
        let environment_path = app
            .object_store()
            .path_for(&environment_digest)
            .expect("environment path");
        drop(app);
        if remove {
            std::fs::remove_file(environment_path).expect("remove environment");
        } else {
            std::fs::write(environment_path, b"runtime-environment-v2").expect("alter environment");
        }

        let error = match Application::open(temporary.path()) {
            Ok(_) => panic!("invalid environment was accepted"),
            Err(error) => error,
        };
        assert!(matches!(
            (remove, error),
            (
                true,
                ApplicationError::VerificationInfrastructure(
                    VerificationInfrastructureError::EnvironmentObjectMissing(_)
                )
            ) | (
                false,
                ApplicationError::VerificationInfrastructure(
                    VerificationInfrastructureError::EnvironmentDigestMismatch(_)
                )
            )
        ));
    }
}

#[test]
fn ambiguous_version_one_evidence_blocks_recovery_as_infrastructure_error() {
    let temporary = tempdir().expect("temporary data root");
    let (app, candidate_digest) = submitted_application(temporary.path());
    let legacy_bytes = serde_json::to_vec(&serde_json::json!({
        "schema_version": 1,
        "candidate_digest": candidate_digest,
        "contract_digest": "1".repeat(64),
        "oracle_digest": "2".repeat(64),
        "verifier_profile": "exact_digest_v1",
        "observation_digest": "3".repeat(64),
        "decision": "accept",
        "evidence_digest": ""
    }))
    .expect("serialize legacy evidence");
    let legacy_digest = app
        .object_store()
        .put(&legacy_bytes)
        .expect("store legacy evidence");
    let state = app.state().clone();
    drop(app);

    let (mut journal, _) =
        Journal::open(temporary.path().join("events.jsonl")).expect("open journal");
    let event = EventEnvelope::new(
        &state.run_id,
        state.last_sequence + 1,
        "legacy-verification",
        legacy_digest.clone(),
        Some(state.last_event_digest),
        EventKind::VerificationRecorded {
            candidate_digest,
            contract_digest: "1".repeat(64),
            oracle_digest: "2".repeat(64),
            evidence_digest: legacy_digest,
            accepted: true,
        },
    )
    .expect("legacy event");
    journal.append(&event).expect("append legacy event");
    drop(journal);

    assert!(matches!(
        Application::open(temporary.path()),
        Err(ApplicationError::VerificationInfrastructure(
            VerificationInfrastructureError::AmbiguousLegacyEvidence
        ))
    ));
    let state: serde_json::Value = serde_json::from_slice(
        &std::fs::read(temporary.path().join("run.json")).expect("read run state"),
    )
    .expect("parse run state");
    assert_eq!(state["status"], "infrastructure_error");
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
