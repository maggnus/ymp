#![forbid(unsafe_code)]

//! Acceptance, driven on the built product: a project with no tests of its own still reaches a
//! contract, and what decides it is a program generated from the request and shown before it is
//! approved.
//!
//! The scenario is the one an operator measured: an empty directory and a request to create an
//! empty html file. It reaches one drafted contract whose verifier ymp wrote, which rejects a
//! fresh copy of that directory and accepts a candidate carrying the file. The draft shows the
//! program in full and names the digest the contract pins it by, so the approval is given on
//! something read rather than on something described.
//!
//! The negative half is the refusal that remains: a request naming no result this host can look
//! for is refused in the same directory, nothing is recorded, and the refusal says what is left to
//! the operator.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use tempfile::TempDir;

struct Project {
    _root: TempDir,
    directory: PathBuf,
    data_root: PathBuf,
    scratch: PathBuf,
}

/// A project as an operator has one before anything has been done in it: a directory, with no way
/// of running tests for a verifier to be proposed from.
fn project() -> Project {
    let root = TempDir::new().expect("temporary root");
    let base = root.path().canonicalize().expect("resolve root");
    let directory = base.join("work");
    let scratch = base.join("scratch");
    fs::create_dir_all(&directory).expect("project directory");
    fs::create_dir_all(&scratch).expect("scratch directory");
    Project {
        _root: root,
        directory,
        data_root: base.join("data"),
        scratch,
    }
}

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

    /// The program the product generated for this draft, as it stands on disk.
    fn generated_program(&self) -> PathBuf {
        let drafts = self.data_root.join("draft");
        let mut found: Vec<PathBuf> = fs::read_dir(&drafts)
            .expect("the product wrote a draft workspace")
            .filter_map(Result::ok)
            .map(|entry| entry.path().join("generated-verify.sh"))
            .filter(|path| path.is_file())
            .collect();
        assert_eq!(
            found.len(),
            1,
            "the draft left {} generated programs behind",
            found.len()
        );
        found.remove(0)
    }

    /// A directory carrying exactly the files named.
    fn candidate(&self, name: &str, files: &[(&str, &[u8])]) -> PathBuf {
        let path = self.scratch.join(name);
        fs::create_dir_all(&path).expect("candidate directory");
        for (relative, bytes) in files {
            fs::write(path.join(relative), bytes).expect("candidate file");
        }
        path
    }
}

/// Run the generated program against one directory, as the executor runs it: the directory is the
/// only argument, and only 0 and 1 mean anything.
fn decides(program: &Path, subject: &Path) -> bool {
    let status = Command::new(program)
        .arg(subject)
        .current_dir(subject)
        .status()
        .expect("run the generated program");
    match status.code() {
        Some(0) => true,
        Some(1) => false,
        code => panic!("the generated program exited with {code:?}, which decides nothing"),
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

/// A 1×1 picture, carried verbatim so what the check is measured against is a picture rather than
/// a header written to satisfy it.
const PNG: &str = "89504e470d0a1a0a0000000d4948445200000001000000010806000000\
                   1f15c4890000000d4944415478da63fccfc0500f000485018084a98c2100000000\
                   49454e44ae426082";

fn picture(hexadecimal: &str) -> Vec<u8> {
    let digits: Vec<char> = hexadecimal
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect();
    digits
        .chunks(2)
        .map(|pair| {
            u8::from_str_radix(&pair.iter().collect::<String>(), 16).expect("hexadecimal byte")
        })
        .collect()
}

#[test]
fn a_testless_project_reaches_a_generated_check_it_can_authorize() {
    let project = project();
    let drafted = project.run(&["request", "--prompt=create an empty html file"]);
    let transcript = flattened(&drafted);

    assert!(
        drafted.status.success(),
        "a request in a project that runs no tests was refused:\n{transcript}\n{}",
        String::from_utf8_lossy(&drafted.stderr)
    );
    assert_eq!(
        transcript.matches("drafted · digest").count(),
        1,
        "the request produced more than one contract to judge:\n{transcript}"
    );

    // What the operator is asked to approve: where the program came from, the one claim it
    // decides, what it leaves to them, and the program itself.
    for stated in [
        "generated by ymp from your request",
        "a file named *.html was produced",
        "stays yours to judge",
        "No model was asked to write it",
        "the program in full",
        "#!/bin/sh",
        "rejected the negative control, and accepted a copy of the project carrying an empty file \
         named sample.html",
    ] {
        assert!(
            transcript.contains(stated),
            "the draft does not state `{stated}`:\n{transcript}"
        );
    }

    // The digest shown is the digest of the program the product wrote, which is what the contract
    // pins the oracle by.
    let program = project.generated_program();
    let bytes = fs::read(&program).expect("read the generated program");
    let digest = ymp_domain::digest_bytes(&bytes);
    assert!(
        transcript.contains(&digest[..10]),
        "the draft states no digest for the program it shows:\n{transcript}"
    );

    // And the program decides the scenario both ways: a fresh copy of the project holds no
    // result, and a candidate carrying the file does.
    let fresh = project.candidate("fresh", &[]);
    let carrying = project.candidate("carrying", &[("index.html", b"")]);
    assert!(
        !decides(&program, &fresh),
        "the generated check accepted a fresh copy of the project"
    );
    assert!(
        decides(&program, &carrying),
        "the generated check rejected a candidate carrying the file the request asked for"
    );

    assert!(
        !project.data_root.join("events.jsonl").exists(),
        "drafting a generated check started a run"
    );

    // The invitation belongs to the opening of the transcript, so it is never restated under the
    // draft that answered it. What stands instead is the status line, which carries the same hint
    // for as long as a request can be stated and costs no transcript line to do it.
    let invitation = "state your request below in one line";
    assert!(
        transcript.matches(invitation).count() <= 1,
        "the invitation is stated more than once:\n{transcript}"
    );
    let drafted_at = transcript
        .find("drafted · digest")
        .expect("the transcript states the draft");
    assert!(
        transcript
            .rfind(invitation)
            .is_none_or(|stated_at| stated_at < drafted_at),
        "the invitation is restated under the draft it answered:\n{transcript}"
    );
    assert!(
        transcript.contains("state your request in one line · store"),
        "the standing hint left the status line:\n{transcript}"
    );
}

/// A request whose result is a picture is checked as a picture: the bytes are read, and what the
/// picture means is named as the operator's own.
#[test]
fn a_request_for_a_picture_checks_the_mechanics_and_names_the_rest_as_the_operators() {
    let project = project();
    let drafted = project.run(&["request", "--prompt=create an image of the release path"]);
    let transcript = flattened(&drafted);
    assert!(
        drafted.status.success(),
        "a request for a picture was refused:\n{transcript}\n{}",
        String::from_utf8_lossy(&drafted.stderr)
    );

    for stated in [
        "decode as an image with non-zero width and height",
        "whether the picture shows what you meant",
        "stay yours to judge",
        // A program too long to state whole is stated to its bound, and the rest is addressed by
        // the path holding it rather than dropped silently.
        "the program's first 24 lines of",
        "#!/bin/sh",
    ] {
        assert!(
            transcript.contains(stated),
            "the draft does not state `{stated}`:\n{transcript}"
        );
    }

    let program = project.generated_program();
    let picture = project.candidate("picture", &[("diagram.png", &picture(PNG))]);
    let named_only = project.candidate("named-only", &[("diagram.png", b"not a picture\n")]);
    assert!(
        decides(&program, &picture),
        "a candidate carrying a picture was rejected"
    );
    assert!(
        !decides(&program, &named_only),
        "a file that only carries the name of a picture was accepted as one"
    );
}

/// The refusal that remains. A request stating no result this host can look for derives nothing,
/// so nothing is offered, nothing is recorded, and the refusal names what is left to the operator.
#[test]
fn a_request_stating_no_artifact_is_refused_and_records_nothing() {
    let project = project();
    let refused = project.run(&["request", "--prompt=keep the replay path idempotent"]);
    let transcript = flattened(&refused);

    assert!(
        !refused.status.success(),
        "a request nothing could be derived from produced a contract:\n{transcript}"
    );
    assert!(
        transcript.contains("state a verifier of your own"),
        "the refusal does not say what is left to the operator:\n{transcript}"
    );
    assert!(
        !transcript.contains("drafted · digest"),
        "a refused request still drafted a contract:\n{transcript}"
    );
    assert!(
        !project.data_root.join("events.jsonl").exists(),
        "a refused request wrote a journal"
    );
}
