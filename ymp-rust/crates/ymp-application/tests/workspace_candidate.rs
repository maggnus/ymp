use tempfile::tempdir;
use ymp_application::{Application, ApplicationError};
use ymp_domain::{Budget, Command};
use ymp_verifier::ExactDigestVerifier;

mod support;

use support::FixtureObjects;

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

#[test]
fn conflicting_integration_cannot_replace_an_existing_candidate() {
    let temporary = tempdir().expect("temporary directory");
    let source = temporary.path().join("source");
    let first_workspace = temporary.path().join("workspace-first");
    let second_workspace = temporary.path().join("workspace-second");
    std::fs::create_dir_all(&source).expect("create source");
    std::fs::write(source.join("result.txt"), b"base\n").expect("write source");

    let data_root = temporary.path().join("data");
    let mut application =
        Application::create(&data_root, "run-1", Budget::new(2, 1)).expect("create application");
    let artifacts = application.artifact_store();
    let base = artifacts.capture_source(&source).expect("capture base");
    artifacts
        .materialize(&base.manifest_digest, &first_workspace)
        .expect("materialize first workspace");
    artifacts
        .materialize(&base.manifest_digest, &second_workspace)
        .expect("materialize second workspace");
    application
        .execute(
            "start-1",
            Command::StartAttempt {
                attempt_id: "attempt-1".to_owned(),
            },
        )
        .expect("start first attempt");
    application
        .execute(
            "start-2",
            Command::StartAttempt {
                attempt_id: "attempt-2".to_owned(),
            },
        )
        .expect("start second attempt");

    std::fs::write(first_workspace.join("result.txt"), b"first\n").expect("modify first workspace");
    let first = application
        .submit_workspace_candidate(
            "submit-1",
            "attempt-1",
            &base.manifest_digest,
            &first_workspace,
        )
        .expect("submit first candidate");
    assert_eq!(
        std::fs::read(source.join("result.txt")).expect("read immutable source"),
        b"base\n"
    );
    let event_count = application.events_after(0).expect("read events").len();

    std::fs::write(second_workspace.join("result.txt"), b"second\n")
        .expect("modify second workspace");
    let error = application
        .submit_workspace_candidate(
            "submit-2",
            "attempt-2",
            &base.manifest_digest,
            &second_workspace,
        )
        .expect_err("reject conflicting integration");

    assert!(matches!(error, ApplicationError::CandidateConflict { .. }));
    assert_eq!(
        application.state().candidate_digest.as_deref(),
        Some(first.candidate.snapshot_digest.as_str())
    );
    assert_eq!(
        application.events_after(0).expect("read events").len(),
        event_count
    );

    let objects = FixtureObjects::at(&data_root);
    let altered_digest = objects.put(b"proposed candidate");
    assert_eq!(objects.read(&altered_digest), b"proposed candidate");
    objects
        .verify(&altered_digest)
        .expect("fixture candidate is initially intact");
    let altered_path = objects.path_for(&altered_digest);
    std::fs::write(altered_path, b"altered candidate").expect("alter proposed candidate");
    assert!(matches!(
        application.execute(
            "submit-altered",
            Command::SubmitCandidate {
                attempt_id: "attempt-2".to_owned(),
                base_digest: base.manifest_digest.clone(),
                object_digest: altered_digest,
            },
        ),
        Err(ApplicationError::ObjectStore(
            ymp_storage::ObjectStoreError::DigestMismatch(_)
        ))
    ));
    assert!(matches!(
        application.execute(
            "submit-invalid-digest",
            Command::SubmitCandidate {
                attempt_id: "attempt-2".to_owned(),
                base_digest: base.manifest_digest,
                object_digest: "not-a-digest".to_owned(),
            },
        ),
        Err(ApplicationError::ObjectStore(
            ymp_storage::ObjectStoreError::InvalidDigest(_)
        ))
    ));
    assert_eq!(
        application.state().candidate_digest.as_deref(),
        Some(first.candidate.snapshot_digest.as_str())
    );
    assert_eq!(
        application.events_after(0).expect("read events").len(),
        event_count
    );
}

#[test]
fn identical_candidate_tree_from_a_different_base_is_rejected() {
    let temporary = tempdir().expect("temporary directory");
    let first_source = temporary.path().join("source-first");
    let second_source = temporary.path().join("source-second");
    let first_workspace = temporary.path().join("workspace-first");
    let second_workspace = temporary.path().join("workspace-second");
    std::fs::create_dir(&first_source).expect("create first source");
    std::fs::create_dir(&second_source).expect("create second source");
    std::fs::write(first_source.join("result.txt"), b"first base\n").expect("first source");
    std::fs::write(second_source.join("result.txt"), b"second base\n").expect("second source");

    let data_root = temporary.path().join("data");
    let mut application =
        Application::create(&data_root, "run-1", Budget::new(2, 1)).expect("create application");
    let artifacts = application.artifact_store();
    let first_base = artifacts.capture_source(&first_source).expect("first base");
    let second_base = artifacts
        .capture_source(&second_source)
        .expect("second base");
    artifacts
        .materialize(&first_base.manifest_digest, &first_workspace)
        .expect("first workspace");
    artifacts
        .materialize(&second_base.manifest_digest, &second_workspace)
        .expect("second workspace");
    for (command_id, attempt_id) in [("start-1", "attempt-1"), ("start-2", "attempt-2")] {
        application
            .execute(
                command_id,
                Command::StartAttempt {
                    attempt_id: attempt_id.to_owned(),
                },
            )
            .expect("start attempt");
    }
    std::fs::write(first_workspace.join("result.txt"), b"same candidate\n")
        .expect("first candidate");
    std::fs::write(second_workspace.join("result.txt"), b"same candidate\n")
        .expect("second candidate");
    let first = application
        .submit_workspace_candidate(
            "submit-1",
            "attempt-1",
            &first_base.manifest_digest,
            &first_workspace,
        )
        .expect("submit first candidate");
    let event_count = application.events_after(0).expect("events").len();
    drop(application);
    let mut application = Application::open(&data_root).expect("recover candidate identity");

    let error = application
        .submit_workspace_candidate(
            "submit-2",
            "attempt-2",
            &second_base.manifest_digest,
            &second_workspace,
        )
        .expect_err("reject alternate ancestry");

    assert!(matches!(error, ApplicationError::CandidateConflict { .. }));
    assert_eq!(
        application.state().candidate_digest.as_deref(),
        Some(first.candidate.snapshot_digest.as_str())
    );
    assert_eq!(
        application.events_after(0).expect("events").len(),
        event_count
    );
}

/// Two attempts working at the same time from one base reach two immutable bundles and two
/// results, and neither reaches into the other's workspace or rewrites what the other produced.
///
/// It is the byte-level half of what the commitment kernel keeps over the ancestry of a result:
/// there the records are compared, and here the directories and the objects they were captured
/// from. Rebuilding the first result after the second exists reproduces it exactly, which is what
/// makes it immutable rather than merely unmodified so far.
#[test]
fn two_workspaces_from_one_base_produce_two_results_and_change_neither() {
    let temporary = tempdir().expect("temporary directory");
    let source = temporary.path().join("source");
    let first_workspace = temporary.path().join("workspace-first");
    let second_workspace = temporary.path().join("workspace-second");
    std::fs::create_dir_all(source.join("src")).expect("create source");
    std::fs::write(source.join("src/lib.rs"), b"pub const VALUE: u8 = 0;\n").expect("write source");

    let application =
        Application::create(temporary.path().join("data"), "run-1", Budget::new(2, 1))
            .expect("create application");
    let artifacts = application.artifact_store();
    let base = artifacts.capture_source(&source).expect("capture base");
    for workspace in [&first_workspace, &second_workspace] {
        artifacts
            .materialize(&base.manifest_digest, workspace)
            .expect("materialize workspace");
    }

    std::fs::write(
        first_workspace.join("src/first.rs"),
        b"pub const FIRST: u8 = 1;\n",
    )
    .expect("modify first workspace");
    std::fs::write(
        second_workspace.join("src/second.rs"),
        b"pub const SECOND: u8 = 2;\n",
    )
    .expect("modify second workspace");
    let quiescent: Vec<String> = [&first_workspace, &second_workspace]
        .into_iter()
        .map(|workspace| {
            artifacts
                .capture_source(workspace)
                .expect("capture workspace")
                .manifest_digest
        })
        .collect();

    let first_bundle = artifacts
        .create_submission(&base.manifest_digest, &first_workspace)
        .expect("first bundle");
    let first = artifacts
        .build_candidate(&base.manifest_digest, &first_bundle.manifest_digest)
        .expect("first result");
    let second_bundle = artifacts
        .create_submission(&base.manifest_digest, &second_workspace)
        .expect("second bundle");
    let second = artifacts
        .build_candidate(&base.manifest_digest, &second_bundle.manifest_digest)
        .expect("second result");

    assert_ne!(first_bundle.manifest_digest, second_bundle.manifest_digest);
    assert_ne!(first.snapshot_digest, second.snapshot_digest);
    assert_eq!(first.base_snapshot_digest, base.manifest_digest);
    assert_eq!(second.base_snapshot_digest, base.manifest_digest);

    // Neither workspace was touched by the other's submission, and neither was the source they
    // were both taken from.
    for (workspace, before) in [&first_workspace, &second_workspace]
        .into_iter()
        .zip(quiescent)
    {
        assert_eq!(
            artifacts
                .capture_source(workspace)
                .expect("capture workspace")
                .manifest_digest,
            before,
            "a submission wrote into a workspace"
        );
    }
    assert_eq!(
        artifacts
            .capture_source(&source)
            .expect("capture source")
            .manifest_digest,
        base.manifest_digest,
        "a submission wrote into the base it was taken from"
    );

    // The first result is rebuilt from the same clean inputs, byte for byte, after the second one
    // exists.
    assert_eq!(
        artifacts
            .build_candidate(&base.manifest_digest, &first_bundle.manifest_digest)
            .expect("rebuild the first result"),
        first
    );
    let files: Vec<String> = artifacts
        .load_snapshot(&first.snapshot_digest)
        .expect("first result manifest")
        .files
        .into_iter()
        .map(|entry| entry.path)
        .collect();
    assert_eq!(
        files,
        vec!["src/first.rs".to_owned(), "src/lib.rs".to_owned()]
    );
}
