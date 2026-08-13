#![forbid(unsafe_code)]

//! Acceptance, driven on the built product: an answer is judged where it is typed, and a verifier
//! answer is judged by what the program does with the negative control.
//!
//! Everything here runs the `ymp` executable through the command that mirrors the interface's
//! draft, so what is asserted is what an operator reaches. Three refusals are required, and each
//! has to arrive at the line that caused it rather than after the lines that follow it: the
//! transcript the command prints shows what the dialogue said, so a refusal that arrived late is
//! visible as a value taken after one that could not be.
//!
//! The check that must fail: make the verifier answer accept a program that exits zero on
//! anything — for example by dropping the negative-control demonstration — and
//! `a_verifier_that_accepts_the_negative_control_never_enters_a_contract` reports a drafted
//! contract with a zero exit.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use tempfile::TempDir;

/// The questions the replaced dialogue put. The product now proposes what it can and takes what
/// the operator states, so none of them may appear — least of all after a line that was refused.
const RETIRED_QUESTIONS: [&str; 3] = [
    "which directory is this work done in",
    "which program decides whether a candidate is accepted",
    "which deliberately wrong candidate must that program reject",
];

/// How the transcript states that a contract exists. The absence of a contract is stated with a
/// sentence of its own, so the word alone would match a store that drafted nothing.
const DRAFTED: &str = "drafted · digest";

struct Workspace {
    _root: TempDir,
    root: PathBuf,
    source: PathBuf,
    negative_control: PathBuf,
    verifier: PathBuf,
}

fn workspace() -> Workspace {
    let root = TempDir::new().expect("temporary root");
    let directory = root.path().canonicalize().expect("resolve root");
    let source = directory.join("source");
    let negative_control = directory.join("negative-control");
    fs::create_dir_all(&source).expect("source directory");
    fs::create_dir_all(&negative_control).expect("negative control directory");
    fs::write(source.join("result.txt"), b"result\n").expect("source file");
    let workspace = Workspace {
        _root: root,
        root: directory,
        source,
        negative_control,
        verifier: PathBuf::new(),
    };
    let verifier = workspace.program("verify.sh", "test -f \"$1/result.txt\"");
    Workspace {
        verifier,
        ..workspace
    }
}

impl Workspace {
    fn program(&self, name: &str, body: &str) -> PathBuf {
        let path = self.root.join(name);
        fs::write(&path, format!("#!/bin/sh\n{body}\n")).expect("write program");
        let mut permissions = fs::metadata(&path).expect("metadata").permissions();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            permissions.set_mode(0o700);
        }
        #[cfg(not(unix))]
        permissions.set_readonly(false);
        fs::set_permissions(&path, permissions).expect("make executable");
        path
    }

    /// Draft a contract from the four answers, as the interface's draft collects them.
    fn request(&self, name: &str, source: &Path, verifier: &Path) -> Drafted {
        let data_root = self.root.join(name);
        let output = Command::new(env!("CARGO_BIN_EXE_ymp"))
            .arg("--data-root")
            .arg(&data_root)
            .arg("request")
            .arg("--prompt=keep the replay path idempotent")
            .arg(format!("--source={}", source.display()))
            .arg(format!("--verifier={}", verifier.display()))
            .arg(format!(
                "--negative-control={}",
                self.negative_control.display()
            ))
            .output()
            .expect("run the ymp executable");
        Drafted { data_root, output }
    }
}

struct Drafted {
    data_root: PathBuf,
    output: Output,
}

impl Drafted {
    fn refusal(&self) -> String {
        assert!(
            !self.output.status.success(),
            "the request was accepted:\n{}",
            self.transcript()
        );
        assert!(
            !self.data_root.join("events.jsonl").exists(),
            "a refused request wrote a journal"
        );
        assert!(
            !self.transcript().contains(DRAFTED),
            "a refused request drafted a contract:\n{}",
            self.transcript()
        );
        String::from_utf8_lossy(&self.output.stderr).into_owned()
    }

    fn transcript(&self) -> String {
        String::from_utf8_lossy(&self.output.stdout).into_owned()
    }

    /// The questions the transcript put, of those the replaced dialogue used to put.
    ///
    /// The transcript is wrapped to the width a command lays its surfaces out at, so a question
    /// is looked for in the text with its line breaks and indentation collapsed.
    fn questions_asked(&self) -> Vec<&'static str> {
        let transcript = self.transcript();
        let flattened = transcript.split_whitespace().collect::<Vec<_>>().join(" ");
        RETIRED_QUESTIONS
            .into_iter()
            .filter(|question| flattened.contains(question))
            .collect()
    }
}

#[test]
fn a_verifier_answer_that_is_not_an_executable_file_is_refused_at_that_answer() {
    let workspace = workspace();
    let command_line = PathBuf::from(format!("{} --strict", workspace.verifier.display()));

    let drafted = workspace.request("command-line", &workspace.source, &command_line);
    let refusal = drafted.refusal();
    assert!(
        refusal.contains("could not be read") && refusal.contains("--strict"),
        "the refusal does not name the answer: {refusal}"
    );
    assert!(
        drafted.questions_asked().is_empty(),
        "the dialogue put a question for a verifier that could not be taken:\n{}",
        drafted.transcript()
    );
}

#[test]
fn a_verifier_that_accepts_the_negative_control_never_enters_a_contract() {
    let workspace = workspace();
    let accepts_anything = workspace.program("accept.sh", "exit 0");

    let drafted = workspace.request("accepts-anything", &workspace.source, &accepts_anything);
    let refusal = drafted.refusal();
    assert!(
        refusal.contains("accepted the negative control"),
        "the refusal does not name the reason: {refusal}"
    );
    assert!(
        refusal.contains(&accepts_anything.display().to_string()),
        "the refusal does not name the program: {refusal}"
    );
}

#[test]
fn a_source_that_does_not_exist_is_refused_before_the_remaining_lines() {
    let workspace = workspace();
    let missing = workspace.root.join("no-such-directory");

    let drafted = workspace.request("missing-source", &missing, &workspace.verifier);
    let refusal = drafted.refusal();
    assert!(
        refusal.contains(&missing.display().to_string()) && refusal.contains("could not be read"),
        "the refusal names neither the path nor what this host reported: {refusal}"
    );
    assert!(
        drafted.questions_asked().is_empty(),
        "a question was put for a source that could not be taken:\n{}",
        drafted.transcript()
    );
}

/// The positive half: the three refusals above are decisions about the answers, not a draft that
/// refuses everything. The same command with a verifier that rejects the negative control drafts
/// its contract and offers the run.
#[test]
fn a_verifier_that_rejects_the_negative_control_drafts_its_contract() {
    let workspace = workspace();
    let drafted = workspace.request("discriminating", &workspace.source, &workspace.verifier);
    assert!(
        drafted.output.status.success(),
        "a discriminating verifier was refused:\n{}\n{}",
        drafted.transcript(),
        String::from_utf8_lossy(&drafted.output.stderr)
    );
    let transcript = drafted.transcript();
    assert!(transcript.contains(DRAFTED), "{transcript}");
    assert!(transcript.contains("verify.sh"), "{transcript}");
    assert!(
        !drafted.data_root.join("events.jsonl").exists(),
        "drafting a contract started a run"
    );
}
