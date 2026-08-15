#![forbid(unsafe_code)]

//! Acceptance: an export can put the accepted candidate into the project directory itself, and
//! puts nothing else there.
//!
//! Each half fails on its own.
//!
//! * Applied in place, the project directory holds the candidate's files and only those: the same
//!   store exported as a bundle writes a journal, a run state and a manifest beside them, which is
//!   the delivery form this mode replaces and the negative half of the first check.
//! * Only an accepted candidate is applied. A candidate that has been submitted and not yet judged
//!   is refused by the run's own status, and the project directory stays empty.
//! * A file the project already holds is named and nothing at all is written; the same call with
//!   the replacement stated writes the candidate over it and reports what it replaced.

use std::fs;
use std::path::Path;

use tempfile::tempdir;
use ymp_application::{Application, ApplicationError};
use ymp_domain::{Budget, Command, RunStatus};
use ymp_verifier::ExactDigestVerifier;

/// A run whose candidate is a two-file tree, carried as far as the caller asks.
struct Run {
    application: Application,
    candidate_digest: String,
}

fn submitted_run(base: &Path) -> Run {
    let source = base.join("source");
    let workspace = base.join("workspace");
    fs::create_dir_all(source.join("src")).expect("create source");
    fs::write(source.join("src/lib.rs"), b"pub const VALUE: u8 = 1;\n").expect("write source");

    let mut application = Application::create(base.join("data"), "run-1", Budget::new(1, 1))
        .expect("create application");
    let artifacts = application.artifact_store();
    let captured = artifacts.capture_source(&source).expect("capture base");
    artifacts
        .materialize(&captured.manifest_digest, &workspace)
        .expect("materialize workspace");
    application
        .execute(
            "start-1",
            Command::StartAttempt {
                attempt_id: "attempt-1".to_owned(),
            },
        )
        .expect("start attempt");
    fs::write(workspace.join("src/lib.rs"), b"pub const VALUE: u8 = 2;\n")
        .expect("modify workspace");
    fs::write(workspace.join("result.txt"), b"done\n").expect("add workspace file");

    let submitted = application
        .submit_workspace_candidate(
            "submit-1",
            "attempt-1",
            &captured.manifest_digest,
            &workspace,
        )
        .expect("submit workspace candidate");
    Run {
        candidate_digest: submitted.candidate.snapshot_digest,
        application,
    }
}

fn accepted_run(base: &Path) -> Run {
    let mut run = submitted_run(base);
    let verifier = ExactDigestVerifier::new("1".repeat(64), "2".repeat(64), &run.candidate_digest)
        .expect("configure verifier");
    run.application
        .verify_with_environment(
            "verify-1",
            br#"{"profile":"workspace-exact-digest-v1"}"#,
            &verifier,
        )
        .expect("record verification");
    assert_eq!(run.application.state().status, RunStatus::Accepted);
    run
}

/// Everything the directory holds, as paths relative to it and in a stated order.
fn tree(root: &Path) -> Vec<String> {
    let mut found = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(&directory).expect("read directory").flatten() {
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
                continue;
            }
            found.push(
                path.strip_prefix(root)
                    .expect("path under the root")
                    .display()
                    .to_string(),
            );
        }
    }
    found.sort();
    found
}

#[test]
fn an_applied_candidate_is_the_only_thing_the_project_directory_receives() {
    let temporary = tempdir().expect("temporary directory");
    let run = accepted_run(temporary.path());
    let project = temporary.path().join("project");

    let report = run
        .application
        .apply_candidate(&project, false)
        .expect("apply the accepted candidate");
    assert_eq!(report.candidate_digest, run.candidate_digest);
    assert_eq!(report.replaced_paths, Vec::<String>::new());
    assert_eq!(
        report.applied_paths,
        vec!["result.txt".to_owned(), "src/lib.rs".to_owned()]
    );
    assert_eq!(
        tree(&project),
        vec!["result.txt".to_owned(), "src/lib.rs".to_owned()],
        "the project directory received something other than the candidate's files"
    );
    assert_eq!(
        fs::read_to_string(project.join("src/lib.rs")).expect("applied file"),
        "pub const VALUE: u8 = 2;\n"
    );

    // The negative half: the delivery form this mode replaces. The same store exported as a
    // bundle carries the run's record as well, which is exactly what an applied candidate must
    // not leave in a directory an operator works in.
    let bundle = temporary.path().join("bundle");
    run.application
        .export_evidence(&bundle)
        .expect("export the evidence bundle");
    for part in ["manifest.json", "events.jsonl", "state.json"] {
        assert!(
            bundle.join(part).is_file(),
            "the bundle carries no {part}, so it is not the form being distinguished from"
        );
    }
    assert!(bundle.join("candidate/result.txt").is_file());
}

#[test]
fn a_candidate_no_verdict_has_accepted_is_not_applied() {
    let temporary = tempdir().expect("temporary directory");
    let run = submitted_run(temporary.path());
    let project = temporary.path().join("project");

    let refusal = run.application.apply_candidate(&project, false);
    assert!(
        matches!(
            refusal,
            Err(ApplicationError::CandidateNotAccepted {
                actual: RunStatus::Running
            })
        ),
        "an unjudged candidate was applied: {refusal:?}"
    );
    assert!(
        !project.exists(),
        "a refused application still reached the project directory"
    );
}

#[test]
fn a_file_the_project_holds_is_named_and_replaced_only_when_the_operator_states_it() {
    let temporary = tempdir().expect("temporary directory");
    let run = accepted_run(temporary.path());
    let project = temporary.path().join("project");
    fs::create_dir_all(project.join("src")).expect("create project");
    fs::write(project.join("src/lib.rs"), b"the operator's own work\n").expect("project file");

    let refusal = run.application.apply_candidate(&project, false);
    assert!(
        matches!(
            &refusal,
            Err(ApplicationError::ApplyWouldOverwrite(paths))
                if paths == &vec!["src/lib.rs".to_owned()]
        ),
        "a file the project already held was not named: {refusal:?}"
    );
    assert_eq!(
        fs::read_to_string(project.join("src/lib.rs")).expect("project file"),
        "the operator's own work\n",
        "a refused application changed a project file"
    );
    assert_eq!(
        tree(&project),
        vec!["src/lib.rs".to_owned()],
        "a refused application wrote the rest of the candidate anyway"
    );

    let report = run
        .application
        .apply_candidate(&project, true)
        .expect("apply with the replacement stated");
    assert_eq!(report.replaced_paths, vec!["src/lib.rs".to_owned()]);
    assert_eq!(
        fs::read_to_string(project.join("src/lib.rs")).expect("applied file"),
        "pub const VALUE: u8 = 2;\n"
    );
    assert_eq!(
        tree(&project),
        vec!["result.txt".to_owned(), "src/lib.rs".to_owned()]
    );
}
