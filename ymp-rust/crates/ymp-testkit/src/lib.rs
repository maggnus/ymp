#![forbid(unsafe_code)]

use serde::Serialize;
use std::path::Path;
use ymp_application::{Application, ApplicationError};
use ymp_artifacts::{ArtifactError, CandidateRef, SnapshotRef, SubmissionRef};
use ymp_domain::{Budget, Command, RunState};
use ymp_verifier::{ExactDigestVerifier, VerifierError};

pub mod origin_start;
pub mod ready_root;

#[derive(Clone, Debug, Serialize)]
pub struct DemoReport {
    pub state: RunState,
    pub source: SnapshotRef,
    pub submission: SubmissionRef,
    pub candidate: CandidateRef,
    pub duplicate_command_replayed: bool,
    pub committed_event_count: usize,
}

#[derive(Debug, thiserror::Error)]
pub enum DemoError {
    #[error(transparent)]
    Application(#[from] ApplicationError),
    #[error(transparent)]
    Artifact(#[from] ArtifactError),
    #[error(transparent)]
    Verifier(#[from] VerifierError),
    #[error("demo fixture I/O error: {0}")]
    Io(#[from] std::io::Error),
}

pub fn run_accepted_demo(data_root: impl AsRef<Path>) -> Result<DemoReport, DemoError> {
    let data_root = data_root.as_ref().to_path_buf();
    let mut app = Application::create(&data_root, "demo-run", Budget::new(2, 1))?;
    let (source, submission, candidate) = build_candidate_fixture(&app, &data_root)?;
    let candidate_object_path = app
        .object_store()
        .path_for(&candidate.snapshot_digest)
        .expect("candidate digest was produced by the object store");
    let verifier = ExactDigestVerifier::new(
        "1111111111111111111111111111111111111111111111111111111111111111",
        "2222222222222222222222222222222222222222222222222222222222222222",
        &candidate.snapshot_digest,
    )?;
    let evidence = verifier.verify_candidate(candidate_object_path, &candidate.snapshot_digest)?;

    app.execute(
        "demo.start-attempt",
        Command::StartAttempt {
            attempt_id: "attempt-1".to_owned(),
        },
    )?;
    let duplicate = app.execute(
        "demo.start-attempt",
        Command::StartAttempt {
            attempt_id: "attempt-1".to_owned(),
        },
    )?;
    app.execute(
        "demo.submit-candidate",
        Command::SubmitCandidate {
            attempt_id: "attempt-1".to_owned(),
            base_digest: source.manifest_digest.clone(),
            object_digest: candidate.snapshot_digest.clone(),
        },
    )?;
    app.record_verification("demo.verify-candidate", &evidence)?;
    let events = app.events_after(0)?;

    Ok(DemoReport {
        state: app.state().clone(),
        source,
        submission,
        candidate,
        duplicate_command_replayed: duplicate.replayed,
        committed_event_count: events.len(),
    })
}

fn build_candidate_fixture(
    app: &Application,
    data_root: &Path,
) -> Result<(SnapshotRef, SubmissionRef, CandidateRef), DemoError> {
    let source_root = data_root.join("demo-source");
    let workspace_root = data_root.join("workspaces/attempt-1");
    std::fs::create_dir_all(source_root.join("src"))?;
    std::fs::write(
        source_root.join("src/lib.rs"),
        b"pub fn answer() -> u8 { 41 }\n",
    )?;
    std::fs::write(source_root.join("README.md"), b"demo source\n")?;

    let artifacts = app.artifact_store();
    let source = artifacts.capture_source(&source_root)?;
    artifacts.materialize(&source.manifest_digest, &workspace_root)?;
    std::fs::write(
        workspace_root.join("src/lib.rs"),
        b"pub fn answer() -> u8 { 42 }\n",
    )?;
    std::fs::write(workspace_root.join("EVIDENCE.txt"), b"fake runtime\n")?;
    let submission = artifacts.create_submission(&source.manifest_digest, &workspace_root)?;
    let candidate =
        artifacts.build_candidate(&source.manifest_digest, &submission.manifest_digest)?;
    Ok((source, submission, candidate))
}
