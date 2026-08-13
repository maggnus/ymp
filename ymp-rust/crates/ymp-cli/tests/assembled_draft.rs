#![forbid(unsafe_code)]

//! Acceptance, driven on the built product: a request in a project that runs its tests reaches a
//! drafted contract without a question sequence, and authorizing it still costs the contract id.
//!
//! Everything here runs the `ymp` executable in a project directory, which is what an operator
//! has: a source tree with a way of running its tests. The product proposes the acceptance
//! condition from that, copies the project as the negative control, demonstrates both decisions,
//! and states the whole draft at once.
//!
//! The negative half is the dialogue this card replaced, which asked three questions — the
//! directory, the verifier and the deliberately wrong candidate — before it could draft anything:
//! `a_request_alone_reaches_a_drafted_contract` reports each of those questions on the screen,
//! and reports the request itself refused for want of an acceptance condition.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use tempfile::TempDir;

/// The questions the replaced dialogue put before it could draft anything. None of them may be
/// put now: the product knows the project, and a path the operator has no way to know is never
/// demanded of them.
const RETIRED_QUESTIONS: [&str; 3] = [
    "which directory is this work done in",
    "which program decides whether a candidate is accepted",
    "which deliberately wrong candidate must that program reject",
];

struct Project {
    _root: TempDir,
    directory: PathBuf,
    data_root: PathBuf,
}

/// A project as an operator has one: it runs its tests through a script of its own, and that
/// script fails while the work is not done.
fn project() -> Project {
    let root = TempDir::new().expect("temporary root");
    let directory = root
        .path()
        .canonicalize()
        .expect("resolve root")
        .join("work");
    let data_root = root.path().join("data");
    fs::create_dir_all(directory.join("scripts")).expect("project directory");
    fs::write(directory.join("README.md"), b"a project\n").expect("project file");
    let entry_point = directory.join("scripts/test.sh");
    fs::write(&entry_point, b"#!/bin/sh\ntest -f result.txt\n").expect("test entry point");
    make_executable(&entry_point);
    Project {
        _root: root,
        directory,
        data_root,
    }
}

#[cfg(unix)]
fn make_executable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;

    let mut permissions = fs::metadata(path).expect("metadata").permissions();
    permissions.set_mode(0o700);
    fs::set_permissions(path, permissions).expect("make executable");
}

#[cfg(not(unix))]
fn make_executable(_path: &Path) {}

impl Project {
    /// Run one command of the product from inside the project, which is what decides the source.
    fn run(&self, arguments: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_ymp"))
            .current_dir(&self.directory)
            .arg("--data-root")
            .arg(&self.data_root)
            .args(arguments)
            .output()
            .expect("run the ymp executable")
    }
}

/// The transcript with its line breaks and indentation collapsed: it is wrapped to the width a
/// command lays its surfaces out at, so a statement is looked for as words.
fn flattened(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// The identifier of the contract a transcript states.
fn contract_id(transcript: &str) -> String {
    transcript
        .split_whitespace()
        .find(|word| word.starts_with("contract-"))
        .expect("the transcript states a contract id")
        .to_owned()
}

#[test]
fn a_request_alone_reaches_a_drafted_contract() {
    let project = project();
    let drafted = project.run(&["request", "--prompt=keep the replay path idempotent"]);
    let transcript = flattened(&drafted);

    assert!(
        drafted.status.success(),
        "a request in a project that runs its tests was refused:\n{transcript}\n{}",
        String::from_utf8_lossy(&drafted.stderr)
    );
    assert!(transcript.contains("drafted · digest"), "{transcript}");

    // The product proposed the verifier from the project's own entry point, supplied the
    // negative control, and stated what each half of the demonstration decided.
    assert!(transcript.contains("scripts/test.sh"), "{transcript}");
    assert!(
        transcript.contains("a copy of the project as it stands"),
        "{transcript}"
    );
    assert!(
        transcript.contains(
            "rejected the negative control, and rejected a candidate that had \
             replaced this project's test entry point"
        ),
        "the draft does not state both halves of what was demonstrated:\n{transcript}"
    );

    // One statement, not a sequence of questions.
    for question in RETIRED_QUESTIONS {
        assert!(
            !transcript.contains(question),
            "the dialogue still asks `{question}`:\n{transcript}"
        );
    }

    assert!(
        !project.data_root.join("events.jsonl").exists(),
        "drafting a contract started a run"
    );
}

/// The contract model underneath is unchanged: the acceptance condition takes effect only
/// through the operator's approval, and a first approval is the contract id typed in full.
#[test]
fn authorizing_the_assembled_contract_costs_the_contract_id() {
    let project = project();
    let drafted = project.run(&["request", "--prompt=keep the replay path idempotent"]);
    assert!(drafted.status.success());
    let identifier = contract_id(&flattened(&drafted));

    for confirmation in [
        None,
        Some("wrong".to_owned()),
        Some(identifier[..8].to_owned()),
    ] {
        let mut arguments = vec![
            "start".to_owned(),
            "--prompt=keep the replay path idempotent".to_owned(),
        ];
        if let Some(typed) = &confirmation {
            arguments.push(format!("--confirm={typed}"));
        }
        let refused = project.run(&arguments.iter().map(String::as_str).collect::<Vec<_>>());
        assert!(
            !refused.status.success(),
            "a run started on the confirmation {confirmation:?}"
        );
        assert!(
            !project.data_root.join("events.jsonl").exists(),
            "the confirmation {confirmation:?} committed a journal"
        );
    }

    let started = project.run(&[
        "start",
        "--prompt=keep the replay path idempotent",
        &format!("--confirm={identifier}"),
    ]);
    assert!(
        started.status.success(),
        "the exact contract id did not start the run:\n{}",
        String::from_utf8_lossy(&started.stderr)
    );
    assert!(
        project.data_root.join("events.jsonl").exists(),
        "the authorized contract recorded no journal"
    );
}
