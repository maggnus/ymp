use tempfile::tempdir;
use ymp_application::{Application, ApplicationError};
use ymp_domain::{Budget, Command};
use ymp_verifier::ExactDigestVerifier;

#[test]
fn quiescent_private_workspace_becomes_reproducible_candidate() {
    let temporary = tempdir().expect("temporary directory");
    let source = temporary.path().join("source");
    let workspace = temporary.path().join("workspace");
    std::fs::create_dir_all(source.join("src")).expect("create source");
    std::fs::write(source.join("src/lib.rs"), b"pub const VALUE: u8 = 1;\n").expect("write source");

    let mut application =
        Application::create(temporary.path().join("data"), "run-1", Budget::new(1, 1))
            .expect("create application");
    let artifacts = application.artifact_store();
    let base = artifacts.capture_source(&source).expect("capture base");
    artifacts
        .materialize(&base.manifest_digest, &workspace)
        .expect("materialize workspace");
    application
        .execute(
            "start-1",
            Command::StartAttempt {
                attempt_id: "attempt-1".to_owned(),
            },
        )
        .expect("start attempt");
    std::fs::write(workspace.join("src/lib.rs"), b"pub const VALUE: u8 = 2;\n")
        .expect("modify workspace");

    let submitted = application
        .submit_workspace_candidate("submit-1", "attempt-1", &base.manifest_digest, &workspace)
        .expect("submit workspace candidate");
    assert_eq!(
        application.state().candidate_digest.as_deref(),
        Some(submitted.candidate.snapshot_digest.as_str())
    );

    let reproduced = artifacts
        .build_candidate(&base.manifest_digest, &submitted.submission.manifest_digest)
        .expect("reproduce candidate");
    assert_eq!(reproduced, submitted.candidate);

    let verifier = ExactDigestVerifier::new(
        "1".repeat(64),
        "2".repeat(64),
        &submitted.candidate.snapshot_digest,
    )
    .expect("configure verifier");
    let environment = br#"{"profile":"workspace-exact-digest-v1"}"#;
    let verification = application
        .verify_with_environment("verify-1", environment, &verifier)
        .expect("record verification");
    let evidence_digest = match verification.event.event {
        ymp_domain::EventKind::VerificationRecorded {
            evidence_digest, ..
        } => evidence_digest,
        unexpected => panic!("unexpected event: {unexpected:?}"),
    };
    let environment_digest = ymp_domain::digest_bytes(environment);
    let export = temporary.path().join("export");
    let report = application
        .export_evidence(&export)
        .expect("export evidence");
    assert_eq!(report.candidate_digest, submitted.candidate.snapshot_digest);
    assert_eq!(report.evidence_digests, vec![evidence_digest.clone()]);
    assert_eq!(report.environment_digests, vec![environment_digest.clone()]);
    assert!(export.join("candidate/src/lib.rs").is_file());
    assert!(
        export
            .join(format!("evidence/{evidence_digest}.json"))
            .is_file()
    );
    assert!(
        export
            .join("environments")
            .join(environment_digest)
            .is_file()
    );
    assert!(matches!(
        application.export_evidence(&export),
        Err(ApplicationError::ExportAlreadyExists(path)) if path == export
    ));
}
